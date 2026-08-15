use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use crate::config;

/// Cap on how many entries the UI ever pulls at once — the file itself can
/// hold more (purged only by retention window), this just bounds one read.
const MAX_ENTRIES_RETURNED: usize = 200;

#[derive(Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub timestamp_ms: u64,
    pub text: String,
    pub app_name: Option<String>,
    pub mode: Option<String>,
}

fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("history.jsonl"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn read_all(app: &AppHandle) -> Vec<HistoryEntry> {
    let Ok(path) = history_path(app) else {
        return Vec::new();
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn write_all(app: &AppHandle, entries: &[HistoryEntry]) -> Result<(), String> {
    let path = history_path(app)?;
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    for entry in entries {
        let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Appends one entry — only called after a transcript actually got pasted,
/// so history reflects what really reached the user, not failed attempts.
pub fn append_entry(app: &AppHandle, text: &str, app_name: Option<String>, mode: Option<String>) {
    let Ok(path) = history_path(app) else {
        return;
    };
    let entry = HistoryEntry {
        timestamp_ms: now_ms(),
        text: text.to_string(),
        app_name,
        mode,
    };
    let Ok(line) = serde_json::to_string(&entry) else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
        // The Settings window (if open) has no other way to know a new
        // entry landed — it only fetches on mount / explicit refresh.
        let _ = app.emit("history-updated", ());
    }
}

/// Deletes entries older than the configured retention window. Called at
/// startup; also safe to call after the user changes the retention setting.
pub fn purge_old_entries(app: &AppHandle, retention_days: u32) {
    let cutoff_ms = now_ms().saturating_sub(retention_days as u64 * 24 * 60 * 60 * 1000);
    let entries = read_all(app);
    let kept: Vec<HistoryEntry> = entries
        .into_iter()
        .filter(|e| e.timestamp_ms >= cutoff_ms)
        .collect();
    let _ = write_all(app, &kept);
}

#[tauri::command]
pub fn list_history_entries(app: AppHandle) -> Vec<HistoryEntry> {
    let mut entries = read_all(&app);
    entries.reverse(); // most recent first
    entries.truncate(MAX_ENTRIES_RETURNED);
    entries
}

#[tauri::command]
pub fn clear_history(app: AppHandle) {
    let _ = write_all(&app, &[]);
}

#[tauri::command]
pub fn get_history_retention_days(app: AppHandle) -> u32 {
    config::load(&app).history_retention_days
}

#[tauri::command]
pub fn set_history_retention_days(app: AppHandle, days: u32) {
    let mut cfg = config::load(&app);
    cfg.history_retention_days = days;
    let _ = config::save(&app, &cfg);
    purge_old_entries(&app, days);
}
