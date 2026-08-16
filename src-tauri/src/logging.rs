/// In-memory ring buffer backing the in-app log viewer (Settings > Logs).
/// Everything that used to be `eprintln!`-only debug output — audio device
/// lifecycle, mode/model resolution, LLM refinement/boilerplate results —
/// now also lands here via the `applog!` macro below, so a user reporting
/// "it didn't paste" or "wrong mode" can check what actually happened
/// without launching the app from a terminal to see stderr.
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub message: String,
}

static LOG_BUFFER: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Pushes a pre-formatted line into the ring buffer, evicting the oldest
/// entry once `MAX_LOG_ENTRIES` is reached. Called by `applog!`, not
/// meant to be called directly at sites that already have a message
/// string to format — use the macro instead so stderr output (still
/// useful during `cargo tauri dev`) isn't lost.
pub fn push(message: String) {
    let entry = LogEntry {
        timestamp_ms: now_ms(),
        message,
    };
    let mut buf = LOG_BUFFER.lock().unwrap();
    if buf.len() >= MAX_LOG_ENTRIES {
        buf.pop_front();
    }
    buf.push_back(entry);
}

pub fn snapshot() -> Vec<LogEntry> {
    LOG_BUFFER.lock().unwrap().iter().cloned().collect()
}

pub fn clear() {
    LOG_BUFFER.lock().unwrap().clear();
}

#[tauri::command]
pub fn get_logs() -> Vec<LogEntry> {
    snapshot()
}

#[tauri::command]
pub fn clear_logs() {
    clear();
}

/// Drop-in replacement for `eprintln!` that also records the formatted
/// line in the in-app log buffer. Exported at the crate root so any
/// module can call it as `crate::applog!(...)` without needing
/// `#[macro_use]`.
#[macro_export]
macro_rules! applog {
    ($($arg:tt)*) => {{
        let __applog_msg = format!($($arg)*);
        eprintln!("{__applog_msg}");
        $crate::logging::push(__applog_msg);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    // LOG_BUFFER is a single process-wide static, and cargo test runs
    // tests in parallel by default — so this is deliberately one test
    // covering the whole module rather than several `#[test]` fns, to
    // avoid one test's `clear()` racing another's `push()`.
    #[test]
    fn ring_buffer_and_macro_behavior() {
        clear();

        push("first".to_string());
        push("second".to_string());
        let snap = snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].message, "first");
        assert_eq!(snap[1].message, "second");

        clear();
        assert!(snapshot().is_empty());

        for i in 0..(MAX_LOG_ENTRIES + 10) {
            push(format!("entry {i}"));
        }
        let snap = snapshot();
        assert_eq!(snap.len(), MAX_LOG_ENTRIES);
        // The oldest 10 should have been evicted, so the buffer should
        // start at "entry 10".
        assert_eq!(snap[0].message, "entry 10");

        clear();
        let name = "whisper";
        crate::applog!("model warm-up skipped: {name}");
        let snap = snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].message, "model warm-up skipped: whisper");

        clear();
    }
}
