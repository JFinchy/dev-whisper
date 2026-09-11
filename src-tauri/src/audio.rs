use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

enum AudioCommand {
    Start,
    Stop(Sender<Result<PathBuf, String>>),
}

/// Handle stored in Tauri state. The `cpal::Stream` itself is not `Send`/`Sync`
/// on every platform, so it lives on a dedicated thread and is driven by
/// commands sent over this channel instead.
pub struct AudioHandle {
    tx: Sender<AudioCommand>,
    /// `None` means "use the system default input device". Shared with the
    /// audio thread so a device picked in Settings takes effect on the next
    /// recording without needing a dedicated command/round-trip.
    selected_device: Arc<Mutex<Option<String>>>,
    /// Most recent input level (0.0-1.0, see `rms_level`), bit-packed since
    /// there's no stable `AtomicF32`. Updated on every audio callback,
    /// polled by `recording.rs`'s level-meter thread at a much lower,
    /// UI-appropriate rate — decoupled from the callback's own cadence
    /// rather than emitting an event per callback, which would be far more
    /// often than any redraw needs.
    level: Arc<AtomicU32>,
}

impl AudioHandle {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        let selected_device: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let selected_device_thread = selected_device.clone();
        let level: Arc<AtomicU32> = Arc::new(AtomicU32::new(0.0f32.to_bits()));
        let level_thread = level.clone();

        std::thread::spawn(move || {
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

            // The stream is built once and kept around *paused* between
            // recordings rather than torn down after every `Stop` — device
            // enumeration + format negotiation (the expensive part) happens
            // here, ahead of time, instead of on the hotkey's critical path.
            // `Start` then only needs `stream.play()`, which is fast, so
            // speech spoken in the first moments after the hotkey fires
            // isn't lost waiting for the driver to spin up from cold.
            // Rebuilt only when the selected device actually changes (see
            // `requested_device` below) or when reuse fails outright.
            let mut current: Option<WarmStream> = build_warm_stream(
                samples.clone(),
                level_thread.clone(),
                selected_device_thread.lock().unwrap().clone(),
            );

            for command in rx {
                match command {
                    AudioCommand::Start => {
                        samples.lock().unwrap().clear();
                        let device_name = selected_device_thread.lock().unwrap().clone();

                        let stale = current
                            .as_ref()
                            .map(|cs| cs.requested_device != device_name)
                            .unwrap_or(true);
                        if stale {
                            crate::applog!(
                                "audio: selected device changed (or no warm stream yet), rebuilding for {}",
                                device_name.as_deref().unwrap_or("<system default>")
                            );
                            current = build_warm_stream(samples.clone(), level_thread.clone(), device_name.clone());
                        }

                        let played = current.as_ref().and_then(|cs| cs.stream.play().ok());
                        if played.is_none() {
                            // Reused stream failed to resume (e.g. a
                            // Continuity/Bluetooth device dropped while
                            // idle) — rebuild fresh once and retry, same
                            // fallback the old per-recording path relied on.
                            crate::applog!("audio: resuming warm stream failed, rebuilding fresh");
                            current = build_warm_stream(samples.clone(), level_thread.clone(), device_name);
                            match current.as_ref().map(|cs| cs.stream.play()) {
                                Some(Ok(())) => crate::applog!("audio: stream playing (rebuilt)"),
                                Some(Err(err)) => crate::applog!("audio: stream.play() failed after rebuild: {err}"),
                                None => crate::applog!("audio: failed to start capture"),
                            }
                        } else {
                            crate::applog!("audio: stream playing (warm)");
                        }
                    }
                    AudioCommand::Stop(reply) => {
                        crate::applog!("audio: stop requested");
                        if let Some(cs) = &current {
                            if let Err(err) = cs.stream.pause() {
                                crate::applog!("audio: stream.pause() failed: {err}");
                            }
                        } else {
                            crate::applog!("audio: stop requested but no stream was active");
                        }
                        // Otherwise the widget's level meter would hold on
                        // whatever the last in-flight callback happened to
                        // report, instead of resting at zero once idle.
                        level_thread.store(0.0f32.to_bits(), Ordering::Relaxed);
                        let captured = std::mem::take(&mut *samples.lock().unwrap());
                        crate::applog!("audio: captured {} samples", captured.len());
                        let result = match &current {
                            Some(cs) if !captured.is_empty() => {
                                write_wav(&captured, cs.sample_rate, cs.channels)
                            }
                            _ => Err("no audio captured".to_string()),
                        };
                        let _ = reply.send(result);
                    }
                }
            }
        });

