use rdev::{listen, EventType, Key};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::AppHandle;

use crate::recording::toggle_recording;

static TRIGGERED: AtomicBool = AtomicBool::new(false);

fn is_cmd(key: &Key) -> bool {
    matches!(key, Key::MetaLeft | Key::MetaRight)
}

fn is_shift(key: &Key) -> bool {
    matches!(key, Key::ShiftLeft | Key::ShiftRight)
}

/// Registers the global push-to-talk hotkey (Cmd+Shift+Space) on a dedicated
/// thread. rdev's `listen` blocks forever, so it can't share a thread with
/// anything else. Requires macOS Accessibility permission for this app.
pub fn register(app: AppHandle) {
    std::thread::spawn(move || {
        let pressed: Mutex<HashSet<Key>> = Mutex::new(HashSet::new());

        let result = listen(move |event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    let mut keys = pressed.lock().unwrap();
                    keys.insert(key);

                    let combo_active = keys.iter().any(is_cmd)
                        && keys.iter().any(is_shift)
                        && keys.contains(&Key::Space);

                    if combo_active && !TRIGGERED.swap(true, Ordering::SeqCst) {
                        toggle_recording(&app);
                    }
                }
                EventType::KeyRelease(key) => {
                    let mut keys = pressed.lock().unwrap();
                    keys.remove(&key);
                    if is_cmd(&key) || is_shift(&key) || key == Key::Space {
                        TRIGGERED.store(false, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        });

        if let Err(err) = result {
            eprintln!(
                "failed to register global hotkey (grant Accessibility permission in System Settings): {err:?}"
            );
        }
    });
}
