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

const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Default developer-jargon vocabulary, used to seed a fresh config (and as
/// a fallback if the persisted list is ever empty). Biases Whisper toward
/// recognizing these terms instead of mishearing them as similar-sounding
/// words. User-editable via Settings — see `config::AppConfig::vocabulary`.
pub fn default_vocabulary() -> Vec<String> {
    [
        "kubectl", "JSON", "macOS", "Hammerspoon", "camelCase", "snake_case", "React",
        "TypeScript", "npm", "git", "Docker", "API", "async", "useState", "useEffect", "Tauri",
        "Rust", "Whisper", "CLI", "terminal", "iTerm", "Neovim", "init.lua", "zshrc", "MCP",
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
    contexts: Mutex<HashMap<String, LoadedModel>>,
}

impl WhisperEngine {
    pub fn new(default_model_id: String, default_model_path: PathBuf) -> Self {
        let mut model_paths = HashMap::new();
        model_paths.insert(default_model_id.clone(), default_model_path);
        Self {
            default_model_id: Mutex::new(default_model_id),
            model_paths: Mutex::new(model_paths),
            vocabulary: Mutex::new(default_vocabulary()),
            contexts: Mutex::new(HashMap::new()),
        }
    }

    pub fn active_model_id(&self) -> String {
        self.default_model_id.lock().unwrap().clone()
    }

    pub fn set_vocabulary(&self, terms: Vec<String>) {
        *self.vocabulary.lock().unwrap() = terms;
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

        let vocab_prompt = self.vocabulary.lock().unwrap().join(", ");

        let contexts = self.contexts.lock().unwrap();
        let ctx = &contexts.get(&model_id).unwrap().context;

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

        state
            .full(params, samples)
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
pub(crate) fn load_samples_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
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
        let model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("models/ggml-base.en-q5_1.bin");
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
        let model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("models/ggml-base.en-q5_1.bin");
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
        let override_result = engine.transcribe_with_model(
            &wav_path,
            Some(("base.en-alias".to_string(), model_path)),
        );
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
