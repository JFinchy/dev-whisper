use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// How many models can stay loaded (Metal-compiled, ready to transcribe)
/// at once. Per-mode model overrides (`AppModeRule.stt_model`) would
/// otherwise repay the multi-second Metal shader compile on every
/// recording that hits a different-model rule; keeping a small pool warm
/// means switching between a handful of models in rotation stays fast.
/// Bounded rather than unbounded since each loaded context holds real
/// memory (tens to hundreds of MB depending on model size).
const MAX_WARM_CONTEXTS: usize = 3;

pub(crate) const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Default developer-jargon vocabulary, used to seed a fresh config (and as
/// a fallback if the persisted list is ever empty). Biases Whisper toward
/// recognizing these terms instead of mishearing them as similar-sounding
/// words. User-editable via Settings — see `config::AppConfig::vocabulary`.
pub fn default_vocabulary() -> Vec<String> {
    [
        "kubectl",
        "JSON",
        "macOS",
        "Hammerspoon",
        "camelCase",
        "snake_case",
        "React",
        "TypeScript",
        "npm",
        "git",
        "Docker",
        "API",
        "async",
        "useState",
        "useEffect",
        "Tauri",
        "Rust",
        "Whisper",
        "CLI",
        "terminal",
        "iTerm",
        "Neovim",
        "init.lua",
        "zshrc",
        "MCP",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

struct LoadedModel {
    context: WhisperContext,
    last_used: Instant,
}

pub struct WhisperEngine {
    default_model_id: Mutex<String>,
    /// Every model id we've ever been told a path for — the default at
    /// construction/`set_default_model`, plus any per-mode override passed
    /// to `transcribe_with_model`. Lets later calls that only pass an id
    /// (e.g. re-warming) resolve a path without the caller re-supplying it.
    model_paths: Mutex<HashMap<String, PathBuf>>,
    vocabulary: Mutex<Vec<String>>,
    /// Names from the project being dictated into (see `project.rs`),
    /// ranked best first; empty when there is none.
    project_terms: Mutex<Vec<String>>,
    contexts: Mutex<HashMap<String, LoadedModel>>,
    /// Learned speed per model: milliseconds of compute per second of
    /// audio, on this Mac. Drives the transcription progress estimate.
    speed_ms_per_audio_s: Mutex<HashMap<String, f32>>,
}

/// Starting guess for an unseen model (base.en on Apple Silicon runs at
/// roughly a tenth of real time); replaced by measurements after the first
/// transcription.
const DEFAULT_SPEED_MS_PER_AUDIO_S: f32 = 100.0;
/// Fixed per-call cost (state setup, Metal dispatch) on top of the
/// per-second cost.
const TRANSCRIBE_OVERHEAD_MS: f32 = 150.0;

/// Whisper's own progress (0-100) for the transcription in flight. Global
/// because the callback can't capture state without leaking it in
/// whisper-rs; safe since transcriptions are serialized by the contexts
/// lock.
static WHISPER_PROGRESS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Progress to show at `elapsed_ms` into a transcription expected to take
/// `expected_ms`: linear to 90%, then easing toward 99% so an underestimate
/// slows down instead of stalling at 100% before the text arrives.
pub fn estimated_percent(elapsed_ms: f32, expected_ms: f32) -> u8 {
    if expected_ms <= 0.0 {
        return 0;
    }
    let ratio = elapsed_ms / expected_ms;
    let percent = if ratio < 0.9 {
        ratio * 100.0
    } else {
        90.0 + 9.0 * (1.0 - (-(ratio - 0.9) * 3.0).exp())
    };
    percent.clamp(0.0, 99.0) as u8
}

impl WhisperEngine {
    pub fn new(default_model_id: String, default_model_path: PathBuf) -> Self {
        let mut model_paths = HashMap::new();
        model_paths.insert(default_model_id.clone(), default_model_path);
        Self {
            default_model_id: Mutex::new(default_model_id),
            model_paths: Mutex::new(model_paths),
            vocabulary: Mutex::new(default_vocabulary()),
            project_terms: Mutex::new(Vec::new()),
            contexts: Mutex::new(HashMap::new()),
            speed_ms_per_audio_s: Mutex::new(HashMap::new()),
        }
    }

    pub fn active_model_id(&self) -> String {
        self.default_model_id.lock().unwrap().clone()
    }

    pub fn set_vocabulary(&self, terms: Vec<String>) {
        *self.vocabulary.lock().unwrap() = terms;
    }

    pub fn set_project_terms(&self, terms: Vec<String>) {
        *self.project_terms.lock().unwrap() = terms;
    }

    /// Switches the global default model (used whenever a recording has no
    /// per-mode `stt_model` override). Does *not* evict it from the warm
    /// pool if already loaded, unlike the old single-context design —
    /// switching back and forth between a couple of models (e.g. global
    /// default vs. one mode's override) stays fast.
    pub fn set_default_model(&self, id: String, path: PathBuf) {
        *self.default_model_id.lock().unwrap() = id.clone();
        self.model_paths.lock().unwrap().insert(id, path);
    }

    /// Loads (or reuses) the default model's context. Exposed separately
    /// from transcribe() so the app can warm it up at startup — first load
    /// pays for Metal shader compilation, which is otherwise a multi-second
    /// delay on the user's first recording.
    pub fn ensure_loaded(&self) -> Result<(), String> {
        let id = self.default_model_id.lock().unwrap().clone();
        self.ensure_context_for(&id)
    }

    /// Loads (or reuses) a non-default model, e.g. a mode's override, so it
    /// can be warmed up before its first dictation too.
    pub fn ensure_loaded_model(&self, id: String, path: PathBuf) -> Result<(), String> {
        self.model_paths.lock().unwrap().insert(id.clone(), path);
        self.ensure_context_for(&id)
    }

    fn ensure_context_for(&self, id: &str) -> Result<(), String> {
        {
            let mut contexts = self.contexts.lock().unwrap();
            if let Some(loaded) = contexts.get_mut(id) {
                loaded.last_used = Instant::now();
                return Ok(());
            }
        }

        let path = self
            .model_paths
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| format!("no known path for model '{id}'"))?;
        let path_str = path.to_str().ok_or("model path is not valid UTF-8")?;
        let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| format!("failed to load whisper model at {path_str}: {e}"))?;

        let mut contexts = self.contexts.lock().unwrap();
        if contexts.len() >= MAX_WARM_CONTEXTS {
            if let Some(lru_id) = contexts
                .iter()
                .min_by_key(|(_, m)| m.last_used)
                .map(|(k, _)| k.clone())
            {
                contexts.remove(&lru_id);
            }
        }
        contexts.insert(
            id.to_string(),
            LoadedModel {
                context: ctx,
                last_used: Instant::now(),
            },
        );
        Ok(())
    }

    /// Transcribes with `model_override` (id + path) if given, else the
    /// global default. An override's path is remembered so the model stays
    /// resolvable by id alone afterward (e.g. if it gets pre-warmed later).
    #[cfg(test)]
    pub fn transcribe_with_model(
        &self,
        wav_path: &Path,
        model_override: Option<(String, PathBuf)>,
    ) -> Result<String, String> {
        let samples = load_samples_16k_mono(wav_path)?;
        self.transcribe_samples(&samples, model_override)
    }

    /// Same as `transcribe_with_model`, but takes already-loaded 16kHz mono
    /// samples directly — the hook point for pre-processing (e.g. isolate.rs
    /// masking out non-primary-speaker audio) between loading and inference.
    pub fn transcribe_samples(
        &self,
        samples: &[f32],
        model_override: Option<(String, PathBuf)>,
    ) -> Result<String, String> {
        self.transcribe_samples_with_progress(samples, model_override, &|_| {})
    }

    /// `transcribe_samples`, reporting progress (0-99, each value once)
    /// while it runs. Whisper only reports progress per 30s window, so a
    /// typical short dictation would jump from 0 to done; the estimate from
    /// this model's measured speed fills that in, and Whisper's real
    /// progress takes over whenever it's further along (long recordings).
    pub fn transcribe_samples_with_progress(
        &self,
        samples: &[f32],
        model_override: Option<(String, PathBuf)>,
        on_progress: &(dyn Fn(u8) + Sync),
    ) -> Result<String, String> {
        self.transcribe_samples_in_context(samples, model_override, "", on_progress)
    }

    /// `transcribe_samples_with_progress` for audio that continues earlier
    /// speech: `context` (the text already transcribed before these
    /// samples) goes into Whisper's prompt after the vocabulary, so a chunk
    /// cut at a pause keeps its casing, spelling and sentence flow.
    pub fn transcribe_samples_in_context(
        &self,
        samples: &[f32],
        model_override: Option<(String, PathBuf)>,
        context: &str,
        on_progress: &(dyn Fn(u8) + Sync),
    ) -> Result<String, String> {
        self.transcribe(samples, model_override, context, on_progress, false)
    }

    /// A quick, lower-fidelity pass for the live preview of a phrase that's
    /// still being spoken: Whisper's encoder is limited to the clip's own
    /// length (`audio_ctx`) instead of a full 30 s window, which makes a
    /// few-second clip several times faster. The final transcript never
    /// uses this.
    pub fn transcribe_preview(
        &self,
        samples: &[f32],
        model_override: Option<(String, PathBuf)>,
        context: &str,
    ) -> Result<String, String> {
        self.transcribe(samples, model_override, context, &|_| {}, true)
    }

    fn transcribe(
        &self,
        samples: &[f32],
        model_override: Option<(String, PathBuf)>,
        context: &str,
        on_progress: &(dyn Fn(u8) + Sync),
        quick: bool,
    ) -> Result<String, String> {
        let model_id = match model_override {
            Some((id, path)) => {
                self.model_paths.lock().unwrap().insert(id.clone(), path);
                id
            }
            None => self.default_model_id.lock().unwrap().clone(),
        };

        self.ensure_context_for(&model_id)?;

        if samples.is_empty() {
            return Err("no audio samples to transcribe".to_string());
        }

        // Project names and the user's vocabulary first, then the text
        // already transcribed. Whisper keeps the end of an over-long
        // prompt, so continuity wins over the lowest-ranked names.
        let terms = crate::names::whisper_prompt(
            &self.project_terms.lock().unwrap(),
            &self.vocabulary.lock().unwrap(),
        );
        let vocab_prompt = build_prompt(std::slice::from_ref(&terms), context);

        let contexts = self.contexts.lock().unwrap();
        let ctx = &contexts.get(&model_id).unwrap().context;

        let audio_s = samples.len() as f32 / 16_000.0;
        let speed = self
            .speed_ms_per_audio_s
            .lock()
            .unwrap()
            .get(&model_id)
            .copied()
            .unwrap_or(DEFAULT_SPEED_MS_PER_AUDIO_S);
        let expected_ms = TRANSCRIBE_OVERHEAD_MS + speed * audio_s;
        let started = Instant::now();

        let mut state = ctx.create_state().map_err(|e| e.to_string())?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if !vocab_prompt.is_empty() {
            params.set_initial_prompt(&vocab_prompt);
        }
        params.set_n_threads(num_cpus());
        if quick {
            params.set_audio_ctx(preview_audio_ctx(samples.len()));
        } else {
            WHISPER_PROGRESS.store(0, std::sync::atomic::Ordering::Relaxed);
            params.set_progress_callback_safe(|percent: i32| {
                WHISPER_PROGRESS.store(percent, std::sync::atomic::Ordering::Relaxed);
            });
        }

        let done = std::sync::atomic::AtomicBool::new(false);
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut last = None;
                while !done.load(std::sync::atomic::Ordering::Relaxed) {
                    let estimate =
                        estimated_percent(started.elapsed().as_millis() as f32, expected_ms);
                    let real = WHISPER_PROGRESS
                        .load(std::sync::atomic::Ordering::Relaxed)
                        .clamp(0, 99) as u8;
                    let percent = estimate.max(real);
                    if last != Some(percent) {
                        on_progress(percent);
                        last = Some(percent);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(60));
                }
            });
            let result = state.full(params, samples);
            done.store(true, std::sync::atomic::Ordering::Relaxed);
            result
        });
        result.map_err(|e| format!("whisper inference failed: {e}"))?;

        // Learn this model's speed on this Mac (short clips are dominated by
        // fixed overhead and would skew it; quick passes aren't comparable).
        if audio_s >= 1.0 && !quick {
            let measured =
                (started.elapsed().as_millis() as f32 - TRANSCRIBE_OVERHEAD_MS).max(0.0) / audio_s;
            let mut speeds = self.speed_ms_per_audio_s.lock().unwrap();
            let current = speeds.get(&model_id).copied();
            speeds.insert(
                model_id.clone(),
                current.map_or(measured, |c| c * 0.7 + measured * 0.3),
            );
        }

        let num_segments = state.full_n_segments().map_err(|e| e.to_string())?;
        let mut transcript = String::new();
        for i in 0..num_segments {
            transcript.push_str(&state.full_get_segment_text(i).map_err(|e| e.to_string())?);
        }

        Ok(transcript.trim().to_string())
    }
}

