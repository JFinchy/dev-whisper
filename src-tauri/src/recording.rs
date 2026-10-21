use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::app_detect::AppInfo;
use crate::audio::AudioHandle;
use crate::capture::{CaptureController, Request, Transition, Trigger};
use crate::modes;
use crate::paste::paste_text;
use crate::pipeline;
use crate::stt::WhisperEngine;

pub struct RecordingState {
    pub audio: AudioHandle,
    pub whisper: WhisperEngine,
    /// Owns "what is the mic being used for" — dictation, enrollment, or
    /// nothing. See `capture.rs`.
    pub capture: CaptureController,
    /// Held across a state transition *and* its audio start/stop command,
    /// so two threads requesting at once can't reach the audio thread in
    /// the opposite order of their transitions (a Stop landing before the
    /// Start it was meant to end would leave the stream running).
    pub capture_op: Mutex<()>,
    /// Whatever app was frontmost when the current/last recording started —
    /// used to pick a formatting mode, and doubles as "last app you were
    /// in" for the Settings quick-add UI (capturing frontmost app right
    /// before Settings opens doesn't work: opening Settings requires
    /// clicking a button in the widget, so the widget is always already
    /// frontmost by that point).
    pub active_app: Mutex<Option<AppInfo>>,
    /// One-shot override for the *next* dictation's mode, set from the
    /// widget's quick-actions flyout (see WidgetView.tsx) as a mode name —
    /// resolved via `modes::settings_for_name` at consume time, so the
    /// override carries the whole mode's settings (behavior, stt_model,
    /// llm_refinement), not just a raw behavior. Takes priority over
    /// whatever `modes::resolve_settings` would otherwise pick for the
    /// frontmost app, and is consumed (cleared) the moment a dictation uses
    /// it, so it never silently applies to a second one.
    pub mode_override: Mutex<Option<String>>,
    /// "Paste as-is" for the current dictation (widget click or the skip
    /// shortcut): skip LLM refinement, or stop it if it's running. Reset
    /// when a new dictation starts.
    pub skip_refine: std::sync::atomic::AtomicBool,
}

pub fn toggle_recording(app: &AppHandle, trigger: Trigger) {
    request_dictation(app, Request::ToggleDictation(trigger));
}

/// The one entry point for starting/stopping dictation from any trigger.
pub fn request_dictation(app: &AppHandle, request: Request) {
    let state = app.state::<RecordingState>();
    let _op = state.capture_op.lock().unwrap();
    match state.capture.request(request) {
        Transition::StartedDictation { session } => start_dictation(app, &state, session),
        Transition::StoppedDictation { .. } => stop_dictation(app, &state),
        Transition::StartedEnrollment { .. }
        | Transition::StoppedEnrollment { .. }
        | Transition::Rejected(_) => {}
    }
}

fn stop_dictation(app: &AppHandle, state: &RecordingState) {
    let stopped_at = std::time::Instant::now();
    crate::refresh_tray_indicator(app);
    match state.audio.stop() {
        Ok(path) => {
            let _ = app.emit("recording-stopped", path.to_string_lossy().to_string());
            let app = app.clone();
            std::thread::spawn(move || {
                transcribe_and_paste(&app, &path, stopped_at);
                // Raw voice recordings shouldn't accumulate in /tmp; history
                // keeps the transcript, not the audio.
                if let Err(err) = std::fs::remove_file(&path) {
                    crate::applog!("recording: could not delete {}: {err}", path.display());
                }
            });
        }
        Err(err) => {
            let _ = app.emit("recording-error", err);
        }
    }
}

fn current_mic_name(state: &RecordingState) -> String {
    use crate::audio::DeviceStatus::*;
    match state.audio.device_status() {
        Some(Preferred { using } | Fallback { using, .. } | SystemDefault { using }) => using,
        Some(Unavailable { .. }) | None => "your microphone".to_string(),
    }
}

