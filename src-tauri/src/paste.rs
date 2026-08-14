use rdev::{simulate, EventType, Key};
use std::{thread, time::Duration};

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

#[cfg(not(target_os = "macos"))]
fn accessibility_trusted() -> bool {
    true
}

/// Copies `text` to the clipboard and simulates Cmd+V so it lands at the
/// active cursor position. Requires macOS Accessibility permission. Without
/// it, `simulate()` below returns Ok but the OS silently drops the
/// synthetic keystroke — checking first turns that into a visible error
/// instead of a paste that mysteriously never happens.
pub fn paste_text(text: &str) -> Result<(), String> {
    if !accessibility_trusted() {
        return Err(
            "Accessibility permission not granted. Enable Dev Whisper in System Settings > \
             Privacy & Security > Accessibility, then try again."
                .to_string(),
        );
    }

    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())?;

    // Give the OS a moment to register the clipboard update before pasting.
    thread::sleep(Duration::from_millis(50));

    send(EventType::KeyPress(Key::MetaLeft))?;
    send(EventType::KeyPress(Key::KeyV))?;
    send(EventType::KeyRelease(Key::KeyV))?;
    send(EventType::KeyRelease(Key::MetaLeft))?;
    Ok(())
}

fn send(event: EventType) -> Result<(), String> {
    simulate(&event).map_err(|e| format!("{e:?}"))?;
    thread::sleep(Duration::from_millis(20));
    Ok(())
}
