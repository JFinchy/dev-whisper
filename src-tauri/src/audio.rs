use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::path::PathBuf;
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
}

impl AudioHandle {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        let selected_device: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let selected_device_thread = selected_device.clone();

        std::thread::spawn(move || {
            let mut active_stream: Option<cpal::Stream> = None;
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
            let mut sample_rate = 0u32;
            let mut channels = 0u16;

            for command in rx {
                match command {
                    AudioCommand::Start => {
                        if active_stream.is_some() {
                            eprintln!("audio: start requested but a stream is already active, ignoring");
                            continue;
                        }
                        samples.lock().unwrap().clear();
                        let device_name = selected_device_thread.lock().unwrap().clone();
                        eprintln!(
                            "audio: starting capture, requested device = {}",
                            device_name.as_deref().unwrap_or("<system default>")
                        );
                        match open_with_retry_and_fallback(samples.clone(), device_name.as_deref()) {
                            Ok((stream, rate, chans, resolved_name)) => {
                                sample_rate = rate;
                                channels = chans;
                                eprintln!(
                                    "audio: opened device '{resolved_name}' at {rate}Hz, {chans}ch"
                                );
                                match stream.play() {
                                    Ok(()) => {
                                        eprintln!("audio: stream playing");
                                        active_stream = Some(stream);
                                    }
                                    Err(err) => {
                                        eprintln!("audio: stream.play() failed: {err}");
                                    }
                                }
                            }
                            Err(err) => {
                                eprintln!("audio: failed to start capture: {err}");
                            }
                        }
                    }
                    AudioCommand::Stop(reply) => {
                        eprintln!("audio: stop requested");
                        if let Some(stream) = active_stream.take() {
                            if let Err(err) = stream.pause() {
                                eprintln!("audio: stream.pause() before drop failed: {err}");
                            }
                            drop(stream);
                            eprintln!("audio: stream paused and dropped");
                        } else {
                            eprintln!("audio: stop requested but no stream was active");
                        }
                        let captured = std::mem::take(&mut *samples.lock().unwrap());
                        eprintln!("audio: captured {} samples", captured.len());
                        let result = if captured.is_empty() {
                            Err("no audio captured".to_string())
                        } else {
                            write_wav(&captured, sample_rate, channels)
                        };
                        let _ = reply.send(result);
                    }
                }
            }
        });

        Self {
            tx,
            selected_device,
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
    device_name: Option<&str>,
) -> Result<(cpal::Stream, u32, u16, String), String> {
    const RETRIES: u32 = 3;
    let mut last_err = String::new();

    for attempt in 0..RETRIES {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(300 * attempt as u64);
            eprintln!("audio: retrying '{}' in {delay:?} (attempt {})", device_name.unwrap_or("<default>"), attempt + 1);
            std::thread::sleep(delay);
        }
        match build_input_stream(samples.clone(), device_name) {
            Ok(ok) => return Ok(ok),
            Err(err) => {
                eprintln!("audio: attempt {} to open '{}' failed: {err}", attempt + 1, device_name.unwrap_or("<default>"));
                last_err = err;
            }
        }
    }

    if device_name.is_some() {
        eprintln!("audio: '{}' unavailable after {RETRIES} attempts ({last_err}), falling back to system default", device_name.unwrap());
        return build_input_stream(samples, None);
    }

    Err(last_err)
}

fn build_input_stream(
    samples: Arc<Mutex<Vec<f32>>>,
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

    let err_fn = |err| eprintln!("audio stream error: {err}");

    // Logs once per stream, on whichever thread's callback happens to
    // deliver the first buffer — used to measure the gap between
    // stream.play() and real audio actually flowing (Continuity/Bluetooth
    // devices can have multi-second connection latency here).
    let first_callback = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let stream = match sample_format {
        SampleFormat::F32 => {
            let first_callback = first_callback.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    if !first_callback.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        eprintln!("audio: first callback received, {} frames", data.len());
                    }
                    samples.lock().unwrap().extend_from_slice(data);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let first_callback = first_callback.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    if !first_callback.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        eprintln!("audio: first callback received, {} frames", data.len());
                    }
                    let mut buf = samples.lock().unwrap();
                    buf.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let first_callback = first_callback.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| {
                    if !first_callback.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        eprintln!("audio: first callback received, {} frames", data.len());
                    }
                    let mut buf = samples.lock().unwrap();
                    buf.extend(
                        data.iter()
                            .map(|s| (*s as f32 / u16::MAX as f32) * 2.0 - 1.0),
                    );
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
