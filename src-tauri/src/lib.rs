mod app_detect;
mod audio;
mod config;
mod history;
mod llm;
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
use history::{
    clear_history, delete_history_entry, get_history_retention_days, list_history_entries,
    set_history_retention_days,
};
use llm::{get_llm_model, list_llm_catalog, list_ollama_models, pull_llm_model, set_llm_model};
use models::{download_model, list_models, set_active_model};
use modes::{get_mode_rules, list_running_apps, remove_mode_rule, set_mode_rule};
use recording::{
    get_active_input_device, get_last_frontmost_app, get_vocabulary, list_input_devices,
    set_input_device, set_vocabulary, toggle_recording, toggle_recording_command, RecordingState,
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

    let mut builder = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Dev Whisper Settings")
    .inner_size(380.0, 640.0)
    .min_inner_size(380.0, 400.0)
    .resizable(true);

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
            list_running_apps,
            get_last_frontmost_app,
            get_vocabulary,
            set_vocabulary,
            list_history_entries,
            clear_history,
            delete_history_entry,
            get_history_retention_days,
            set_history_retention_days,
            list_ollama_models,
            list_llm_catalog,
            pull_llm_model,
            get_llm_model,
            set_llm_model,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                prompt_for_accessibility_permission();
            }

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
            whisper.set_vocabulary(saved_config.vocabulary.clone());

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

            // Purge transcript history older than the configured retention
            // window on every launch.
            let purge_app = app.handle().clone();
            let retention_days = saved_config.history_retention_days;
            std::thread::spawn(move || {
                history::purge_old_entries(&purge_app, retention_days);
            });

            // Reopen the widget wherever the user last dragged it, instead
            // of always re-centering (tauri.conf.json's "center": true is
            // just the first-ever-launch fallback, before any position has
            // been saved).
            if let Some(widget) = app.get_webview_window("widget") {
                if let Some((x, y)) = saved_config.widget_position {
                    let _ = widget.set_position(tauri::LogicalPosition::new(x, y));
                }

                let position_app = app.handle().clone();
                widget.on_window_event(move |event| {
                    if let tauri::WindowEvent::Moved(physical_pos) = event {
                        let Some(window) = position_app.get_webview_window("widget") else {
                            return;
                        };
                        let Ok(scale) = window.scale_factor() else {
                            return;
                        };
                        let logical = physical_pos.to_logical::<f64>(scale);
                        let mut cfg = config::load(&position_app);
                        cfg.widget_position = Some((logical.x, logical.y));
                        let _ = config::save(&position_app, &cfg);
                    }
                });
            }

            let toggle_recording_item = MenuItem::with_id(app, "toggle_recording", "Start/Stop Recording", true, None::<&str>)?;
            let toggle_widget = MenuItem::with_id(app, "toggle_widget", "Show/Hide Widget", true, None::<&str>)?;
            let open_settings_item = MenuItem::with_id(app, "open_settings", "Open Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&toggle_recording_item, &toggle_widget, &open_settings_item, &quit],
            )?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle_recording" => toggle_recording(app),
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
                    "open_settings" => {
                        let _ = open_settings(app.clone());
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