        Self {
            tx,
            selected_device,
            level,
        }
    }

    pub fn start(&self) {
        let _ = self.tx.send(AudioCommand::Start);
    }

    pub fn stop(&self) -> Result<PathBuf, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(AudioCommand::Stop(reply_tx))
            .map_err(|_| "audio thread unavailable".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread did not respond".to_string())?
    }

    /// `None` resets to the system default input device.
    pub fn set_device(&self, name: Option<String>) {
        *self.selected_device.lock().unwrap() = name;
    }

    /// The explicitly selected device, if any (does not resolve the default).
    pub fn selected_device(&self) -> Option<String> {
        self.selected_device.lock().unwrap().clone()
    }

    /// Most recent input level, 0.0-1.0 — 0.0 at rest (not recording, or no
    /// callback has landed yet). Polled by `recording.rs`'s level-meter
    /// thread; not itself pushed anywhere, so reading it costs nothing when
    /// nobody's watching.
    pub fn current_level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

/// A built (but not necessarily playing) input stream, kept alive between
/// recordings so `Start` only has to call `play()` instead of paying the
/// full device-open cost from cold. `requested_device` is whatever the
/// caller asked for (`None` = system default) — compared against the
/// currently-selected device on each `Start` to know whether this stream
/// is still valid or needs rebuilding for a newly-picked device.
struct WarmStream {
    stream: cpal::Stream,
    requested_device: Option<String>,
    sample_rate: u32,
    channels: u16,
}

/// Builds a stream for `device_name` (logging the resolved device/rate/
/// channels), leaving it paused/not-yet-playing. `None` on failure — the
/// caller falls back to treating capture as unavailable, same as before.
fn build_warm_stream(
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    device_name: Option<String>,
) -> Option<WarmStream> {
    match open_with_retry_and_fallback(samples, level, device_name.as_deref()) {
        Ok((stream, rate, chans, resolved_name)) => {
            crate::applog!("audio: opened device '{resolved_name}' at {rate}Hz, {chans}ch");
            Some(WarmStream {
                stream,
                requested_device: device_name,
                sample_rate: rate,
                channels: chans,
            })
        }
        Err(err) => {
            crate::applog!("audio: failed to open device: {err}");
            None
        }
    }
}

/// Names of all available input devices, for the settings picker.
pub fn list_device_names() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// The name of whichever device cpal would pick with no explicit selection.
pub fn default_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
}

/// Continuity/Bluetooth input devices (e.g. "iPhone Microphone") can report
/// a stale handle right after being selected — the underlying CoreAudio
/// object gets renegotiated and a snapshot `Device` from a moment earlier
/// no longer resolves. Retrying with a fresh enumeration usually recovers
/// within a second; if it still won't open, fall back to the system default
/// input rather than silently capturing nothing.
fn open_with_retry_and_fallback(
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    device_name: Option<&str>,
) -> Result<(cpal::Stream, u32, u16, String), String> {
    const RETRIES: u32 = 3;
    let mut last_err = String::new();

    for attempt in 0..RETRIES {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(300 * attempt as u64);
            crate::applog!("audio: retrying '{}' in {delay:?} (attempt {})", device_name.unwrap_or("<default>"), attempt + 1);
            std::thread::sleep(delay);
        }
        match build_input_stream(samples.clone(), level.clone(), device_name) {
            Ok(ok) => return Ok(ok),
            Err(err) => {
                crate::applog!("audio: attempt {} to open '{}' failed: {err}", attempt + 1, device_name.unwrap_or("<default>"));
                last_err = err;
            }
        }
    }

    if device_name.is_some() {
        crate::applog!("audio: '{}' unavailable after {RETRIES} attempts ({last_err}), falling back to system default", device_name.unwrap());
        return build_input_stream(samples, level, None);
    }

    Err(last_err)
}

