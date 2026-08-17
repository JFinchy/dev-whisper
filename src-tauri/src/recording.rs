use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::app_detect::AppInfo;
use crate::audio::AudioHandle;
use crate::modes;
use crate::paste::paste_text;
use crate::stt::WhisperEngine;

pub struct RecordingState {
    pub audio: AudioHandle,
    pub whisper: WhisperEngine,
    pub is_recording: AtomicBool,
    /// Whatever app was frontmost when the current/last recording started —
    /// used to pick a formatting mode, and doubles as "last app you were
    /// in" for the Settings quick-add UI (capturing frontmost app right
    /// before Settings opens doesn't work: opening Settings requires
    /// clicking a button in the widget, so the widget is always already
    /// frontmost by that point).
    pub active_app: Mutex<Option<AppInfo>>,
}

/// Swaps the tray icon between its default template look and a red-dot
/// "recording" variant so the recording state is visible even when the
/// widget window is hidden or off-screen. Template mode is turned off
/// while recording since template images can't show color, and back on
/// when idle so the glyph keeps auto-adapting to light/dark menu bars.
/// Best-effort: a missing tray is silently skipped rather than failing
/// the recording toggle over a cosmetic issue.
fn set_tray_recording_indicator(app: &AppHandle, recording: bool) {
    let Some(tray) = app.tray_by_id(crate::TRAY_ICON_ID) else {
        return;
    };
    let base = crate::tray_base_icon();
    let icon = if recording {
        crate::recording_tray_icon(&base)
    } else {
        base
    };
    let _ = tray.set_icon_with_as_template(Some(icon), !recording);
}

/// Shared by the tray/UI toggle command and the global hotkey listener so
/// both drive the same start/stop lifecycle.
pub fn toggle_recording(app: &AppHandle) {
    let state = app.state::<RecordingState>();
    let was_recording = state.is_recording.fetch_xor(true, Ordering::SeqCst);

    if was_recording {
        set_tray_recording_indicator(app, false);
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
        set_tray_recording_indicator(app, true);
        state.audio.start();

        // Capture which app the user was in *before* showing/focusing our
        // own widget below — otherwise NSWorkspace reports Dev Whisper
        // itself as frontmost. Both steps run in one main-thread closure
        // (NSWorkspace requires the main thread) so the ordering is
        // guaranteed rather than a race between two separately-queued
        // main-thread dispatches.
        let main_thread_app = app.clone();
        let _ = app.run_on_main_thread(move || {
            let info = crate::app_detect::frontmost_app_info();
            crate::applog!(
                "modes: frontmost app at recording start = {:?}",
                info.as_ref().map(|i| (&i.bundle_id, &i.name))
            );
            let state = main_thread_app.state::<RecordingState>();
            *state.active_app.lock().unwrap() = info;

            // The widget starts hidden; without this, triggering a
            // recording via the global hotkey gives no visual feedback
            // that it's running.
            if let Some(window) = main_thread_app.get_webview_window("widget") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        });

        let _ = app.emit("recording-started", ());
    }
}

