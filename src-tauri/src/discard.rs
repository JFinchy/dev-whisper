//! Throwing a dictation away, and getting it back.
//!
//! Esc while recording discards the recording (nothing is transcribed);
//! Esc while it's being transcribed or refined cancels the paste. Either
//! way the recording (or its finished text) is held for [`UNDO_WINDOW`],
//! and Undo (the widget button, or ⇧Esc) delivers it after all.
//!
//! Esc and ⇧Esc are only registered while they mean something here (a
//! recording or a paste in progress, an undo on offer), so other apps keep
//! them the rest of the time.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

pub const UNDO_WINDOW: Duration = Duration::from_secs(8);

enum KeyOp {
    Register(Shortcut),
    Unregister(Shortcut),
}

/// Registering a shortcut waits for the main thread. Callers here can hold
/// the recording lock the main thread may itself be waiting on, so every
/// change goes through one worker thread that holds nothing (and keeps the
/// changes in order).
fn key_op(app: &AppHandle, op: KeyOp) {
    static WORKER: std::sync::OnceLock<Mutex<std::sync::mpsc::Sender<KeyOp>>> =
        std::sync::OnceLock::new();
    let sender = WORKER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<KeyOp>();
        let app = app.clone();
        std::thread::spawn(move || {
            for op in rx {
                let shortcuts = app.global_shortcut();
                match op {
                    KeyOp::Register(key) => {
                        if !shortcuts.is_registered(key) {
                            if let Err(err) = shortcuts.register(key) {
                                crate::applog!("discard: couldn't register {key:?}: {err}");
                            }
                        }
                    }
                    KeyOp::Unregister(key) => {
                        if shortcuts.is_registered(key) {
                            let _ = shortcuts.unregister(key);
                        }
                    }
                }
            }
        });
        Mutex::new(tx)
    });
    let _ = sender.lock().unwrap().send(op);
}

pub fn escape() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub fn undo_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SHIFT), Code::Escape)
}

/// What Undo would bring back.
pub enum Held {
    /// A recording stopped by Esc before it was transcribed.
    Recording {
        path: PathBuf,
        reply: Option<crate::agents::WaitingAgent>,
    },
    /// Text that was ready to paste when Esc cancelled it.
    Text(String),
}

#[derive(Default)]
pub struct DiscardState {
    /// A paste is being prepared (transcribing or refining).
    processing: AtomicBool,
    /// Esc was pressed while processing: deliver nothing.
    cancel_requested: AtomicBool,
    held: Mutex<Option<(u64, Held)>>,
    next_id: AtomicU64,
}

impl DiscardState {
    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }
}

/// Registers Esc for as long as a dictation is being recorded or turned
/// into a paste. Skipped if the user's own hotkey is Esc.
pub fn grab_escape(app: &AppHandle) {
    if escape_is_users_hotkey(app) {
        return;
    }
    key_op(app, KeyOp::Register(escape()));
}

pub fn release_escape(app: &AppHandle) {
    if escape_is_users_hotkey(app) {
        return;
    }
    key_op(app, KeyOp::Unregister(escape()));
}

fn escape_is_users_hotkey(app: &AppHandle) -> bool {
    let current = app
        .state::<crate::shortcut::PushToTalkState>()
        .current
        .lock()
        .unwrap()
        .clone();
    [current.clone(), crate::shortcut::skip_variant(&current)]
        .iter()
        .filter_map(|c| crate::shortcut::to_shortcut(c).ok())
        .any(|s| s == escape() || s == undo_shortcut())
}

/// Called when a recording's processing starts and ends.
pub fn set_processing(app: &AppHandle, processing: bool) {
    let state = discard_state(app);
    state.processing.store(processing, Ordering::SeqCst);
    if processing {
        state.cancel_requested.store(false, Ordering::SeqCst);
    } else {
        state.cancel_requested.store(false, Ordering::SeqCst);
        release_escape(app);
    }
}

fn discard_state(app: &AppHandle) -> tauri::State<'_, DiscardState> {
    app.state::<DiscardState>()
}

/// Esc: discard the recording, or cancel the paste being prepared.
pub fn on_escape(app: &AppHandle) {
    use crate::capture::{CaptureState, Request};
    let recording = app.state::<crate::recording::RecordingState>();
    if matches!(recording.capture.state(), CaptureState::Dictating { .. }) {
        crate::recording::request_dictation(app, Request::DiscardDictation);
        return;
    }
    let state = discard_state(app);
    if state.processing.load(Ordering::SeqCst) {
        crate::applog!("discard: paste cancelled while processing");
        state.cancel_requested.store(true, Ordering::SeqCst);
        // Stop the LLM now rather than waiting for it; Undo then pastes
        // whatever text was ready.
        recording
            .skip_refine
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = app.emit("dictation-discarded", "cancelled");
    }
}

/// Keeps what was discarded for the undo window, then forgets it.
pub fn hold(app: &AppHandle, held: Held) {
    let state = discard_state(app);
    let id = state.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some((_, old)) = state.held.lock().unwrap().replace((id, held)) {
        forget(old);
    }
    key_op(app, KeyOp::Register(undo_shortcut()));
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(UNDO_WINDOW);
        let state = discard_state(&app);
        let expired = {
            let mut held = state.held.lock().unwrap();
            match held.as_ref() {
                Some((held_id, _)) if *held_id == id => held.take(),
                _ => None,
            }
        };
        if let Some((_, held)) = expired {
            forget(held);
            key_op(&app, KeyOp::Unregister(undo_shortcut()));
            let _ = app.emit("discard-expired", ());
        }
    });
}

/// Drops anything on offer (a new recording started).
pub fn clear(app: &AppHandle) {
    if let Some((_, held)) = discard_state(app).held.lock().unwrap().take() {
        forget(held);
        key_op(app, KeyOp::Unregister(undo_shortcut()));
        let _ = app.emit("discard-expired", ());
    }
}

fn forget(held: Held) {
    if let Held::Recording { path, .. } = held {
        if let Err(err) = std::fs::remove_file(&path) {
            crate::applog!("discard: could not delete {}: {err}", path.display());
        }
    }
}

/// Undo: transcribe and paste the discarded recording, or paste the text
/// whose paste was cancelled.
pub fn undo(app: &AppHandle) -> bool {
    let Some((_, held)) = discard_state(app).held.lock().unwrap().take() else {
        return false;
    };
    key_op(app, KeyOp::Unregister(undo_shortcut()));
    crate::applog!("discard: undone");
    let _ = app.emit("discard-undone", ());
    match held {
        Held::Recording { path, reply } => {
            let app = app.clone();
            std::thread::spawn(move || {
                crate::recording::transcribe_held_recording(&app, &path, reply);
                let _ = std::fs::remove_file(&path);
            });
        }
        Held::Text(text) => {
            let app = app.clone();
            std::thread::spawn(move || {
                if let Err(err) = crate::paste::paste_text(&text) {
                    let _ = app.emit("transcript-error", err);
                } else {
                    let _ = app.emit("transcript-copied", "Pasted".to_string());
                }
            });
        }
    }
    true
}

#[tauri::command]
pub fn discard_recording(app: AppHandle) {
    on_escape(&app);
}

#[tauri::command]
pub fn undo_discard(app: AppHandle) -> bool {
    undo(&app)
}