/// Encoder context for a clip of `samples` (16 kHz): Whisper's 1500 frames
/// cover 30 s, so 50 per second, plus a margin so the last word isn't cut.
/// Rounded up to a multiple of 64: whisper.cpp's Metal backend aborts the
/// process (`GGML_ASSERT(nb01 % 8 == 0)`) on other values.
pub fn preview_audio_ctx(samples: usize) -> i32 {
    let seconds = samples as f32 / WHISPER_SAMPLE_RATE as f32;
    let frames = (seconds * 50.0).ceil() as i32 + 64;
    (((frames + 63) / 64) * 64).clamp(128, 1472)
}

/// How much of the earlier transcript is fed back as prompt. Whisper keeps
/// only the tail of an over-long prompt anyway; this just bounds the work.
const PROMPT_CONTEXT_CHARS: usize = 400;

/// Whisper's initial prompt: the vocabulary list, then the tail of the
/// text transcribed so far (starting at a word boundary), so the audio
/// being decoded reads as a continuation.
pub(crate) fn build_prompt(vocabulary: &[String], context: &str) -> String {
    let vocab = vocabulary.join(", ");
    let context = context.trim();
    let tail = if context.chars().count() > PROMPT_CONTEXT_CHARS {
        let skip = context.chars().count() - PROMPT_CONTEXT_CHARS;
        let start = context.char_indices().nth(skip).map_or(0, |(i, _)| i);
        let cut = &context[start..];
        // Drop the partial word at the cut.
        cut.split_once(char::is_whitespace)
            .map_or(cut, |(_, rest)| rest)
    } else {
        context
    };
    match (vocab.is_empty(), tail.is_empty()) {
        (_, true) => vocab,
        (true, false) => tail.to_string(),
        (false, false) => format!("{vocab}. {tail}"),
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
}

/// Reads the mono/stereo 16-bit PCM wav written by `audio.rs`, downmixes to
/// mono, and resamples to the 16kHz whisper.cpp expects.
pub(crate) fn load_samples_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();

    let raw: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mono = downmix(&raw, spec.channels);
    Ok(resample_linear(
        &mono,
        spec.sample_rate,
        WHISPER_SAMPLE_RATE,
    ))
}

