mod app_detect;
mod audio;
mod boilerplate;
mod clipboard;
mod config;
mod history;
mod isolate;
mod llm;
mod logging;
mod models;
mod modes;
mod paste;
mod punctuation;
mod recording;
mod shortcut;
mod stt;
mod syntax;
mod webhook;
mod widget;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState as PressState};

use audio::AudioHandle;
use history::{
    clear_history, delete_history_entry, get_history_retention_days, get_journal_enabled,
    list_history_entries, reprocess_history_text, search_history_entries,
    set_history_retention_days, set_journal_enabled, update_history_entry_text,
};
use llm::{get_llm_model, list_llm_catalog, list_ollama_models, pull_llm_model, set_llm_model};
use logging::{clear_logs, get_logs};
use models::{download_model, list_models, set_active_model};
use modes::{get_mode_rules, list_running_apps, remove_mode_rule, set_mode_rule};
use recording::{
    get_active_input_device, get_copy_only, get_isolated_voice_enabled, get_last_frontmost_app,
    get_press_enter_enabled, get_vocabulary, list_input_devices, set_copy_only, set_input_device,
    set_isolated_voice_enabled, set_press_enter_enabled, set_vocabulary, toggle_recording,
    toggle_recording_command, RecordingState,
};
use shortcut::{get_shortcut, set_shortcut, PushToTalkState};
use webhook::{get_webhook_url, send_test_webhook, set_webhook_url};
use widget::{get_widget_mode, set_widget_mode, set_widget_size};
use stt::WhisperEngine;

/// Shared between the tray builder in `run()` and `recording::toggle_recording`
/// (which looks the tray back up via `AppHandle::tray_by_id` to swap its icon)
/// so both sides agree on which tray they mean without threading a handle
/// through `RecordingState`.
pub const TRAY_ICON_ID: &str = "main";

/// The tray glyph is deliberately a separate, simpler asset from the Dock
/// icon (icons/icon.png) — mic silhouette only, no waveform, no
/// background — since the waveform accent turns to mush at 16-22px menu
/// bar sizes. Rendered as a macOS "template" image by default so the
/// system tints it to match light/dark menu bars, like every other
/// well-behaved menu-bar app; recording.rs switches off template mode
/// and overlays a red dot on top of it while actively recording, since
/// template images can't show color.
pub fn tray_base_icon() -> tauri::image::Image<'static> {
    tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon-template.png"))
        .expect("bundled tray-icon-template.png must decode")
}

/// Draws a small red dot over the bottom-right corner of `base` — gives the
/// tray icon a distinct "recording" state without needing a separate icon
/// asset checked into the repo. Recomputed on each recording start rather
/// than cached; a 32x32 pixel loop is sub-millisecond.
pub fn recording_tray_icon(base: &tauri::image::Image) -> tauri::image::Image<'static> {
    let width = base.width();
    let height = base.height();
    let mut rgba = base.rgba().to_vec();

    let radius = (width.min(height) as f32 * 0.3).max(3.0);
    let cx = width as f32 * 0.72;
    let cy = height as f32 * 0.72;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= radius * radius {
                let idx = ((y * width + x) * 4) as usize;
                rgba[idx] = 220;
                rgba[idx + 1] = 38;
                rgba[idx + 2] = 38;
                rgba[idx + 3] = 255;
            }
        }
    }

    tauri::image::Image::new_owned(rgba, width, height)
}

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
    .inner_size(520.0, 680.0)
    .min_inner_size(460.0, 400.0)
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

/// Whether Dev Whisper is currently registered as a macOS login item.
/// Off by default (see `run()` — the autostart plugin is registered but
/// never auto-enabled) since silently adding a login item without an
/// explicit user action would be a surprising thing for a privacy-first
/// app to do; this only reflects whatever the user has toggled in Settings.
#[tauri::command]
fn get_autostart_enabled(app: tauri::AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    }
    .map_err(|e| e.to_string())
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_deep_link::init())
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
            get_autostart_enabled,
            set_autostart_enabled,
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
            get_copy_only,
            set_copy_only,
            get_isolated_voice_enabled,
            set_isolated_voice_enabled,
            get_press_enter_enabled,
            set_press_enter_enabled,
            list_history_entries,
            search_history_entries,
            reprocess_history_text,
            update_history_entry_text,
            clear_history,
            delete_history_entry,
            get_history_retention_days,
            set_history_retention_days,
            get_journal_enabled,
            set_journal_enabled,
            list_ollama_models,
            list_llm_catalog,
            pull_llm_model,
            get_llm_model,
            set_llm_model,
            get_logs,
            clear_logs,
            get_widget_mode,
            set_widget_mode,
            set_widget_size,
            get_webhook_url,
            set_webhook_url,
            send_test_webhook,
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

            // Deep-link hooks for external automation (Raycast, Hammerspoon,
            // Alfred, shell scripts): `open devwhisper://toggle-recording`
            // and friends. start/stop are idempotent — calling
            // start-recording while already recording is a no-op rather
            // than toggling it off, so a script doesn't need to track state
            // itself.
            let deep_link_app = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    let action = url.host_str().unwrap_or("");
                    crate::applog!("deep-link: received action={action:?} url={url}");
                    let state = deep_link_app.state::<RecordingState>();
                    let is_recording = state.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                    match action {
                        "toggle-recording" => recording::toggle_recording(&deep_link_app),
                        "start-recording" if !is_recording => recording::toggle_recording(&deep_link_app),
                        "stop-recording" if is_recording => recording::toggle_recording(&deep_link_app),
                        "start-recording" | "stop-recording" => {}
                        other => crate::applog!("deep-link: unrecognized action {other:?}"),
                    }
                }
            });

            // Warm up the model in the background so the first real
            // recording doesn't pay for Metal shader compilation, which can
            // take several seconds and otherwise happens on the user's
            // first "transcribing…".
            let warm_app = app.handle().clone();
            std::thread::spawn(move || {
                let state = warm_app.state::<RecordingState>();
                if let Err(err) = state.whisper.ensure_loaded() {
                    crate::applog!("model warm-up skipped: {err}");
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
                // tauri.conf.json's fixed 220x60 is only the Compact-mode
                // size; apply whatever mode was last saved before the
                // window is shown.
                let (width, height) = saved_config.widget_mode.base_size();
                let _ = widget.set_size(tauri::LogicalSize::new(width, height));

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

            TrayIconBuilder::with_id(TRAY_ICON_ID)
                .icon(tray_base_icon())
                .icon_as_template(true)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_tray_icon_paints_a_red_dot_without_changing_dimensions() {
        let width = 32;
        let height = 32;
        let base = tauri::image::Image::new_owned(vec![10u8; (width * height * 4) as usize], width, height);

        let marked = recording_tray_icon(&base);
        assert_eq!(marked.width(), width);
        assert_eq!(marked.height(), height);

        // Center of the dot (per recording_tray_icon's 0.72/0.72 placement)
        // should be opaque red.
        let cx = (width as f32 * 0.72) as u32;
        let cy = (height as f32 * 0.72) as u32;
        let idx = ((cy * width + cx) * 4) as usize;
        let rgba = marked.rgba();
        assert_eq!(&rgba[idx..idx + 4], &[220, 38, 38, 255]);

        // Top-left corner, far from the dot, should be untouched.
        assert_eq!(&rgba[0..4], &[10, 10, 10, 10]);
    }
}
