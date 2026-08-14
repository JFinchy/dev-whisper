mod audio;
mod paste;
mod recording;
mod stt;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use audio::AudioHandle;
use recording::{toggle_recording, toggle_recording_command, RecordingState};
use stt::WhisperEngine;

fn model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models/ggml-base.en-q5_1.bin")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let push_to_talk = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if *shortcut == push_to_talk && event.state == ShortcutState::Pressed {
                        toggle_recording(app);
                    }
                })
                .build(),
        )
        .manage(RecordingState {
            audio: AudioHandle::spawn(),
            whisper: WhisperEngine::new(model_path()),
            is_recording: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![toggle_recording_command])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.global_shortcut().register(push_to_talk)?;

            let toggle_widget = MenuItem::with_id(app, "toggle_widget", "Show/Hide Widget", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle_widget, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle_widget" => {
                        if let Some(window) = app.get_webview_window("widget") {
                            let visible = window.is_visible().unwrap_or(false);
                            if visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
