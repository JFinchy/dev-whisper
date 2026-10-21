//! End-to-end dictation tests with real audio: macOS `say` speaks each
//! phrase in `tests/fixtures/audio_cases.json` to a 16kHz WAV, the real
//! Whisper model transcribes it, and the real pipeline formats it, exactly
//! as a push-to-talk recording would go (minus the mic and the paste).
//!
//! Ignored by default (needs macOS `say` and the downloaded model, and
//! takes ~30s). Run with `bun run test:audio`, which also prints a word
//! error rate and latency report usable for comparing models.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::config::AppConfig;
use crate::modes::{Behavior, LlmRefinement, ResolvedSettings};
use crate::pipeline::{self, Refiner};
use crate::stt::WhisperEngine;

#[derive(serde::Deserialize)]
struct AudioCase {
    name: String,
    say: String,
    mode: String,
    expect: String,
    /// Allowed word error rate against `expect` (case and punctuation
    /// ignored). 0 means the words must match exactly.
    #[serde(default)]
    max_wer: f64,
    #[serde(default)]
    press_enter_enabled: bool,
    expect_press_enter: Option<bool>,
    /// Why a tolerance is set, when it isn't 0. Documentation only.
    #[serde(default)]
    #[allow(dead_code)]
    note: Option<String>,
}

struct NoLlm;

impl Refiner for NoLlm {
    fn refine(
        &self,
        _: Behavior,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &[String],
    ) -> Result<String, String> {
        Err("LLM disabled in audio e2e".into())
    }
    fn boilerplate(&self, _: &str, _: &str) -> Result<String, String> {
        Err("LLM disabled in audio e2e".into())
    }
}