fn start_dictation(app: &AppHandle, state: &RecordingState, session: u64) {
    state
        .skip_refine
        .store(false, std::sync::atomic::Ordering::Relaxed);
    crate::refresh_tray_indicator(app);
    state.audio.start();

    for model in crate::llm::models_in_use(&crate::config::load(app)) {
        crate::llm::prewarm(&model);
    }

    // Live level meter: polls the audio thread's current RMS level (see
    // `audio::AudioHandle::current_level`) at 20fps rather than per audio
    // callback, which fires far more often than any redraw needs. Bound to
    // this session id, so it exits when *this* recording ends even if a
    // new one has already started.
    let level_app = app.clone();
    std::thread::spawn(move || {
        let state = level_app.state::<RecordingState>();
        let mut silence = crate::audio::SilenceWatch::default();
        while state.capture.is_session_active(session) {
            let level = state.audio.current_level();
            let _ = level_app.emit("audio-level", level);
            match silence.observe(level, std::time::Instant::now()) {
                Some(true) => {
                    let mic = current_mic_name(&state);
                    crate::applog!("audio: no signal from {mic} for 2s while recording");
                    let _ = level_app.emit(
                        "mic-silent",
                        format!("No sound from {mic}. Is it muted? Hover to switch mics."),
                    );
                }
                Some(false) => {
                    crate::applog!("audio: signal resumed");
                    let _ = level_app.emit("mic-silent-cleared", ());
                }
                None => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    // Capture which app the user was in *before* showing our own widget —
    // otherwise NSWorkspace reports Dev Whisper itself as frontmost. Both
    // steps run in one main-thread closure (NSWorkspace requires the main
    // thread) so the ordering is guaranteed.
    let main_thread_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let info = crate::app_detect::frontmost_app_info();
        crate::applog!(
            "modes: frontmost app at recording start = {:?}",
            info.as_ref().map(|i| (&i.bundle_id, &i.name))
        );
        let state = main_thread_app.state::<RecordingState>();
        *state.active_app.lock().unwrap() = info;

        // `show()` only, not `set_focus()` — stealing keyboard focus made
        // the paste land in the widget instead of the app being dictated
        // into.
        if let Some(window) = main_thread_app.get_webview_window("widget") {
            let _ = window.show();
        }
    });

    let _ = app.emit("recording-started", ());
}

fn transcribe_and_paste(
    app: &AppHandle,
    wav_path: &std::path::Path,
    stopped_at: std::time::Instant,
) {
    let state = app.state::<RecordingState>();

    let active_app = state.active_app.lock().unwrap().clone();
    let bundle_id = active_app.as_ref().map(|info| info.bundle_id.clone());
    let app_name = active_app.map(|info| info.name);

    // Mode (and its stt_model override, if any) has to be resolved *before*
    // transcribing now, not after — which model to transcribe with depends
    // on it. Casing-command detection still happens after, on the
    // resulting text, since that's orthogonal to which model produced it.
    let cfg = crate::config::load(app);
    let mut settings = modes::resolve_settings(bundle_id.as_deref(), &cfg.modes);
    if let Some(override_name) = state.mode_override.lock().unwrap().take() {
        match modes::settings_for_name(&override_name, &cfg.modes) {
            Some(override_settings) => {
                crate::applog!(
                    "modes: next-dictation override {override_name:?} applied over resolved {:?}",
                    settings.mode
                );
                settings = override_settings;
            }
            None => {
                crate::applog!(
                    "modes: next-dictation override {override_name:?} no longer exists, ignoring"
                );
            }
        }
    }
    let model_override = settings
        .stt_model
        .as_ref()
        .and_then(|id| crate::models::resolve_model_path(app, id).map(|path| (id.clone(), path)));
    crate::applog!(
        "modes: bundle_id={bundle_id:?} modes={} resolved_mode={:?} stt_model_override={:?} (resolved={})",
        cfg.modes.len(),
        settings.mode,
        settings.stt_model,
        model_override.is_some(),
    );

    // Read once, up front, from the wav header (not the decoded/resampled
    // samples above) — feeds the words-per-minute stat in Insights.
    let duration_ms = crate::stt::wav_duration_ms(wav_path);

    let stt_started = std::time::Instant::now();
    let samples = crate::stt::load_samples_16k_mono(wav_path);
    let transcribed = match samples {
        Ok(samples) if crate::audio::is_dead_air(&samples) => {
            let mic = current_mic_name(&state);
            crate::applog!(
                "audio: recording from {mic} was silent (no signal at all), skipping transcription"
            );
            let _ = app.emit(
                "transcript-error",
                format!("{mic} sent no audio. Is it muted? Hover to switch mics."),
            );
            return;
        }
        Ok(samples) => {
            let samples = crate::isolate::apply(app, samples);
            let progress_app = app.clone();
            let result = state.whisper.transcribe_samples_with_progress(
                &samples,
                model_override,
                &move |percent| {
                    let _ = progress_app.emit("transcribe-progress", percent);
                },
            );
            if result.is_ok() {
                let _ = app.emit("transcribe-progress", 100u8);
            }
            result
        }
        Err(err) => Err(err),
    };

    let stt_ms = Some(stt_started.elapsed().as_millis() as u64);

    match transcribed {
        Ok(text) if !text.is_empty() => {
            // Raw model output, logged before any pipeline pass touches it
            // (punctuation/backtrack/trigger-stripping/mode/LLM all mutate
            // `text` below) — the only place to see what Whisper actually
            // heard, needed to tell a real STT mis-transcription apart from
            // a bug in one of those passes when a voice command misfires.
            crate::applog!("whisper: raw transcript={text:?}");

            let text = crate::stt::strip_non_speech(&text);
            if text.is_empty() {
                crate::applog!(
                    "whisper: transcript was only non-speech annotations, nothing to paste"
                );
                let _ = app.emit("transcript-error", "no speech detected".to_string());
                return;
            }

            process_transcript(
                app,
                text,
                TranscriptContext {
                    cfg: &cfg,
                    settings: &settings,
                    app_name,
                    duration_ms,
                    paste_guard: None,
                    stopped_at: Some(stopped_at),
                    stt_ms,
                },
            );
        }
        Ok(_) => {
            let _ = app.emit("transcript-error", "no speech detected".to_string());
        }
        Err(err) => {
            let _ = app.emit("transcript-error", err);
        }
    }
}

/// Everything a transcript needs besides its text to be formatted and
/// delivered — shared by push-to-talk dictation and ambient listening.
pub(crate) struct TranscriptContext<'a> {
    pub cfg: &'a crate::config::AppConfig,
    pub settings: &'a modes::ResolvedSettings,
    pub app_name: Option<String>,
    pub duration_ms: Option<u64>,
    /// Bundle id that must still be frontmost at delivery time for the text
    /// to be pasted (ambient only); `None` pastes unconditionally.
    pub paste_guard: Option<String>,
    /// When the user stopped recording, for end-to-end latency. `None` for
    /// ambient dictation, which has no explicit stop.
    pub stopped_at: Option<std::time::Instant>,
    /// How long Whisper (plus voice isolation) took.
    pub stt_ms: Option<u64>,
}

fn ms_or_dash(ms: Option<u64>) -> String {
    ms.map_or_else(|| "-".to_string(), |ms| format!("{ms}ms"))
}

/// Runs a cleaned transcript through the formatting pipeline (lists,
/// punctuation, backtrack, voice commands, snippets, casing, boilerplate,
/// mode formatting, LLM refinement) and delivers it.
pub(crate) fn process_transcript(app: &AppHandle, text: String, ctx: TranscriptContext) {
    let TranscriptContext {
        cfg,
        settings,
        app_name,
        duration_ms,
        ..
    } = &ctx;
    let (cfg, settings, app_name, duration_ms) = (*cfg, *settings, app_name.clone(), *duration_ms);
    let pipeline::Formatted {
        text: formatted,
        mode_label,
        press_enter: should_press_enter,
        append_clipboard: should_append_clipboard,
        features_used,
        spoken_words,
        llm_ms,
        unrefined,
    } = pipeline::format(
        text,
        cfg,
        settings,
        &pipeline::OllamaRefiner {
            on_progress: &|percent| {
                let _ = app.emit("refine-progress", percent);
            },
            cancel: &app.state::<RecordingState>().skip_refine,
        },
        &mut |model| {
            let _ = app.emit("refining-started", model.to_string());
        },
    );

    // Applied uniformly after whichever branch above resolved
    // `formatted` — clipboard content is appended raw, not run
    // through mode formatting or LLM refinement, the same way a
    // snippet body would be. A trigger with nothing on the
    // clipboard (or a non-text clipboard) leaves `formatted`
    // unchanged rather than pasting a Rust error string.
    let (formatted, unrefined) = if should_append_clipboard {
        match crate::clipboard::read_clipboard_text() {
            Some(clip) => (
                crate::clipboard::append(&formatted, &clip),
                unrefined.map(|u| crate::clipboard::append(&u, &clip)),
            ),
            None => {
                crate::applog!("clipboard: append triggered but clipboard was empty or unreadable");
                (formatted, unrefined)
            }
        }
    } else {
        (formatted, unrefined)
    };

    // A fully-consumed "press enter"-only utterance formats down
    // to an empty string — nothing to paste/copy in that case
    // (would otherwise silently overwrite the clipboard with
    // nothing), but Enter still needs to fire below.
    // Ambient dictation passes the app that was frontmost when the speech
    // arrived; if focus moved during transcription, pasting would type into
    // whatever took focus, so the text goes to the clipboard instead.
    let focus_moved = ctx.paste_guard.as_deref().is_some_and(|expected| {
        let now = crate::app_detect::frontmost_bundle_id_blocking(app);
        if now.as_deref() != Some(expected) {
            crate::applog!("paste guard: frontmost changed from {expected:?} to {now:?}, copying instead of pasting");
            true
        } else {
            false
        }
    });
    let copy_only = cfg.copy_only || focus_moved;
    let deliver_started = std::time::Instant::now();
    let deliver = if formatted.is_empty() {
        Ok(())
    } else if copy_only {
        crate::paste::copy_text(&formatted)
    } else {
        paste_text(&formatted)
    };
    let latency_ms = ctx.stopped_at.map(|t| t.elapsed().as_millis() as u64);
    crate::applog!(
        "perf: stt={} llm={} deliver={}ms total={}",
        ms_or_dash(ctx.stt_ms),
        ms_or_dash(llm_ms),
        deliver_started.elapsed().as_millis(),
        ms_or_dash(latency_ms),
    );

    match deliver {
        Ok(()) => {
            // Copy-only means the user opted out of synthetic
            // keystrokes entirely (that's the point of the mode,
            // and why it doesn't need Accessibility permission),
            // so "press enter" is skipped rather than sending one
            // anyway.
            if should_press_enter && !copy_only {
                if let Err(err) = crate::paste::press_enter() {
                    crate::applog!("press_enter: failed to simulate Enter: {err}");
                }
            }

            // Cloned before the moves into `append_entry` below —
            // the webhook payload needs its own copies of whatever
            // it fires after.
            let webhook_app_name = app_name.clone();
            let webhook_mode = mode_label.clone();

            // Only log to history what actually reached the user —
            // a failed paste/copy shouldn't silently show up as
            // history, and neither should an empty "press enter
            // only" utterance with nothing to show.
            let timestamp_ms = if !formatted.is_empty() {
                Some(crate::history::append_entry(
                    app,
                    crate::history::NewEntry {
                        text: formatted.clone(),
                        app_name,
                        mode: Some(mode_label),
                        duration_ms,
                        features_used,
                        spoken_words,
                        latency_ms,
                        original_text: unrefined,
                    },
                ))
            } else {
                None
            };

            // Mirrors the history-logging condition above — an
            // empty "press enter only" utterance has no real
            // transcript to send downstream either.
            if let (Some(webhook_url), Some(timestamp_ms)) = (cfg.webhook_url.clone(), timestamp_ms)
            {
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
            if let (true, Some(timestamp_ms)) = (cfg.journal_enabled, timestamp_ms) {
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

            if focus_moved {
                let _ = app.emit(
                    "transcript-copied",
                    "Copied — you switched apps before it could paste".to_string(),
                );
            } else if copy_only && !formatted.is_empty() {
                let _ = app.emit("transcript-copied", "Copied to clipboard".to_string());
            } else {
                let _ = app.emit("transcript-ready", formatted);
            }
        }
        Err(err) => {
            let _ = app.emit("transcript-error", err);
        }
    }
}

/// "Paste as-is": while recording, stops and skips refinement; while
/// refining, stops the LLM and pastes the unrefined text now. From the
/// widget's refining bar or the skip shortcut (hotkey + Shift).
pub fn paste_as_is(app: &AppHandle) {
    let state = app.state::<RecordingState>();
    state
        .skip_refine
        .store(true, std::sync::atomic::Ordering::Relaxed);
    crate::applog!("llm: paste as-is requested");
    if matches!(
        state.capture.state(),
        crate::capture::CaptureState::Dictating { .. }
    ) {
        request_dictation(app, Request::StopDictation(Trigger::Hotkey));
    }
}

#[tauri::command]
pub fn paste_as_is_command(app: AppHandle) {
    paste_as_is(&app);
}

#[tauri::command]
pub fn toggle_recording_command(app: AppHandle) {
    toggle_recording(&app, Trigger::Button);
}

/// Which mic capture is actually using (or falling back from). Read once by
/// the widget on mount; changes after that arrive as `audio-device-status`.
#[tauri::command]
pub fn get_audio_device_status(
    state: tauri::State<RecordingState>,
) -> Option<crate::audio::DeviceStatus> {
    state.audio.device_status()
}

// Off the main thread: sync commands run on it, and this one does I/O.
#[tauri::command(async)]
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

/// Set (or clear, with `None`) the one-shot mode override for the next
/// dictation — see `RecordingState::mode_override`. Not persisted to
/// config: this is deliberately session/one-shot state, not a setting.
#[tauri::command]
pub fn set_next_mode_override(mode_name: Option<String>, state: tauri::State<RecordingState>) {
    *state.mode_override.lock().unwrap() = mode_name;
}

#[tauri::command]
pub fn get_next_mode_override(state: tauri::State<RecordingState>) -> Option<String> {
    state.mode_override.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_press_enter_enabled(app: AppHandle) -> bool {
    crate::config::load(&app).press_enter_enabled
}

#[tauri::command]
pub fn set_press_enter_enabled(app: AppHandle, enabled: bool) {
    let mut cfg = crate::config::load(&app);
    cfg.press_enter_enabled = enabled;
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
