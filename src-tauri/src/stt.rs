use std::path::{Path, PathBuf};
use std::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Seeds the model with developer jargon so it's biased toward recognizing
/// these terms instead of mishearing them as similar-sounding words.
const DEV_VOCAB_PROMPT: &str = "kubectl, JSON, macOS, Hammerspoon, camelCase, snake_case, \
React, TypeScript, npm, git, Docker, API, async, useState, useEffect, Tauri, Rust, \
Whisper, CLI, terminal, iTerm, Neovim, init.lua, zshrc, MCP";

pub struct WhisperEngine {
    model_path: PathBuf,
    context: Mutex<Option<WhisperContext>>,
}

impl WhisperEngine {
    pub fn new(model_path: PathBuf) -> Self {
        Self {
            model_path,
            context: Mutex::new(None),
        }
    }

    pub fn transcribe(&self, wav_path: &Path) -> Result<String, String> {
        let mut guard = self.context.lock().unwrap();
        if guard.is_none() {
            let path_str = self
                .model_path
                .to_str()
                .ok_or("model path is not valid UTF-8")?;
            let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
                .map_err(|e| format!("failed to load whisper model at {path_str}: {e}"))?;
            *guard = Some(ctx);
        }
        let ctx = guard.as_ref().unwrap();

        let samples = load_samples_16k_mono(wav_path)?;
        if samples.is_empty() {
            return Err("no audio samples to transcribe".to_string());
        }

        let mut state = ctx.create_state().map_err(|e| e.to_string())?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_initial_prompt(DEV_VOCAB_PROMPT);
        params.set_n_threads(num_cpus());

        state
            .full(params, &samples)
            .map_err(|e| format!("whisper inference failed: {e}"))?;

        let num_segments = state.full_n_segments().map_err(|e| e.to_string())?;
        let mut transcript = String::new();
        for i in 0..num_segments {
            transcript.push_str(&state.full_get_segment_text(i).map_err(|e| e.to_string())?);
        }

        Ok(transcript.trim().to_string())
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
}

/// Reads the mono/stereo 16-bit PCM wav written by `audio.rs`, downmixes to
/// mono, and resamples to the 16kHz whisper.cpp expects.
fn load_samples_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();

    let raw: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mono = downmix(&raw, spec.channels);
    Ok(resample_linear(&mono, spec.sample_rate, WHISPER_SAMPLE_RATE))
}

fn downmix(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let channels = channels as usize;
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: loads the real ggml model and runs inference on a
    /// synthetic tone, just to confirm whisper-rs is wired up correctly
    /// end-to-end. Skips if the (gitignored, locally-downloaded) model file
    /// isn't present, e.g. on a fresh clone that hasn't run
    /// scripts/download-model.sh yet.
    #[test]
    fn transcribes_without_panicking() {
        let model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("models/ggml-base.en-q5_1.bin");
        if !model_path.exists() {
            eprintln!("skipping: model not downloaded, run scripts/download-model.sh");
            return;
        }

        let wav_path = std::env::temp_dir().join("dev-whisper-smoke-test.wav");
        write_test_tone(&wav_path);

        let engine = WhisperEngine::new(model_path);
        let result = engine.transcribe(&wav_path);

        std::fs::remove_file(&wav_path).ok();
        assert!(result.is_ok(), "transcription failed: {:?}", result.err());
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
}

fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
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
