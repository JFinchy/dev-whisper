//! Transcribe while you talk: during a push-to-talk recording a worker
//! thread runs Silero VAD over the incoming audio and cuts it at natural
//! pauses; each chunk is transcribed in the background while you keep
//! speaking. When you stop, only the audio after the last cut is left, and
//! silence between phrases (a minute spent reading something, say) is never
//! transcribed at all.
//!
//! Only speech, plus `PAD` of context on each side, reaches Whisper. Chunks
//! are transcribed one at a time, each prompted with the text before it.
//!
//! Isolated Voice runs per chunk (`isolate::apply`): it already scores each
//! voiced range on its own, so chunking changes which audio each call sees
//! but not what any range is judged against.
//!
//! Anything unexpected (VAD model missing, a chunk failing, a very short
//! recording, no speech found) falls back to transcribing the whole buffer
//! exactly as before; see `plan_stop`.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};
use tauri::{AppHandle, Emitter, Manager};

use crate::recording::RecordingState;

const SAMPLE_RATE: usize = crate::stt::WHISPER_SAMPLE_RATE as usize;
/// Silero's frame size at 16kHz: the chunker's unit of time (32 ms).
const WINDOW: usize = 512;
const fn ms(n: usize) -> usize {
    n * SAMPLE_RATE / 1000
}
/// Audio kept on each side of speech so word edges and soft onsets aren't
/// clipped.
const PAD: usize = ms(300);
/// Non-speech needed after a phrase before it can be cut off as a chunk...
const MIN_PAUSE: usize = ms(600);
/// ...provided the chunk holds at least this much speech (shorter pieces
/// transcribe worse and cost the same fixed overhead).
const MIN_CHUNK_SPEECH: usize = ms(1500);
/// After this much speech a briefer pause is enough, so a long monologue
/// still gets cut at a word boundary...
const SOFT_CAP: usize = ms(20_000);
const SOFT_CAP_PAUSE: usize = ms(150);
/// ...and speech with no gap at all is cut here regardless, well inside
/// Whisper's 30 s window.
const HARD_CAP: usize = ms(28_000);
/// Speech runs shorter than this (a click, a cough) are not worth a chunk.
const MIN_RUN: usize = 3 * WINDOW;
/// Recordings shorter than this just use the whole-buffer path.
const MIN_DICTATION: usize = 3 * SAMPLE_RATE;
/// Chunks waiting for the transcriber; the VAD thread blocks beyond this
/// (the audio keeps buffering meanwhile) instead of piling up work.
const QUEUE_DEPTH: usize = 2;
const POLL: Duration = Duration::from_millis(100);

/// A stretch of audio worth transcribing: speech plus padding, as sample
/// ranges into the recording (silence between ranges is dropped).
#[derive(Debug, PartialEq, Eq)]
pub struct Chunk {
    pub ranges: Vec<(usize, usize)>,
}

impl Chunk {
    pub fn len(&self) -> usize {
        self.ranges.iter().map(|(s, e)| e - s).sum()
    }

    pub fn audio(&self, recording: &[f32]) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.len());
        for &(s, e) in &self.ranges {
            out.extend_from_slice(&recording[s.min(recording.len())..e.min(recording.len())]);
        }
        out
    }
}

/// Turns a stream of per-window speech/non-speech decisions into chunks.
/// Knows nothing about audio, so the cutting rules are testable with
/// scripted decisions.
#[derive(Default)]
pub struct Chunker {
    /// Samples seen so far (always a whole number of windows).
    pos: usize,
    /// Start of the speech run in progress.
    run_start: Option<usize>,
    last_speech_end: usize,
    /// Closed speech runs in the chunk being built, as `(start, end)`.
    runs: Vec<(usize, usize)>,
    /// Where the previous chunk ended, so padding never re-includes audio
    /// that's already been transcribed.
    prev_end: usize,
}

