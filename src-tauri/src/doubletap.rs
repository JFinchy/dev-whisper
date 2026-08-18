/// Double-tap Fn to toggle recording — an alternative to the modifier+key
/// push-to-talk shortcut (`shortcut.rs`) for keyboards/users who'd rather
/// tap the Fn/Globe key twice, matching macOS's own "press Fn twice for
/// dictation" gesture. Runs alongside the existing shortcut rather than
/// replacing it.
///
/// This needs an entirely different capture mechanism than the existing
/// shortcut: `tauri-plugin-global-shortcut` (Carbon's `RegisterEventHotKey`)
/// requires a real modifier + key code and can't bind to a bare modifier
/// key like Fn, or detect a tap-tap pattern — it only fires a single
/// press/release for one registered combo. Fn key state only shows up
/// through raw global input monitoring, which `rdev` (already a dependency,
/// used for synthetic paste keystrokes in `paste.rs`) also provides via
/// `listen()`. Passive/listen-only on macOS (`CGEventTapOptionListenOnly`),
/// so it observes without consuming — Fn's normal OS behavior (dictation,
/// media keys, etc.) is unaffected — but it needs macOS's Input Monitoring
/// permission, a different TCC bucket than Accessibility (used for paste).
use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::config;

/// Two Fn presses within this window count as a double-tap. Long enough for
/// an intentional quick double-press, short enough that two unrelated taps
/// (Fn used normally, then again moments later for something else) aren't
/// misread as one.
const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(400);

/// Pulled out of the `rdev::listen` callback so the timing decision is
/// testable without a real event loop or `AppHandle`.
fn is_double_tap(last_tap: Option<Instant>, now: Instant) -> bool {
    last_tap.is_some_and(|previous| now.duration_since(previous) <= DOUBLE_TAP_WINDOW)
}

pub struct DoubleTapState {
    enabled: AtomicBool,
}

impl DoubleTapState {
    pub fn new(enabled: bool) -> Self {
        Self { enabled: AtomicBool::new(enabled) }
    }
}

/// Starts the background key-monitoring thread at most once per process.
/// `rdev::listen` blocks forever with no clean shutdown API, so rather than
/// starting/stopping an OS-level tap on every Settings toggle, it's spawned
/// once — lazily, the first time the feature is turned on (or eagerly at
/// launch if it was already on) — and left running for the app's lifetime.
/// Disabling the feature just makes the callback a no-op via
/// `DoubleTapState.enabled` instead of tearing down the tap.
static LISTENER_STARTED: Once = Once::new();

pub fn ensure_listener_started(app: &AppHandle) {
    LISTENER_STARTED.call_once(|| {
        let app = app.clone();
        std::thread::spawn(move || {
            let mut last_tap: Option<Instant> = None;

            let result = listen(move |event: Event| {
                if !matches!(event.event_type, EventType::KeyPress(Key::Function)) {
                    return;
                }

                let state = app.state::<DoubleTapState>();
                if !state.enabled.load(Ordering::SeqCst) {
                    return;
                }

                let now = Instant::now();

                if is_double_tap(last_tap, now) {
                    // Consumed — a third quick tap starts a fresh pair
                    // rather than immediately re-triggering.
                    last_tap = None;
                    crate::applog!("doubletap: Fn double-tap detected, toggling recording");
                    crate::recording::toggle_recording(&app);
                } else {
                    last_tap = Some(now);
                }
            });

            if let Err(err) = result {
                crate::applog!("doubletap: rdev::listen failed to start: {err:?}");
            }
        });
    });
}

#[tauri::command]
pub fn get_double_tap_enabled(app: AppHandle) -> bool {
    config::load(&app).double_tap_fn_enabled
}

#[tauri::command]
pub fn set_double_tap_enabled(app: AppHandle, enabled: bool, state: tauri::State<DoubleTapState>) {
    state.enabled.store(enabled, Ordering::SeqCst);
    if enabled {
        ensure_listener_started(&app);
    }

    let mut cfg = config::load(&app);
    cfg.double_tap_fn_enabled = enabled;
    let _ = config::save(&app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_previous_tap_is_not_a_double_tap() {
        assert!(!is_double_tap(None, Instant::now()));
    }

    #[test]
    fn a_second_tap_within_the_window_is_a_double_tap() {
        let now = Instant::now();
        let previous = now.checked_sub(Duration::from_millis(100)).unwrap();
        assert!(is_double_tap(Some(previous), now));
    }

    #[test]
    fn a_second_tap_right_at_the_window_edge_is_a_double_tap() {
        let now = Instant::now();
        let previous = now.checked_sub(DOUBLE_TAP_WINDOW).unwrap();
        assert!(is_double_tap(Some(previous), now));
    }

    #[test]
    fn a_second_tap_outside_the_window_is_not_a_double_tap() {
        let now = Instant::now();
        let previous = now.checked_sub(Duration::from_millis(401)).unwrap();
        assert!(!is_double_tap(Some(previous), now));
    }
}