fn transcribe_and_paste(app: &AppHandle, wav_path: &std::path::Path) {
    let state = app.state::<RecordingState>();

    let active_app = state.active_app.lock().unwrap().clone();
    let bundle_id = active_app.as_ref().map(|info| info.bundle_id.clone());
    let app_name = active_app.map(|info| info.name);

    // Mode (and its stt_model override, if any) has to be resolved *before*
    // transcribing now, not after — which model to transcribe with depends
    // on it. Casing-command detection still happens after, on the
    // resulting text, since that's orthogonal to which model produced it.
    let cfg = crate::config::load(app);
    let settings = modes::resolve_settings(bundle_id.as_deref(), &cfg.mode_rules);
    let model_override = settings.stt_model.as_ref().and_then(|id| {
        crate::models::resolve_model_path(app, id).map(|path| (id.clone(), path))
    });
    crate::applog!(
        "modes: bundle_id={bundle_id:?} rules={} resolved_mode={:?} stt_model_override={:?} (resolved={})",
        cfg.mode_rules.len(),
        settings.mode,
        settings.stt_model,
        model_override.is_some(),
    );

    let samples = crate::stt::load_samples_16k_mono(wav_path);
    let transcribed = match samples {
        Ok(samples) => {
            let samples = crate::isolate::apply(app, samples);
            state.whisper.transcribe_samples(&samples, model_override)
        }
        Err(err) => Err(err),
    };

    match transcribed {
        Ok(text) if !text.is_empty() => {
            // Casing directives ("snake case error response handler") are a
            // cross-cutting syntax command, not gated behind a Mode — they
            // apply no matter which app/mode is active, and skip both
            // rule-based formatting and LLM refinement entirely since the
            // mechanical transform already fully resolves the output.
            let (formatted, mode_label) = if let Some(cased) =
                crate::syntax::try_apply_casing_command(&text)
            {
                crate::applog!("syntax: casing command matched, transcript={text:?} output={cased:?}");
                (cased, "casing".to_string())
            } else if let Some(request) = crate::boilerplate::try_extract_request(&text) {
                crate::applog!("boilerplate: request matched, transcript={text:?} request={request:?}");
                let _ = app.emit("refining-started", ());
                match crate::llm::generate_boilerplate(&request, &cfg.llm_model) {
                    Ok(code) => (code, "boilerplate".to_string()),
                    Err(err) => {
                        // Falls back to normal mode formatting rather than
                        // pasting nothing — a down/missing Ollama shouldn't
                        // eat the user's dictation.
                        crate::applog!("boilerplate: generation failed, falling back to plain formatting: {err}");
                        let formatted = modes::apply_mode(settings.mode, &text);
                        (formatted, format!("{:?}", settings.mode))
                    }
                }
            } else {
                let formatted = modes::apply_mode(settings.mode, &text);
                crate::applog!("modes: transcript={text:?} formatted={formatted:?}");

                let formatted = if settings.use_llm_refinement {
                    let _ = app.emit("refining-started", ());
                    match crate::llm::refine(settings.mode, &formatted, &cfg.llm_model) {
                        Ok(refined) => refined,
                        Err(err) => {
                            crate::applog!("llm: refinement failed, pasting unrefined text: {err}");
                            formatted
                        }
                    }
                } else {
                    formatted
                };
                (formatted, format!("{:?}", settings.mode))
            };

            let deliver = if cfg.copy_only {
                crate::paste::copy_text(&formatted)
            } else {
                paste_text(&formatted)
            };

            match deliver {
                Ok(()) => {
                    // Cloned before the moves into `append_entry` below —
                    // the webhook payload needs its own copies of whatever
                    // it fires after.
                    let webhook_app_name = app_name.clone();
                    let webhook_mode = mode_label.clone();

                    // Only log to history what actually reached the user —
                    // a failed paste/copy shouldn't silently show up as
                    // history.
                    let timestamp_ms =
                        crate::history::append_entry(app, &formatted, app_name, Some(mode_label));

                    if let Some(webhook_url) = cfg.webhook_url.clone() {
                        crate::webhook::send_entry(
                            webhook_url,
                            crate::webhook::WebhookPayload {
                                timestamp_ms,
                                text: formatted.clone(),
                                // Known v1 gap: the journal summary (if
                                // enabled) is generated asynchronously
                                // below and isn't available yet — see
                                // BACKLOG.md.
                                summary: None,
                                app_name: webhook_app_name,
                                mode: Some(webhook_mode),
                            },
                        );
                    }

                    // Journal summarization happens after the entry is
                    // already saved and the transcript already delivered —
                    // a slow/unreachable Ollama call here should never add
                    // latency to the thing the user is actually waiting on.
                    if cfg.journal_enabled {
                        let journal_app = app.clone();
                        let journal_text = formatted.clone();
                        let journal_model = cfg.llm_model.clone();
                        std::thread::spawn(move || {
                            match crate::llm::summarize_for_journal(&journal_text, &journal_model) {
                                Some(Ok(summary)) => {
                                    crate::history::set_entry_summary(&journal_app, timestamp_ms, summary);
                                }
                                Some(Err(err)) => {
                                    crate::applog!("journal: summarization failed: {err}");
                                }
                                None => {}
                            }
                        });
                    }

                    let _ = app.emit("transcript-ready", formatted);
                }
                Err(err) => {
                    let _ = app.emit("transcript-error", err);
                }
            }
        }
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

#[tauri::command]
pub fn get_vocabulary(app: AppHandle) -> Vec<String> {
    crate::config::load(&app).vocabulary
}

#[tauri::command]
pub fn set_vocabulary(app: AppHandle, terms: Vec<String>, state: tauri::State<RecordingState>) {
    state.whisper.set_vocabulary(terms.clone());

    let mut cfg = crate::config::load(&app);
    cfg.vocabulary = terms;
    let _ = crate::config::save(&app, &cfg);
}

#[tauri::command]
pub fn get_copy_only(app: AppHandle) -> bool {
    crate::config::load(&app).copy_only
}

#[tauri::command]
pub fn set_copy_only(app: AppHandle, enabled: bool) {
    let mut cfg = crate::config::load(&app);
    cfg.copy_only = enabled;
    let _ = crate::config::save(&app, &cfg);
}

#[tauri::command]
pub fn get_isolated_voice_enabled(app: AppHandle) -> bool {
    crate::config::load(&app).isolated_voice_enabled
}

#[tauri::command]
pub fn set_isolated_voice_enabled(app: AppHandle, enabled: bool) {
    let mut cfg = crate::config::load(&app);
    cfg.isolated_voice_enabled = enabled;
    let _ = crate::config::save(&app, &cfg);
}

#[derive(serde::Serialize)]
pub struct FrontmostAppPayload {
    pub bundle_id: String,
    pub name: String,
    pub icon_data_uri: Option<String>,
}

/// The app the most recent recording was started in — used by Settings to
/// offer "add a mode rule for the app you just came from".
#[tauri::command]
pub fn get_last_frontmost_app(state: tauri::State<RecordingState>) -> Option<FrontmostAppPayload> {
    state
        .active_app
        .lock()
        .unwrap()
        .as_ref()
        .map(|info| FrontmostAppPayload {
            bundle_id: info.bundle_id.clone(),
            name: info.name.clone(),
            icon_data_uri: info.icon_data_uri.clone(),
        })
}