impl Chunker {
    pub fn feed(&mut self, speech: bool) -> Option<Chunk> {
        let start = self.pos;
        let end = start + WINDOW;
        self.pos = end;

        if speech {
            self.run_start.get_or_insert(start);
            self.last_speech_end = end;
        } else if let Some(run_start) = self.run_start.take() {
            self.close_run(run_start, self.last_speech_end);
        }

        let in_speech = self.run_start.is_some();
        let speech_len = self.speech_len();
        if !in_speech && !self.runs.is_empty() {
            let quiet = end - self.last_speech_end;
            let enough_speech = speech_len >= MIN_CHUNK_SPEECH && quiet >= MIN_PAUSE;
            let long_enough = speech_len >= SOFT_CAP && quiet >= SOFT_CAP_PAUSE;
            if enough_speech || long_enough {
                return Some(self.cut(end));
            }
        }
        if speech_len >= HARD_CAP {
            // No gap to cut at; split the run and keep it going.
            if let Some(run_start) = self.run_start {
                self.runs.push((run_start, end));
                self.run_start = Some(end);
            }
            return Some(self.cut(end));
        }
        None
    }

    /// The recording ended after `total` samples: whatever speech is still
    /// pending becomes the final chunk.
    pub fn finish(&mut self, total: usize) -> Option<Chunk> {
        if let Some(run_start) = self.run_start.take() {
            self.close_run(run_start, self.last_speech_end);
        }
        (!self.runs.is_empty()).then(|| self.cut(total))
    }

    fn close_run(&mut self, start: usize, end: usize) {
        if end - start >= MIN_RUN {
            self.runs.push((start, end));
        }
    }

    fn speech_len(&self) -> usize {
        let closed: usize = self.runs.iter().map(|(s, e)| e - s).sum();
        closed + self.run_start.map_or(0, |s| self.last_speech_end - s)
    }

    /// Pads and merges the pending runs into a chunk ending no later than
    /// `limit`.
    fn cut(&mut self, limit: usize) -> Chunk {
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for (s, e) in self.runs.drain(..) {
            let lo = s.saturating_sub(PAD).max(self.prev_end);
            let hi = (e + PAD).min(limit);
            match ranges.last_mut() {
                Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
                _ => ranges.push((lo, hi)),
            }
        }
        if let Some(&(_, hi)) = ranges.last() {
            self.prev_end = hi;
        }
        Chunk { ranges }
    }
}

/// A [`Chunker`] plus the audio it's cutting: feed it 16 kHz samples in
/// any size, get back the audio of each finished chunk.
pub struct Segmenter {
    chunker: Chunker,
    audio: Vec<f32>,
    /// How much of `audio` the chunker has seen.
    analyzed: usize,
    /// Samples handed out in chunks so far.
    emitted: usize,
}

impl Segmenter {
    pub fn new() -> Self {
        Self {
            chunker: Chunker::default(),
            audio: Vec::new(),
            analyzed: 0,
            emitted: 0,
        }
    }

    pub fn push(
        &mut self,
        samples: &[f32],
        is_speech: &mut dyn FnMut(&[f32]) -> bool,
    ) -> Vec<Vec<f32>> {
        self.audio.extend_from_slice(samples);
        let mut chunks = Vec::new();
        while self.audio.len() - self.analyzed >= WINDOW {
            let window = &self.audio[self.analyzed..self.analyzed + WINDOW];
            let speech = is_speech(window);
            self.analyzed += WINDOW;
            if let Some(chunk) = self.chunker.feed(speech) {
                self.emitted += chunk.len();
                chunks.push(chunk.audio(&self.audio));
            }
        }
        chunks
    }

    /// Ends the recording and returns the audio after the last cut, if any
    /// of it is speech.
    pub fn finish(&mut self) -> Option<Vec<f32>> {
        let chunk = self.chunker.finish(self.audio.len())?;
        self.emitted += chunk.len();
        Some(chunk.audio(&self.audio))
    }

    pub fn total_samples(&self) -> usize {
        self.audio.len()
    }

