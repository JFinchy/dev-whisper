mod app_detect;
mod audio;
mod config;
mod models;
mod modes;
mod paste;
mod recording;
mod shortcut;
mod stt;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState as PressState};

use audio::AudioHandle;
use models::{download_model, list_models, set_active_model};
use modes::{get_last_frontmost_app, get_mode_rules, remove_mode_rule, set_mode_rule, ModesState};
use recording::{
    get_active_input_device, list_input_devices, set_input_device, toggle_recording,
    toggle_recording_command, RecordingState,
};
use shortcut::{get_shortcut, set_shortcut, PushToTalkState};
use stt::WhisperEngine;

#[tauri::command]
fn open_settings(app: tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }

    // Capture whatever app the user was in before focus shifts to Settings,
    // so the UI can offer "add a mode rule for the app you just came from".
    let capture_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let info = app_detect::frontmost_app_info();
        let state = capture_app.state::<ModesState>();
        *state.last_frontmost.lock().unwrap() = info;
    });

    let mut builder = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Dev Whisper Settings")
    .inner_size(360.0, 540.0)
    .resizable(false);

    // Anchor settings just below the widget instead of both windows
    // centering on top of each other.
    if let Some(widget) = app.get_webview_window("widget") {
        if let (Ok(pos), Ok(size), Ok(scale)) = (
            widget.outer_position(),
            widget.outer_size(),
            widget.scale_factor(),
        ) {
            let logical_pos = pos.to_logical::<f64>(scale);
            let logical_size = size.to_logical::<f64>(scale);
            let gap = 12.0;
            builder = builder.position(logical_pos.x, logical_pos.y + logical_size.height + gap);
        } else {
            builder = builder.center();
        }
    } else {
        builder = builder.center();
    }

    builder.build()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn prompt_for_accessibility_permission() {
    // Shows the system permission dialog if not already granted. Spawned so
    // the (potentially blocking) prompt doesn't delay startup.
    std::thread::spawn(|| {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt();
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, triggered, event| {
                    let state = app.state::<PushToTalkState>();
                    let cfg = state.current.lock().unwrap().clone();
                    if let Ok(expected) = shortcut::to_shortcut(&cfg) {
                        if *triggered == expected && event.state == PressState::Pressed {
                            toggle_recording(app);
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            toggle_recording_command,
            open_settings,
            list_input_devices,
            get_active_input_device,
            set_input_device,
            get_shortcut,
            set_shortcut,
            list_models,
            download_model,
            set_active_model,
            get_mode_rules,
            set_mode_rule,
            remove_mode_rule,
            get_last_frontmost_app,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                prompt_for_accessibility_permission();
            }

            app.manage(ModesState {
                last_frontmost: Mutex::new(None),
            });

            let saved_config = config::load(app.handle());

            let shortcut_cfg = saved_config.shortcut.clone().unwrap_or_default();
            let initial_shortcut = shortcut::to_shortcut(&shortcut_cfg)?;
            app.global_shortcut().register(initial_shortcut)?;
            app.manage(PushToTalkState {
                current: Mutex::new(shortcut_cfg),
            });

            let audio = AudioHandle::spawn();
            audio.set_device(saved_config.input_device.clone());

            let model_id = saved_config
                .active_model
                .clone()
                .unwrap_or_else(|| models::default_model_id().to_string());
            let model_path = models::resolve_model_path(app.handle(), &model_id)
                .or_else(|| models::model_path(app.handle(), &model_id).ok())
                .unwrap_or_default();
            let whisper = WhisperEngine::new(model_id, model_path);

            app.manage(RecordingState {
                audio,
                whisper,
                is_recording: AtomicBool::new(false),
                active_app: Mutex::new(None),
            });

            // Warm up the model in the background so the first real
            // recording doesn't pay for Metal shader compilation, which can
            // take several seconds and otherwise happens on the user's
            // first "transcribing…".
            let warm_app = app.handle().clone();
            std::thread::spawn(move || {
                let state = warm_app.state::<RecordingState>();
                if let Err(err) = state.whisper.ensure_loaded() {
                    eprintln!("model warm-up skipped: {err}");
                }
            });

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
