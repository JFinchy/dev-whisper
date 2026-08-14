use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::AudioHandle;
use crate::paste::paste_text;
use crate::stt::WhisperEngine;

pub struct RecordingState {
    pub audio: AudioHandle,
    pub whisper: WhisperEngine,
    pub is_recording: AtomicBool,
}

/// Shared by the tray/UI toggle command and the global hotkey listener so
/// both drive the same start/stop lifecycle.
pub fn toggle_recording(app: &AppHandle) {
    let state = app.state::<RecordingState>();
    let was_recording = state.is_recording.fetch_xor(true, Ordering::SeqCst);

    if was_recording {
        match state.audio.stop() {
            Ok(path) => {
                let _ = app.emit("recording-stopped", path.to_string_lossy().to_string());
                let app = app.clone();
                std::thread::spawn(move || transcribe_and_paste(&app, &path));
            }
            Err(err) => {
                let _ = app.emit("recording-error", err);
            }
        }
    } else {
        state.audio.start();
        // The widget starts hidden; without this, triggering a recording via
        // the global hotkey gives no visual feedback that it's running.
        if let Some(window) = app.get_webview_window("widget") {
            let _ = window.show();
            let _ = window.set_focus();
        }
        let _ = app.emit("recording-started", ());
    }
}

fn transcribe_and_paste(app: &AppHandle, wav_path: &std::path::Path) {
    let state = app.state::<RecordingState>();
    match state.whisper.transcribe(wav_path) {
        Ok(text) if !text.is_empty() => match paste_text(&text) {
            Ok(()) => {
                let _ = app.emit("transcript-ready", text);
            }
            Err(err) => {
                let _ = app.emit("transcript-error", err);
            }
        },
        Ok(_) => {
            let _ = app.emit("transcript-error", "no speech detected".to_string());
        }
        Err(err) => {
            let _ = app.emit("transcript-error", err);
        }
    }
}

#[tauri::command]
pub fn toggle_recording_command(app: AppHandle) {
    toggle_recording(&app);
}

#[tauri::command]
pub fn list_input_devices() -> Vec<String> {
    crate::audio::list_device_names()
}

/// Whichever device recording will actually use next: the explicit
/// selection if one was made, otherwise whatever cpal resolves as default.
#[tauri::command]
pub fn get_active_input_device(state: tauri::State<RecordingState>) -> Option<String> {
    state
        .audio
        .selected_device()
        .or_else(crate::audio::default_device_name)
}

#[tauri::command]
pub fn set_input_device(app: AppHandle, name: Option<String>, state: tauri::State<RecordingState>) {
    state.audio.set_device(name.clone());

    let mut cfg = crate::config::load(&app);
    cfg.input_device = name;
    let _ = crate::config::save(&app, &cfg);
}
