/// Double-tap Fn to toggle recording — an alternative to the modifier+key
/// push-to-talk shortcut (`shortcut.rs`) for keyboards/users who'd rather
/// tap the Fn/Globe key twice, matching macOS's own "press Fn twice for
/// dictation" gesture. Runs alongside the existing shortcut rather than
/// replacing it.
///
/// This needs a different capture mechanism than the existing shortcut:
/// `tauri-plugin-global-shortcut` (Carbon's `RegisterEventHotKey`) needs a
/// real modifier + key code and can't bind a bare Fn press or a tap-tap
/// pattern. Fn only shows up as a modifier-flags change, so this installs a
/// listen-only CoreGraphics event tap for `FlagsChanged` events alone. It
/// observes without consuming (Fn's normal OS behavior is unaffected) and
/// needs macOS's Input Monitoring permission.
///
/// Not `rdev::listen`: rdev converts *every* key event to a character via
/// `TSMGetInputSourceProperty` on its background thread, and current macOS
/// asserts that API runs on the main thread, killing the app with SIGTRAP.
/// Our own synthetic Cmd+V paste was enough to trigger it.
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
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
        Self {
            enabled: AtomicBool::new(enabled),
        }
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

/// macOS virtual key code for Fn/Globe (`kVK_Function`).
const FN_KEY_CODE: i64 = 63;

extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

/// The tap's mach port, so the callback can re-enable the tap if macOS
/// disables it (slow callback, or secure keyboard entry turned on).
static TAP_PORT: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

/// True for the flags-changed event of Fn going *down* (key code 63 with
/// the secondary-Fn flag set); its release has the flag cleared.
fn is_fn_press(key_code: i64, flags: CGEventFlags) -> bool {
    key_code == FN_KEY_CODE && flags.contains(CGEventFlags::CGEventFlagSecondaryFn)
}

pub fn ensure_listener_started(app: &AppHandle) {
    LISTENER_STARTED.call_once(|| {
        let app = app.clone();
        std::thread::spawn(move || {
            let last_tap = std::sync::Mutex::new(None::<Instant>);
            let tap = CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::TailAppendEventTap,
                CGEventTapOptions::ListenOnly,
                vec![CGEventType::FlagsChanged],
                move |_proxy, event_type, event| {
                    if matches!(event_type, CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput) {
                        crate::applog!("doubletap: event tap was disabled by macOS ({event_type:?}), re-enabling");
                        let port = TAP_PORT.load(Ordering::SeqCst);
                        if !port.is_null() {
                            unsafe { CGEventTapEnable(port as CFMachPortRef, true) };
                        }
                        return CallbackResult::Keep;
                    }

                    let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    if !is_fn_press(key_code, event.get_flags()) {
                        return CallbackResult::Keep;
                    }
                    if !app.state::<DoubleTapState>().enabled.load(Ordering::SeqCst) {
                        return CallbackResult::Keep;
                    }

                    let now = Instant::now();
                    let mut last = last_tap.lock().unwrap();
                    if is_double_tap(*last, now) {
                        // Consumed — a third quick tap starts a fresh pair
                        // rather than immediately re-triggering.
                        *last = None;
                        crate::applog!("doubletap: Fn double-tap detected, toggling recording");
                        // Off the tap's thread: stopping a recording blocks
                        // on writing the WAV, and macOS disables a tap whose
                        // callback stalls.
                        let app = app.clone();
                        std::thread::spawn(move || {
                            crate::recording::toggle_recording(&app, crate::capture::Trigger::DoubleTapFn);
                        });
                    } else {
                        *last = Some(now);
                    }
                    CallbackResult::Keep
                },
            );

            let Ok(tap) = tap else {
                crate::applog!("doubletap: couldn't create the key event tap (Input Monitoring not granted?)");
                return;
            };
            TAP_PORT.store(tap.mach_port().as_concrete_TypeRef() as *mut _, Ordering::SeqCst);
            let source = match tap.mach_port().create_runloop_source(0) {
                Ok(source) => source,
                Err(()) => {
                    crate::applog!("doubletap: couldn't attach the key event tap to a run loop");
                    return;
                }
            };
            CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
            tap.enable();
            crate::applog!("doubletap: listening for Fn double-tap");
            CFRunLoop::run_current();
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

    #[test]
    fn fn_press_is_key_code_63_with_the_fn_flag_set() {
        assert!(is_fn_press(
            FN_KEY_CODE,
            CGEventFlags::CGEventFlagSecondaryFn
        ));
        assert!(
            !is_fn_press(FN_KEY_CODE, CGEventFlags::empty()),
            "Fn release"
        );
        assert!(
            !is_fn_press(
                56,
                CGEventFlags::CGEventFlagSecondaryFn | CGEventFlags::CGEventFlagShift
            ),
            "Shift pressed while Fn is held"
        );
    }
}