/// Reads just the wav header (no sample decoding) to get a recording's
/// length — used by `recording.rs` to compute words-per-minute for
/// Insights. `None` on a malformed/unreadable file, same tolerance as the
/// rest of the pipeline treats a bad wav (never worth failing the paste
/// over a stat).
pub(crate) fn wav_duration_ms(path: &Path) -> Option<u64> {
    let reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return None;
    }
    Some(reader.duration() as u64 * 1000 / spec.sample_rate as u64)
}

pub(crate) fn downmix(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let channels = channels as usize;
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Parenthesized descriptions Whisper uses for non-speech audio. Square
/// brackets are always annotations (Whisper never transcribes speech into
/// them), but parentheses can be real dictated text, so only these known
/// sound labels are dropped from them.
const PAREN_SOUND_LABELS: &[&str] = &[
    "music",
    "upbeat music",
    "laughs",
    "laughter",
    "laughing",
    "applause",
    "silence",
    "inaudible",
    "coughs",
    "coughing",
    "sighs",
    "typing",
    "clicking",
    "mouse clicking",
    "keyboard clicking",
    "background noise",
    "noise",
    "static",
    "breathing",
];

/// Removes Whisper's sound annotations (`[BLANK_AUDIO]`, `[click]`,
/// `(music)`, `*coughs*`) so they don't get pasted as if they were dictated
/// text. Returns an empty string when nothing but annotations (and stray
/// punctuation) was transcribed, which callers treat as "no speech".
pub fn strip_non_speech(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(['[', '(', '*']) {
        out.push_str(&rest[..i]);
        let open = rest.as_bytes()[i] as char;
        let close = match open {
            '[' => ']',
            '(' => ')',
            _ => '*',
        };
        let after_open = &rest[i + 1..];
        match after_open.find(close) {
            Some(j) => {
                let inner = &after_open[..j];
                let is_annotation = match open {
                    '[' => true,
                    '(' => PAREN_SOUND_LABELS.contains(&inner.trim().to_lowercase().as_str()),
                    _ => !inner.is_empty() && inner.len() <= 30 && !inner.contains('*'),
                };
                if is_annotation {
                    rest = &after_open[j + 1..];
                } else {
                    out.push(open);
                    rest = after_open;
                }
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);

    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().any(char::is_alphanumeric) {
        collapsed
    } else {
        String::new()
    }
}

pub(crate) fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (input.len() as f64 / ratio).round() as usize;
    let mut output = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        output.push(a + (b - a) * frac);
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn prompt_is_vocabulary_then_recent_context() {
        let vocab = vec!["kubectl".to_string(), "Tauri".to_string()];
        assert_eq!(build_prompt(&vocab, ""), "kubectl, Tauri");
        assert_eq!(
            build_prompt(&vocab, " First sentence. "),
            "kubectl, Tauri. First sentence."
        );
        assert_eq!(build_prompt(&[], "Hello there."), "Hello there.");
        assert_eq!(build_prompt(&[], ""), "");
    }

    #[test]
    fn prompt_context_is_trimmed_to_whole_words() {
        let long = "word ".repeat(200);
        let prompt = build_prompt(&[], &long);
        assert!(prompt.chars().count() <= PROMPT_CONTEXT_CHARS);
        assert!(prompt.split(' ').all(|w| w == "word"), "{prompt:?}");
    }

    #[test]
    fn progress_estimate_is_linear_then_eases_below_100() {
        assert_eq!(estimated_percent(0.0, 1000.0), 0);
        assert_eq!(estimated_percent(500.0, 1000.0), 50);
        assert_eq!(estimated_percent(890.0, 1000.0), 89);
        let late = estimated_percent(1500.0, 1000.0);
        assert!((95..=99).contains(&late), "{late}");
        assert_eq!(estimated_percent(100_000.0, 1000.0), 99);
        assert_eq!(estimated_percent(10.0, 0.0), 0);
    }
    use super::*;

    #[test]
    fn preview_audio_ctx_is_metal_safe_and_bounded() {
        for samples in [0, 1, 8_000, 16_000, 47_999, 160_000, 480_000, 2_000_000] {
            let ctx = preview_audio_ctx(samples);
            assert_eq!(ctx % 64, 0, "{samples} samples -> {ctx}");
            assert!((128..=1500).contains(&ctx));
            let needed = (samples as f32 / 16_000.0 * 50.0).ceil() as i32;
            assert!(
                ctx >= needed.min(1472),
                "{samples} samples -> {ctx} < {needed}"
            );
        }
    }
    use std::sync::Mutex;

    /// `transcribes_without_panicking` and
    /// `transcribe_with_model_override_switches_contexts` both load a real
    /// ggml model onto Metal; cargo test runs tests in parallel by
    /// default, and running both at once produced an observed
    /// "Failed to create a new whisper context" failure (transient
    /// Metal/GPU contention between the two independent loads, not a code
    /// bug — see BACKLOG.md). Serializing them via this lock fixes it.
    static WHISPER_CONTEXT_LOAD_LOCK: Mutex<()> = Mutex::new(());

    /// Smoke test: loads the real ggml model and runs inference on a
    /// synthetic tone, just to confirm whisper-rs is wired up correctly
    /// end-to-end. Skips if the (gitignored, locally-downloaded) model file
    /// isn't present, e.g. on a fresh clone that hasn't run
    /// scripts/download-model.sh yet.
    #[test]
    fn transcribes_without_panicking() {
        let model_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models/ggml-base.en-q5_1.bin");
        if !model_path.exists() {
            eprintln!("skipping: model not downloaded, run scripts/download-model.sh");
            return;
        }

        let _guard = WHISPER_CONTEXT_LOAD_LOCK.lock().unwrap();

        let wav_path = std::env::temp_dir().join("dev-whisper-smoke-test.wav");
        write_test_tone(&wav_path);

        let engine = WhisperEngine::new("base.en".to_string(), model_path);
        let result = engine.transcribe_with_model(&wav_path, None);

        std::fs::remove_file(&wav_path).ok();
        assert!(result.is_ok(), "transcription failed: {:?}", result.err());
    }

    /// Exercises the warm-context pool's model-switching path: transcribing
    /// with a `model_override` different from the engine's default should
    /// load and cache a second context under its own id, then a later call
    /// back on the default should still work off the original one. Reuses
    /// the single locally-downloaded model file under two different ids
    /// (rather than requiring a second real model in the repo) purely to
    /// exercise the HashMap-keyed load/lookup path in `ensure_context_for`
    /// and `transcribe_with_model`'s override handling.
    #[test]
    fn transcribe_with_model_override_switches_contexts() {
        let model_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models/ggml-base.en-q5_1.bin");
        if !model_path.exists() {
            eprintln!("skipping: model not downloaded, run scripts/download-model.sh");
            return;
        }

        let _guard = WHISPER_CONTEXT_LOAD_LOCK.lock().unwrap();

        let wav_path = std::env::temp_dir().join("dev-whisper-smoke-test-override.wav");
        write_test_tone(&wav_path);

        let engine = WhisperEngine::new("base.en".to_string(), model_path.clone());
        assert_eq!(engine.active_model_id(), "base.en");

        // Default model, no override.
        let default_result = engine.transcribe_with_model(&wav_path, None);
        assert!(
            default_result.is_ok(),
            "default transcription failed: {:?}",
            default_result.err()
        );

        // Override to a differently-keyed "model" (same underlying file) —
        // should load a second warm context rather than reusing the
        // default's, and still transcribe successfully.
        let override_result = engine
            .transcribe_with_model(&wav_path, Some(("base.en-alias".to_string(), model_path)));
        assert!(
            override_result.is_ok(),
            "override transcription failed: {:?}",
            override_result.err()
        );

        // Switching back to the default (no override) should still work
        // off the original warm context.
        let default_again = engine.transcribe_with_model(&wav_path, None);
        assert!(
            default_again.is_ok(),
            "post-override default transcription failed: {:?}",
            default_again.err()
        );
        assert_eq!(engine.active_model_id(), "base.en");

        std::fs::remove_file(&wav_path).ok();
    }

    #[test]
    fn resample_same_rate_is_a_noop() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        assert_eq!(resample_linear(&input, 16000, 16000), input);
    }

    #[test]
    fn resample_empty_input_stays_empty() {
        assert!(resample_linear(&[], 44100, 16000).is_empty());
    }

    #[test]
    fn resample_downsamples_to_expected_length() {
        // 1 second at 48kHz -> 1 second at 16kHz (3:1 ratio).
        let input = vec![0.0f32; 48000];
        let output = resample_linear(&input, 48000, 16000);
        assert_eq!(output.len(), 16000);
    }

    #[test]
    fn resample_upsamples_to_expected_length() {
        let input = vec![0.0f32; 16000];
        let output = resample_linear(&input, 16000, 48000);
        assert_eq!(output.len(), 48000);
    }

    #[test]
    fn resample_interpolates_between_samples() {
        // 2 samples at 2x the target rate -> roughly 1 sample, interpolated.
        let input = vec![0.0, 1.0];
        let output = resample_linear(&input, 2, 1);
        assert_eq!(output.len(), 1);
        assert!(output[0] >= 0.0 && output[0] <= 1.0);
    }

    #[test]
    fn downmix_mono_is_a_noop() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(downmix(&input, 1), input);
    }

    #[test]
    fn downmix_stereo_averages_channels() {
        // L, R, L, R
        let input = vec![1.0, 0.0, 0.5, 0.5];
        let output = downmix(&input, 2);
        assert_eq!(output, vec![0.5, 0.5]);
    }

    #[test]
    fn downmix_handles_odd_trailing_frame() {
        // A trailing partial frame (shouldn't happen with real wav data,
        // but shouldn't panic either).
        let input = vec![1.0, 0.0, 0.5];
        let output = downmix(&input, 2);
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn wav_duration_ms_reads_a_one_second_tone() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("dw-wav-duration-test-{}.wav", std::process::id()));
        write_test_tone(&path);
        let duration = wav_duration_ms(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(duration, 1000);
    }

    #[test]
    fn wav_duration_ms_none_for_missing_file() {
        assert!(wav_duration_ms(Path::new("/nonexistent/not-a-real.wav")).is_none());
    }

    fn write_test_tone(path: &Path) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: WHISPER_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..WHISPER_SAMPLE_RATE {
            let t = i as f32 / WHISPER_SAMPLE_RATE as f32;
            let sample = (t * 440.0 * std::f32::consts::TAU).sin() * 0.2;
            writer
                .write_sample((sample * i16::MAX as f32) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn annotation_only_transcripts_become_empty() {
        for raw in [
            "[BLANK_AUDIO]",
            "[click]",
            " [Music] ",
            "(music)",
            "*coughs*",
            "[click] .",
        ] {
            assert_eq!(strip_non_speech(raw), "", "{raw:?}");
        }
    }

    #[test]
    fn trailing_annotation_is_removed_from_real_speech() {
        assert_eq!(
            strip_non_speech(
                "Hello, comma, world period, New Line, this is a test. [mouse clicking]"
            ),
            "Hello, comma, world period, New Line, this is a test."
        );
    }

    #[test]
    fn real_parenthetical_dictation_is_kept() {
        assert_eq!(
            strip_non_speech("call foo (with the retry flag) now"),
            "call foo (with the retry flag) now"
        );
    }

    #[test]
    fn unmatched_brackets_are_left_alone() {
        assert_eq!(strip_non_speech("use the [ key"), "use the [ key");
        assert_eq!(strip_non_speech("2 * 3"), "2 * 3");
    }

    #[test]
    fn plain_speech_is_unchanged() {
        assert_eq!(
            strip_non_speech("just a normal sentence."),
            "just a normal sentence."
        );
    }
}