    pub fn emitted_samples(&self) -> usize {
        self.emitted
    }
}

/// Joins chunk transcripts with single spaces, skipping empty ones.
pub fn join_texts(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resamples a stream to 16 kHz piece by piece, producing exactly the
/// samples `stt::resample_linear` would for the whole recording, so
/// positions found while recording line up with the saved clip.
struct StreamResampler {
    ratio: f64,
    raw: Vec<f32>,
    /// Index into the full input of `raw[0]`.
    raw_base: usize,
    produced: usize,
}

impl StreamResampler {
    fn new(from_rate: u32) -> Self {
        Self {
            ratio: from_rate as f64 / crate::stt::WHISPER_SAMPLE_RATE as f64,
            raw: Vec::new(),
            raw_base: 0,
            produced: 0,
        }
    }

    fn push(&mut self, mono: &[f32]) -> Vec<f32> {
        self.raw.extend_from_slice(mono);
        let mut out = Vec::new();
        loop {
            let src = self.produced as f64 * self.ratio;
            let idx = src.floor() as usize;
            if idx + 1 >= self.raw_base + self.raw.len() {
                break;
            }
            let frac = (src - idx as f64) as f32;
            let a = self.raw[idx - self.raw_base];
            let b = self.raw[idx + 1 - self.raw_base];
            out.push(a + (b - a) * frac);
            self.produced += 1;
        }
        let keep_from = (self.produced as f64 * self.ratio).floor() as usize;
        let drop = keep_from.saturating_sub(self.raw_base).min(self.raw.len());
        self.raw.drain(..drop);
        self.raw_base += drop;
        out
    }
}

/// What the worker hands back when the recording stops.
#[derive(Default)]
pub struct LiveOutcome {
    /// Transcripts of the chunks cut while recording, in order.
    pub texts: Vec<String>,
    /// Audio after the last cut (speech only), still to be transcribed.
    pub tail: Option<Vec<f32>>,
    /// The VAD couldn't be loaded; nothing was chunked.
    pub unavailable: bool,
    /// A background chunk failed to transcribe, so `texts` has a hole.
    pub failed: bool,
    pub total_samples: usize,
    /// Samples that went (or will go) to Whisper: chunks plus tail.
    pub kept_samples: usize,
    /// How long stopping waited on chunks still being transcribed.
    pub drain_ms: u64,
}

impl LiveOutcome {
    pub fn skipped_seconds(&self) -> f32 {
        self.total_samples.saturating_sub(self.kept_samples) as f32 / SAMPLE_RATE as f32
    }
}

/// How to transcribe a stopped recording.
#[derive(Debug, PartialEq, Eq)]
pub enum StopPlan {
    /// Chunks are done; transcribe the tail and join.
    Chunked,
    /// The old path: transcribe the whole recording. Carries why.
    WholeBuffer(&'static str),
}

pub fn plan_stop(total_samples: usize, outcome: Option<&LiveOutcome>) -> StopPlan {
    if total_samples < MIN_DICTATION {
        return StopPlan::WholeBuffer("recording under 3s");
    }
    let Some(outcome) = outcome else {
        return StopPlan::WholeBuffer("live transcription was off");
    };
    if outcome.unavailable {
        return StopPlan::WholeBuffer("speech detection unavailable");
    }
    if outcome.failed {
        return StopPlan::WholeBuffer("a chunk failed to transcribe");
    }
    if outcome.texts.is_empty() && outcome.tail.is_none() {
        return StopPlan::WholeBuffer("no speech found");
    }
    StopPlan::Chunked
}

/// The background half of one push-to-talk recording. Dropping it without
/// calling `finish` (cancelled or failed recording) stops the worker.
pub struct LiveSession {
    finish_tx: mpsc::Sender<Vec<f32>>,
    handle: JoinHandle<LiveOutcome>,
}

impl LiveSession {
    pub fn start(app: &AppHandle) -> Self {
        let (finish_tx, finish_rx) = mpsc::channel();
        let app = app.clone();
        let handle = std::thread::spawn(move || run(&app, finish_rx));
        Self { finish_tx, handle }
    }

    /// Hands the worker the finished recording (16 kHz mono), waits for
    /// the chunks still in flight, and returns what's left to transcribe.
    pub fn finish(self, recording: Vec<f32>) -> Option<LiveOutcome> {
        self.finish_tx.send(recording).ok()?;
        self.handle.join().ok()
    }
}

#[derive(Default)]
struct Transcripts {
    texts: Vec<String>,
    failed: bool,
}

fn run(app: &AppHandle, finish_rx: mpsc::Receiver<Vec<f32>>) -> LiveOutcome {
    let vad = match create_vad(app) {
        Ok(vad) => vad,
        Err(err) => {
            crate::applog!("live: speech detection unavailable, using whole-recording path: {err}");
            // Wait for the stop so the caller gets a definite answer.
            let _ = finish_rx.recv();
            return LiveOutcome {
                unavailable: true,
                ..Default::default()
            };
        }
    };

    let transcripts = Arc::new(Mutex::new(Transcripts::default()));
    let (job_tx, job_rx) = mpsc::sync_channel::<Vec<f32>>(QUEUE_DEPTH);
    let transcriber = {
        let (app, transcripts) = (app.clone(), transcripts.clone());
        std::thread::spawn(move || transcribe_jobs(&app, &job_rx, &transcripts))
    };

    let mut is_speech = |window: &[f32]| {
        vad.accept_waveform(window);
        let speech = vad.detected();
        vad.clear();
        speech
    };
    let mut segmenter = Segmenter::new();
    let mut resampler: Option<StreamResampler> = None;
    let mut offset = 0usize;

    let recording = loop {
        match finish_rx.recv_timeout(POLL) {
            Ok(recording) => break recording,
            Err(RecvTimeoutError::Disconnected) => {
                // Cancelled. Closing the queue ends the transcriber.
                drop(job_tx);
                let _ = transcriber.join();
                return LiveOutcome::default();
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        let (raw, channels, rate) = app.state::<RecordingState>().audio.recorded_since(offset);
        if raw.is_empty() || rate == 0 {
            continue;
        }
        offset += raw.len();
        let mono = crate::stt::downmix(&raw, channels);
        let resampler = resampler.get_or_insert_with(|| StreamResampler::new(rate));
        let samples = resampler.push(&mono);
        for chunk in segmenter.push(&samples, &mut is_speech) {
            if job_tx.send(chunk).is_err() {
                break;
            }
        }
    };

    // Catch up on what arrived after the last poll, from the saved clip
    // (whose first samples are the ones already analyzed).
    let analyzed = segmenter.total_samples();
    if recording.len() > analyzed {
        for chunk in segmenter.push(&recording[analyzed..], &mut is_speech) {
            let _ = job_tx.send(chunk);
        }
    }
    let tail = segmenter.finish();

    let drain_started = Instant::now();
    drop(job_tx);
    let _ = transcriber.join();
    let drain_ms = drain_started.elapsed().as_millis() as u64;

    let transcripts = std::mem::take(&mut *transcripts.lock().unwrap());
    LiveOutcome {
        texts: transcripts.texts,
        kept_samples: segmenter.emitted_samples(),
        total_samples: segmenter.total_samples().max(recording.len()),
        tail,
        unavailable: false,
        failed: transcripts.failed,
        drain_ms,
    }
}

/// Transcribes queued chunks in order, prompting each with the text so far
/// and publishing the running transcript for the widget.
fn transcribe_jobs(
    app: &AppHandle,
    jobs: &mpsc::Receiver<Vec<f32>>,
    transcripts: &Mutex<Transcripts>,
) {
    for audio in jobs {
        let state = app.state::<RecordingState>();
        let model = crate::recording::resolve_mode(app, false).model;
        let context = join_texts(&transcripts.lock().unwrap().texts);
        let started = Instant::now();
        let audio_s = audio.len() as f32 / SAMPLE_RATE as f32;
        let audio = crate::isolate::apply(app, audio);
        let result = state
            .whisper
            .transcribe_samples_in_context(&audio, model, &context, &|_| {});
        match result {
            Ok(text) => {
                crate::applog!(
                    "live: chunk of {audio_s:.1}s transcribed in {}ms",
                    started.elapsed().as_millis()
                );
                let text = crate::stt::strip_non_speech(&text);
                let mut transcripts = transcripts.lock().unwrap();
                if !text.is_empty() {
                    transcripts.texts.push(text);
                    let _ = app.emit("live-transcript", join_texts(&transcripts.texts));
                }
            }
            Err(err) => {
                crate::applog!("live: chunk failed, will transcribe the whole recording: {err}");
                transcripts.lock().unwrap().failed = true;
            }
        }
    }
}

fn create_vad(app: &AppHandle) -> Result<VoiceActivityDetector, String> {
    let model = crate::ambient::vad_model_path(app)
        .ok_or("speech detection model (silero_vad.onnx) not found")?;
    vad_from_model(&model)
}

pub(crate) fn vad_from_model(model: &std::path::Path) -> Result<VoiceActivityDetector, String> {
    // Short hysteresis: `detected()` is read per window and the chunker
    // applies its own pause and length rules on top.
    let config = VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(model.to_string_lossy().to_string()),
            threshold: 0.5,
            min_silence_duration: 0.1,
            min_speech_duration: 0.05,
            window_size: WINDOW as i32,
            max_speech_duration: 10.0,
        },
        sample_rate: SAMPLE_RATE as i32,
        num_threads: 1,
        ..Default::default()
    };
    VoiceActivityDetector::create(&config, 30.0)
        .ok_or_else(|| "couldn't initialize speech detection".to_string())
}

#[tauri::command]
pub fn get_live_transcribe_enabled(app: AppHandle) -> bool {
    crate::config::load(&app).live_transcribe_enabled
}

#[tauri::command]
pub fn set_live_transcribe_enabled(app: AppHandle, enabled: bool) {
    let mut cfg = crate::config::load(&app);
    cfg.live_transcribe_enabled = enabled;
    let _ = crate::config::save(&app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = WINDOW;

    /// Windows covering `secs` seconds.
    fn windows(secs: f32) -> usize {
        (secs * SAMPLE_RATE as f32 / W as f32).round() as usize
    }

    /// Feeds `secs` of speech or silence, collecting any chunks.
    fn feed(c: &mut Chunker, speech: bool, secs: f32, out: &mut Vec<Chunk>) {
        for _ in 0..windows(secs) {
            out.extend(c.feed(speech));
        }
    }

    fn secs(samples: usize) -> f32 {
        samples as f32 / SAMPLE_RATE as f32
    }

    #[test]
    fn cuts_at_a_pause_after_enough_speech() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 3.0, &mut out);
        assert!(out.is_empty());
        feed(&mut c, false, 0.5, &mut out);
        assert!(out.is_empty(), "0.5s is not a pause yet");
        feed(&mut c, false, 0.2, &mut out);
        assert_eq!(out.len(), 1);
        let (lo, hi) = out[0].ranges[0];
        assert_eq!(lo, 0, "padding can't reach before the recording");
        // Speech ends at 3.0s; padding adds 0.3s.
        assert!((secs(hi) - 3.3).abs() < 0.05, "{}", secs(hi));
    }

    #[test]
    fn short_speech_is_held_until_there_is_enough() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 1.0, &mut out);
        feed(&mut c, false, 5.0, &mut out);
        assert!(out.is_empty(), "1s of speech is below the 1.5s minimum");
        feed(&mut c, true, 1.0, &mut out);
        feed(&mut c, false, 0.7, &mut out);
        assert_eq!(out.len(), 1);
        // Two separate speech ranges; the 5s of silence between is gone.
        assert_eq!(out[0].ranges.len(), 2);
        assert!(secs(out[0].len()) < 3.0, "{}", secs(out[0].len()));
    }

    #[test]
    fn a_long_pause_costs_nothing() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 2.0, &mut out);
        feed(&mut c, false, 60.0, &mut out);
        feed(&mut c, true, 2.0, &mut out);
        feed(&mut c, false, 1.0, &mut out);
        assert_eq!(out.len(), 2);
        let kept: usize = out.iter().map(Chunk::len).sum();
        assert!(secs(kept) < 5.5, "kept {}s of 65s", secs(kept));
    }

    #[test]
    fn chunks_never_overlap() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        for _ in 0..3 {
            feed(&mut c, true, 2.0, &mut out);
            feed(&mut c, false, 0.7, &mut out);
        }
        assert_eq!(out.len(), 3);
        for pair in out.windows(2) {
            let end = pair[0].ranges.last().unwrap().1;
            let next = pair[1].ranges.first().unwrap().0;
            assert!(next >= end, "{next} < {end}");
        }
    }

    #[test]
    fn brief_pauses_stay_inside_one_chunk() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        for _ in 0..4 {
            feed(&mut c, true, 1.0, &mut out);
            feed(&mut c, false, 0.3, &mut out);
        }
        assert!(out.is_empty());
        let tail = c.finish(windows(5.2) * W).unwrap();
        // 0.3s gaps are bridged by the padding on both sides.
        assert_eq!(tail.ranges.len(), 1);
    }

    #[test]
    fn leading_silence_is_trimmed_to_the_padding() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, false, 1.0, &mut out);
        feed(&mut c, true, 2.0, &mut out);
        feed(&mut c, false, 0.7, &mut out);
        let (lo, hi) = out[0].ranges[0];
        assert!((secs(lo) - 0.7).abs() < 0.05, "{}", secs(lo));
        assert!((secs(hi) - 3.3).abs() < 0.05, "{}", secs(hi));
    }

    #[test]
    fn continuous_speech_is_cut_at_the_cap() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 70.0, &mut out);
        assert_eq!(out.len(), 2, "28s hard cap");
        for chunk in &out {
            assert!(secs(chunk.len()) <= 28.5);
        }
        // The speech in progress keeps going into the next chunk.
        let tail = c.finish(windows(70.0) * W).unwrap();
        assert!(secs(tail.len()) > 13.0);
    }

    #[test]
    fn a_long_monologue_is_cut_at_the_first_small_gap_past_the_soft_cap() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 21.0, &mut out);
        assert!(out.is_empty());
        feed(&mut c, false, 0.2, &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn clicks_do_not_make_chunks() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 0.05, &mut out);
        feed(&mut c, false, 3.0, &mut out);
        assert!(out.is_empty());
        assert!(c.finish(windows(3.05) * W).is_none());
    }

    #[test]
    fn finish_returns_pending_speech_and_nothing_for_silence() {
        let mut c = Chunker::default();
        let mut out = Vec::new();
        feed(&mut c, true, 2.0, &mut out);
        feed(&mut c, false, 0.3, &mut out);
        let total = windows(2.3) * W;
        let tail = c.finish(total).unwrap();
        assert_eq!(tail.ranges.last().unwrap().1, total);
        assert!(Chunker::default().finish(0).is_none());
    }

    #[test]
    fn join_skips_empty_parts_and_trims() {
        let parts = ["  Hello there. ", "", "How are you?  ", "  "].map(String::from);
        assert_eq!(join_texts(&parts), "Hello there. How are you?");
        assert_eq!(join_texts(&[]), "");
    }

    fn loud(secs_: f32) -> Vec<f32> {
        (0..(secs_ * SAMPLE_RATE as f32) as usize)
            .map(|i| (i as f32 * 0.1).sin() * 0.3)
            .collect()
    }

    fn quiet(secs_: f32) -> Vec<f32> {
        vec![0.0; (secs_ * SAMPLE_RATE as f32) as usize]
    }

    fn energy(window: &[f32]) -> bool {
        window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32 > 0.001
    }

    #[test]
    fn segmenter_drops_silence_and_is_independent_of_push_size() {
        let mut recording = Vec::new();
        recording.extend(loud(2.0));
        recording.extend(quiet(45.0));
        recording.extend(loud(2.5));
        recording.extend(quiet(1.0));
        recording.extend(loud(1.0));

        let mut chunk_sizes = Vec::new();
        for push_size in [700, 4096, 160_000] {
            let mut seg = Segmenter::new();
            let mut chunks: Vec<Vec<f32>> = Vec::new();
            for piece in recording.chunks(push_size) {
                chunks.extend(seg.push(piece, &mut energy));
            }
            chunks.push(seg.finish().expect("trailing speech"));
            assert_eq!(chunks.len(), 3, "push size {push_size}");
            assert!(seg.emitted_samples() < recording.len() / 5);
            chunk_sizes.push(chunks.iter().map(Vec::len).collect::<Vec<_>>());
        }
        assert!(
            chunk_sizes.windows(2).all(|w| w[0] == w[1]),
            "{chunk_sizes:?}"
        );
    }

    #[test]
    fn segmenter_with_only_silence_has_no_tail() {
        let mut seg = Segmenter::new();
        assert!(seg.push(&quiet(5.0), &mut energy).is_empty());
        assert!(seg.finish().is_none());
    }

    #[test]
    fn stream_resampler_matches_whole_buffer_resampling() {
        for (rate, len) in [(48_000u32, 96_000usize), (44_100, 44_100)] {
            let input: Vec<f32> = (0..len).map(|i| (i as f32 * 0.013).sin()).collect();
            let whole = crate::stt::resample_linear(&input, rate, 16_000);
            let mut stream = StreamResampler::new(rate);
            let got: Vec<f32> = input.chunks(1000).flat_map(|p| stream.push(p)).collect();
            assert!(
                got.len() + 2 >= whole.len(),
                "{} vs {}",
                got.len(),
                whole.len()
            );
            for (i, (a, b)) in got.iter().zip(&whole).enumerate() {
                assert!((a - b).abs() < 1e-5, "{rate}Hz sample {i}: {a} vs {b}");
            }
        }
    }

    fn outcome(texts: &[&str], tail: bool) -> LiveOutcome {
        LiveOutcome {
            texts: texts.iter().map(|t| t.to_string()).collect(),
            tail: tail.then(|| vec![0.0; 10]),
            ..Default::default()
        }
    }

    #[test]
    fn short_dictations_use_the_old_path() {
        let o = outcome(&["Hi."], true);
        assert_eq!(
            plan_stop(MIN_DICTATION - 1, Some(&o)),
            StopPlan::WholeBuffer("recording under 3s")
        );
        assert_eq!(plan_stop(MIN_DICTATION, Some(&o)), StopPlan::Chunked);
    }

    #[test]
    fn anything_unexpected_falls_back_to_the_whole_buffer() {
        let long = 10 * SAMPLE_RATE;
        assert!(matches!(plan_stop(long, None), StopPlan::WholeBuffer(_)));
        let mut o = outcome(&["a"], true);
        o.unavailable = true;
        assert!(matches!(
            plan_stop(long, Some(&o)),
            StopPlan::WholeBuffer(_)
        ));
        let mut o = outcome(&["a"], true);
        o.failed = true;
        assert!(matches!(
            plan_stop(long, Some(&o)),
            StopPlan::WholeBuffer(_)
        ));
        assert_eq!(
            plan_stop(long, Some(&outcome(&[], false))),
            StopPlan::WholeBuffer("no speech found")
        );
        assert_eq!(
            plan_stop(long, Some(&outcome(&[], true))),
            StopPlan::Chunked
        );
        assert_eq!(
            plan_stop(long, Some(&outcome(&["a"], false))),
            StopPlan::Chunked
        );
    }
}
