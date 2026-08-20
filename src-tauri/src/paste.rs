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

/// Copies `text` to the clipboard without simulating a paste keystroke —
/// used by the "copy only" setting, and doesn't need Accessibility
/// permission since it never sends a synthetic keystroke.
pub fn copy_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())
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

    copy_text(text)?;

    // Give the OS a moment to register the clipboard update before pasting.
    thread::sleep(Duration::from_millis(50));

    // The Meta keydown needs to actually register as "held" with the OS
    // before the V keydown arrives, or the frontmost app sees a bare "v"
    // instead of Cmd+V — observed intermittently with `send`'s normal 20ms
    // gap. A longer settle specifically here (not on every event) targets
    // that race without slowing down the rest of the sequence.
    send(EventType::KeyPress(Key::MetaLeft))?;
    thread::sleep(Duration::from_millis(40));
    send(EventType::KeyPress(Key::KeyV))?;
    send(EventType::KeyRelease(Key::KeyV))?;
    send(EventType::KeyRelease(Key::MetaLeft))?;
    Ok(())
}

/// Simulates pressing Enter — used by the "press enter" voice command
/// (`punctuation::extract_press_enter`) after the transcript itself has
/// already been delivered. Requires Accessibility permission, same as
/// `paste_text`, since it's also a synthetic keystroke.
pub fn press_enter() -> Result<(), String> {
    if !accessibility_trusted() {
        return Err(
            "Accessibility permission not granted. Enable Dev Whisper in System Settings > \
             Privacy & Security > Accessibility, then try again."
                .to_string(),
        );
    }

    send(EventType::KeyPress(Key::Return))?;
    send(EventType::KeyRelease(Key::Return))?;
    Ok(())
}

fn send(event: EventType) -> Result<(), String> {
    simulate(&event).map_err(|e| format!("{e:?}"))?;
    thread::sleep(Duration::from_millis(20));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the real system clipboard rather than mocking it — save
    /// and restore whatever was already there so the test doesn't clobber
    /// the developer's actual clipboard contents.
    #[test]
    fn copy_text_writes_to_the_real_clipboard() {
        let mut clipboard = arboard::Clipboard::new().expect("clipboard unavailable");
        let previous = clipboard.get_text().ok();

        let marker = "dev-whisper-copy-only-test-marker";
        copy_text(marker).expect("copy_text failed");
        assert_eq!(clipboard.get_text().unwrap(), marker);

        if let Some(previous) = previous {
            let _ = clipboard.set_text(previous);
        }
    }
}
