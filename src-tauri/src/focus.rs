//! Bringing another app's window to the front, and putting focus back.
//!
//! Uses the Accessibility API (the same permission pasting already needs):
//! `AXFrontmost` on the application activates it, and `AXRaise` on one of
//! its windows brings that window above its siblings. Window identity is
//! only the title, which is all AX exposes for a terminal: a pane or tab
//! inside a window is not addressable.

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;

type AXUIElementRef = *const c_void;
type AXError = i32;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
}

/// An unresponsive app must not hang a dictation: AX calls give up after this.
const AX_TIMEOUT_SECONDS: f32 = 1.0;

struct Element(CFType);

impl Element {
    fn application(pid: i32) -> Option<Element> {
        let raw = unsafe { AXUIElementCreateApplication(pid) };
        if raw.is_null() {
            return None;
        }
        let element = Element(unsafe { CFType::wrap_under_create_rule(raw as CFTypeRef) });
        unsafe { AXUIElementSetMessagingTimeout(element.raw(), AX_TIMEOUT_SECONDS) };
        Some(element)
    }

    fn raw(&self) -> AXUIElementRef {
        self.0.as_concrete_TypeRef() as AXUIElementRef
    }

    fn attribute(&self, name: &str) -> Option<CFType> {
        let key = CFString::new(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValue(self.raw(), key.as_concrete_TypeRef(), &mut out)
        };
        if err != 0 || out.is_null() {
            return None;
        }
        Some(unsafe { CFType::wrap_under_create_rule(out) })
    }

    fn set_bool(&self, name: &str, value: bool) -> bool {
        let key = CFString::new(name);
        let value = if value {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        unsafe {
            AXUIElementSetAttributeValue(
                self.raw(),
                key.as_concrete_TypeRef(),
                value.as_CFTypeRef(),
            ) == 0
        }
    }

    fn perform(&self, action: &str) -> bool {
        let action = CFString::new(action);
        unsafe { AXUIElementPerformAction(self.raw(), action.as_concrete_TypeRef()) == 0 }
    }

    fn title(&self) -> String {
        self.attribute("AXTitle")
            .and_then(|t| t.downcast_into::<CFString>())
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    fn windows(&self) -> Option<Vec<Element>> {
        let value = self.attribute("AXWindows")?;
        if value.type_of() != CFArray::<CFType>::type_id() {
            return None;
        }
        let array = unsafe {
            CFArray::<CFType>::wrap_under_get_rule(value.as_concrete_TypeRef() as *const _)
        };
        Some(array.iter().map(|w| Element((*w).clone())).collect())
    }

    fn raise(&self) {
        // A minimized window has to be restored before it can come forward.
        self.set_bool("AXMinimized", false);
        self.perform("AXRaise");
        self.set_bool("AXMain", true);
    }
}

/// Title of the app's focused window, if it has one.
pub fn focused_window_title(pid: i32) -> Option<String> {
    let app = Element::application(pid)?;
    let window = app.attribute("AXFocusedWindow")?;
    Some(Element(window).title())
}

/// Activates the app and raises one window, chosen by `choose` from the
/// current window titles. Returns the raised window's title. The choice and
/// the raise use the same window list, so a window opening in between can't
/// shift which one is raised.
pub fn raise_chosen(
    pid: i32,
    choose: &dyn Fn(&[String]) -> Result<usize, String>,
) -> Result<String, String> {
    let app = Element::application(pid).ok_or("the app isn't running any more")?;
    let windows = app
        .windows()
        .ok_or("couldn't read its windows (is Accessibility allowed?)")?;
    let titles: Vec<String> = windows.iter().map(Element::title).collect();
    let index = choose(&titles)?;
    let window = windows.get(index).ok_or("that window closed")?;
    if !app.set_bool("AXFrontmost", true) {
        return Err("couldn't bring the app to the front".to_string());
    }
    window.raise();
    Ok(titles[index].clone())
}

/// The app and window that had focus before a hop, so focus can return.
pub struct Snapshot {
    pid: i32,
    window: Option<Element>,
}

pub fn snapshot(pid: i32) -> Snapshot {
    let window = Element::application(pid)
        .and_then(|app| app.attribute("AXFocusedWindow"))
        .map(Element);
    Snapshot { pid, window }
}

/// Brings the snapshotted app back, and its window if it had a focused one.
pub fn restore(snapshot: &Snapshot) -> bool {
    let Some(app) = Element::application(snapshot.pid) else {
        return false;
    };
    let ok = app.set_bool("AXFrontmost", true);
    if let Some(window) = &snapshot.window {
        window.raise();
    }
    ok
}
