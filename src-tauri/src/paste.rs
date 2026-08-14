use rdev::{simulate, EventType, Key};
use std::{thread, time::Duration};

/// Copies `text` to the clipboard and simulates Cmd+V so it lands at the
/// active cursor position. Requires macOS Accessibility permission (same
/// grant used by the global hotkey listener).
pub fn paste_text(text: &str) -> Result<(), String> {
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
