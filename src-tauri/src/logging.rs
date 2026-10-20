/// In-memory ring buffer backing the in-app log viewer (Settings > Logs).
/// Everything that used to be `eprintln!`-only debug output — audio device
/// lifecycle, mode/model resolution, LLM refinement/boilerplate results —
/// now also lands here via the `applog!` macro below, so a user reporting
/// "it didn't paste" or "wrong mode" can check what actually happened
/// without launching the app from a terminal to see stderr.
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub message: String,
}

static LOG_BUFFER: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());

/// Append-only file mirror of every `applog!` line, so logs survive an app
/// restart (the in-memory buffer above doesn't). Set once during setup.
static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// `app.log` is rotated to `app.log.1` at launch once it passes this size,
/// so a long-lived install keeps roughly the last two sessions' worth of
/// history instead of growing without bound.
const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;

pub fn init_file(path: PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    rotate_if_large(&path, MAX_LOG_FILE_BYTES);
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let _ = LOG_FILE.set(Mutex::new(file));
    Ok(())
}

fn rotate_if_large(path: &Path, max_bytes: u64) {
    let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > max_bytes);
    if too_big {
        let mut rotated = path.as_os_str().to_owned();
        rotated.push(".1");
        let _ = std::fs::rename(path, rotated);
    }
}

/// Routes panics into `app.log` before the default hook runs. A panic on a
/// background thread otherwise only reaches stderr, which nobody sees in a
/// release build, so the thread just silently stops doing its job.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let backtrace = std::backtrace::Backtrace::force_capture();
        push(format!("panic: thread '{name}' {info}\n{backtrace}"));
        default_hook(info);
    }));
}

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
    let timestamp_ms = now_ms();
    if let Some(file) = LOG_FILE.get() {
        if let Ok(mut file) = file.lock() {
            let _ = writeln!(file, "{timestamp_ms} {message}");
        }
    }
    let entry = LogEntry {
        timestamp_ms,
        message,
    };
    let mut buf = LOG_BUFFER.lock().unwrap_or_else(|e| e.into_inner());
    if buf.len() >= MAX_LOG_ENTRIES {
        buf.pop_front();
    }
    buf.push_back(entry);
}

pub fn snapshot() -> Vec<LogEntry> {
    LOG_BUFFER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

pub fn clear() {
    LOG_BUFFER.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[tauri::command]
pub fn get_logs() -> Vec<LogEntry> {
    snapshot()
}

#[tauri::command]
pub fn clear_logs() {
    clear();
}

/// Writes a line to stderr, ignoring failure. Never `eprintln!` here:
/// it panics when stderr is gone, and under `tauri dev` stderr is a pipe to
/// the Tauri CLI. Once the CLI exited, every log call panicked — killing
/// the heartbeat and signal threads, and aborting the whole app when it
/// happened on the main thread inside an AppKit callback.
pub fn emit(message: String) {
    let _ = writeln!(std::io::stderr(), "{message}");
    push(message);
}

/// Like `eprintln!`, but also records the line in the in-app log buffer
/// and `app.log`, and can never panic. Exported at the crate root so any
/// module can call it as `crate::applog!(...)` without `#[macro_use]`.
#[macro_export]
macro_rules! applog {
    ($($arg:tt)*) => {{
        $crate::logging::emit(format!($($arg)*));
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

    #[test]
    fn oversized_log_is_rotated_and_small_one_is_kept() {
        let dir = std::env::temp_dir().join(format!("dw-log-rotate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.log");

        std::fs::write(&path, "small").unwrap();
        rotate_if_large(&path, 100);
        assert!(path.exists());

        std::fs::write(&path, "x".repeat(200)).unwrap();
        rotate_if_large(&path, 100);
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("app.log.1"))
                .unwrap()
                .len(),
            200
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