/// Lowercased words with surrounding punctuation stripped, keeping the
/// characters that carry meaning inside developer tokens (`_`, `@`, `.`).
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '@')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word error rate: word-level edit distance over the reference length.
fn wer(reference: &str, hypothesis: &str) -> f64 {
    let (r, h) = (words(reference), words(hypothesis));
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for i in 1..=r.len() {
        let mut cur = vec![i; h.len() + 1];
        for j in 1..=h.len() {
            let substitution = prev[j - 1] + usize::from(r[i - 1] != h[j - 1]);
            cur[j] = substitution.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        prev = cur;
    }
    prev[h.len()] as f64 / r.len() as f64
}

fn speak(text: &str, out: &Path) -> Result<(), String> {
    let status = Command::new("say")
        .args(["-v", "Samantha", "--data-format=LEI16@16000", "-o"])
        .arg(out)
        .arg(text)
        .status()
        .map_err(|e| format!("couldn't run `say`: {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("`say` exited with {status}"))
}

fn model_path() -> PathBuf {
    std::env::var_os("DW_E2E_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models/ggml-base.en-q5_1.bin")
        })
}

#[test]
#[ignore = "needs macOS `say` and a Whisper model; run with `bun run test:audio`"]
fn dictation_from_real_audio() {
    let model = model_path();
    assert!(
        model.exists(),
        "no Whisper model at {} (run ./scripts/download-model.sh)",
        model.display()
    );
    let cases: Vec<AudioCase> =
        serde_json::from_str(include_str!("../tests/fixtures/audio_cases.json")).unwrap();

    let dir = std::env::temp_dir().join(format!("dw-audio-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let engine = WhisperEngine::new("e2e".into(), model.clone());
    engine.ensure_loaded().expect("model failed to load");

    let mut failures = Vec::new();
    let mut total_wer = 0.0;
    let mut stt_ms_total = 0u128;
    println!("\n{:<28} {:>6} {:>7}  output", "case", "WER", "stt ms");
    for case in &cases {
        let wav = dir.join(format!("{}.wav", case.name.replace(' ', "_")));
        speak(&case.say, &wav).unwrap();

        let started = Instant::now();
        let samples = crate::stt::load_samples_16k_mono(&wav).unwrap();
        let ticks = std::sync::Mutex::new(Vec::new());
        let raw = engine
            .transcribe_samples_with_progress(&samples, None, &|p| ticks.lock().unwrap().push(p))
            .unwrap();
        let ticks = ticks.into_inner().unwrap();
        assert!(
            ticks.windows(2).all(|w| w[0] < w[1]) && ticks.last().is_some_and(|&p| p < 100),
            "progress must rise monotonically and stay below 100 until done: {ticks:?}"
        );
        let stt_ms = started.elapsed().as_millis();
        stt_ms_total += stt_ms;

        let cleaned = crate::stt::strip_non_speech(&raw);
        let cfg = AppConfig {
            press_enter_enabled: case.press_enter_enabled,
            ..AppConfig::default()
        };
        let settings = ResolvedSettings {
            mode: match case.mode.as_str() {
                "cli" => Behavior::Cli,
                "casual" => Behavior::Casual,
                _ => Behavior::Plain,
            },
            stt_model: None,
            llm_refinement: LlmRefinement::Off,
            custom_instructions: None,
        };
        let out = pipeline::format(cleaned, &cfg, &settings, &NoLlm, &mut |_| {});

        let error_rate = wer(&case.expect, &out.text);
        total_wer += error_rate;
        println!(
            "{:<28} {:>6.3} {:>7}  {:?}  ({} progress updates, last {}%)",
            case.name,
            error_rate,
            stt_ms,
            out.text,
            ticks.len(),
            ticks.last().copied().unwrap_or(0)
        );

        let mut problems = Vec::new();
        if error_rate > case.max_wer {
            problems.push(format!(
                "WER {error_rate:.3} > {} (whisper heard {raw:?}, delivered {:?}, expected {:?})",
                case.max_wer, out.text, case.expect
            ));
        }
        if case.max_wer == 0.0 && out.text.contains('\n') != case.expect.contains('\n') {
            problems.push(format!(
                "line structure differs: {:?} vs {:?}",
                out.text, case.expect
            ));
        }
        if let Some(enter) = case.expect_press_enter {
            if out.press_enter != enter {
                problems.push(format!("press_enter {} expected {enter}", out.press_enter));
            }
        }
        if !problems.is_empty() {
            failures.push(format!("{}: {}", case.name, problems.join("; ")));
        }
    }
    println!(
        "\nmodel {}: mean WER {:.3}, mean stt {}ms over {} cases\n",
        model.file_name().unwrap().to_string_lossy(),
        total_wer / cases.len() as f64,
        stt_ms_total / cases.len() as u128,
        cases.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} case(s) failed:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Live chunking against real speech: three phrases separated by long
/// pauses go through the real Silero VAD and the chunker exactly as a
/// push-to-talk recording would, are transcribed chunk by chunk with the
/// previous text as context, and the joined result is compared with
/// transcribing the whole buffer. Prints both WERs and the work left at
/// stop.
#[test]
#[ignore = "needs macOS `say` and a Whisper model; run with `bun run test:audio`"]
fn chunked_dictation_with_long_pauses_matches_whole_buffer() {
    let model = model_path();
    assert!(model.exists(), "no Whisper model at {}", model.display());
    let vad_model = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/silero_vad.onnx");
    let phrases = [
        "I want to refactor the authentication middleware so that it validates tokens before hitting the database.",
        "Then run the migration script and check that the users table has the new column.",
        "Finally open a pull request and ask Alice to review it.",
    ];
    let pauses = [6.0f32, 20.0];

    let dir = std::env::temp_dir().join(format!("dw-audio-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut recording: Vec<f32> = Vec::new();
    for (i, phrase) in phrases.iter().enumerate() {
        let wav = dir.join(format!("phrase{i}.wav"));
        speak(phrase, &wav).unwrap();
        recording.extend(crate::stt::load_samples_16k_mono(&wav).unwrap());
        if let Some(pause) = pauses.get(i) {
            recording.extend(std::iter::repeat_n(0.0f32, (pause * 16_000.0) as usize));
        }
    }
    let expected = phrases.join(" ");
    let audio_s = recording.len() as f32 / 16_000.0;

    let engine = WhisperEngine::new("e2e".into(), model);
    engine.ensure_loaded().expect("model failed to load");

    let started = Instant::now();
    let whole = engine.transcribe_samples(&recording, None).unwrap();
    let whole_ms = started.elapsed().as_millis();

    let vad = crate::live::vad_from_model(&vad_model).expect("VAD failed to load");
    let mut is_speech = |window: &[f32]| {
        vad.accept_waveform(window);
        let speech = vad.detected();
        vad.clear();
        speech
    };
    let mut segmenter = crate::live::Segmenter::new();
    let mut texts: Vec<String> = Vec::new();
    for piece in recording.chunks(1600) {
        for chunk in segmenter.push(piece, &mut is_speech) {
            let context = crate::live::join_texts(&texts);
            let text = engine
                .transcribe_samples_in_context(&chunk, None, &context, &|_| {})
                .unwrap();
            texts.push(crate::stt::strip_non_speech(&text));
        }
    }
    let chunks = texts.len();
    let tail = segmenter.finish();
    let tail_started = Instant::now();
    if let Some(tail) = &tail {
        let context = crate::live::join_texts(&texts);
        let text = engine
            .transcribe_samples_in_context(tail, None, &context, &|_| {})
            .unwrap();
        texts.push(crate::stt::strip_non_speech(&text));
    }
    let tail_ms = tail_started.elapsed().as_millis();
    let chunked = crate::live::join_texts(&texts);
    let kept_s = segmenter.emitted_samples() as f32 / 16_000.0;

    let whole_wer = wer(&expected, &crate::stt::strip_non_speech(&whole));
    let chunked_wer = wer(&expected, &chunked);
    println!(
        "\nlive chunking: {audio_s:.1}s recording, {chunks} chunks + tail, {kept_s:.1}s transcribed\n  whole buffer: WER {whole_wer:.3}, {whole_ms}ms to text after stop\n  chunked:      WER {chunked_wer:.3}, {tail_ms}ms to text after stop\n  whole   {whole:?}\n  chunked {chunked:?}\n"
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert!(chunks >= 1, "expected the long pauses to cut chunks");
    assert!(
        kept_s < audio_s * 0.6,
        "silence should have been skipped ({kept_s:.1}s of {audio_s:.1}s)"
    );
    assert!(
        chunked_wer <= whole_wer + 0.05,
        "chunked WER {chunked_wer:.3} is worse than whole-buffer {whole_wer:.3}"
    );
    assert!(chunked_wer <= 0.2, "chunked WER {chunked_wer:.3}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wer_ignores_case_and_punctuation() {
        assert_eq!(wer("Hello, world.", "hello world"), 0.0);
        assert_eq!(wer("a b c d", "a x c d"), 0.25);
        assert_eq!(wer("a b", ""), 1.0);
        assert_eq!(
            wer("look at @recording.rs", "look at recording.rs"),
            1.0 / 3.0
        );
    }
}