/// Root-mean-square level of a chunk, scaled into a visually useful
/// 0.0-1.0 range for the widget's live level meter. A straight linear gain
/// (tried at 4x, then 8x) couldn't satisfy both ends at once: high enough
/// to make normal speech clearly move the meter meant loud speech
/// overshot 1.0 almost immediately, low enough to keep loud speech from
/// clipping meant normal speech barely registered above the floor. A
/// sqrt-compressed (perceptual) curve fixes that — it boosts quiet/
/// moderate input much more than loud input, so normal speech already
/// shows clear, obvious movement, while genuinely loud speech still
/// saturates to a full "every bar lit" 1.0 rather than needing to be
/// shouted. Still a rough tuning (15x gain before the compression), not a
/// calibrated measurement.
fn rms_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum_sq / samples.len() as f32).sqrt();
    (rms * 15.0).clamp(0.0, 1.0).sqrt()
}

fn build_input_stream(
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    device_name: Option<&str>,
) -> Result<(cpal::Stream, u32, u16, String), String> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| format!("input device '{name}' not found"))?,
        None => host
            .default_input_device()
            .ok_or_else(|| "no input device available".to_string())?,
    };
    let resolved_name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
    let config = device
        .default_input_config()
        .map_err(|e| e.to_string())?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();
    let stream_config: StreamConfig = config.into();

    let err_fn = |err| crate::applog!("audio stream error: {err}");

    // Logs once per stream, on whichever thread's callback happens to
    // deliver the first buffer — used to measure the gap between
    // stream.play() and real audio actually flowing (Continuity/Bluetooth
    // devices can have multi-second connection latency here).
    let first_callback = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let stream = match sample_format {
        SampleFormat::F32 => {
            let first_callback = first_callback.clone();
            let level = level.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    if !first_callback.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        crate::applog!("audio: first callback received, {} frames", data.len());
                    }
                    level.store(rms_level(data).to_bits(), Ordering::Relaxed);
                    samples.lock().unwrap().extend_from_slice(data);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let first_callback = first_callback.clone();
            let level = level.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    if !first_callback.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        crate::applog!("audio: first callback received, {} frames", data.len());
                    }
                    let converted: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                    level.store(rms_level(&converted).to_bits(), Ordering::Relaxed);
                    samples.lock().unwrap().extend_from_slice(&converted);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let first_callback = first_callback.clone();
            let level = level.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| {
                    if !first_callback.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        crate::applog!("audio: first callback received, {} frames", data.len());
                    }
                    let converted: Vec<f32> =
                        data.iter().map(|s| (*s as f32 / u16::MAX as f32) * 2.0 - 1.0).collect();
                    level.store(rms_level(&converted).to_bits(), Ordering::Relaxed);
                    samples.lock().unwrap().extend_from_slice(&converted);
                },
                err_fn,
                None,
            )
        }
        other => return Err(format!("unsupported sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    Ok((stream, sample_rate, channels, resolved_name))
}

fn write_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let path = std::env::temp_dir().join(format!("dev-whisper-{timestamp}.wav"));

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&path, spec).map_err(|e| e.to_string())?;
    for &sample in samples {
        let clamped = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(clamped).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_zero() {
        assert_eq!(rms_level(&[0.0; 100]), 0.0);
    }

    #[test]
    fn empty_slice_is_zero() {
        assert_eq!(rms_level(&[]), 0.0);
    }

    #[test]
    fn a_quiet_tone_still_produces_a_clearly_visible_level() {
        // The whole point of the sqrt compression: even fairly quiet input
        // should already read as a clear, visible movement on the meter,
        // not a barely-there flicker near the floor.
        let quiet = vec![0.02; 100];
        let level = rms_level(&quiet);
        assert!(level > 0.4 && level < 1.0, "expected a clearly visible but unsaturated level, got {level}");
    }

    #[test]
    fn a_loud_tone_clamps_to_one() {
        let loud = vec![0.9; 100];
        assert_eq!(rms_level(&loud), 1.0);
    }

    #[test]
    fn louder_input_produces_a_higher_level() {
        let quiet = vec![0.02; 100];
        let louder = vec![0.1; 100];
        assert!(rms_level(&louder) > rms_level(&quiet));
    }
}
