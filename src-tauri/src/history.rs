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
    /// One-line "what this was about" summary for the work journal (see
    /// `llm::summarize_for_journal`), filled in by a background LLM call
    /// after the entry is written — never blocks the paste. `None` until
    /// that call finishes (or if journaling is off / the call failed /
    /// the entry predates this field).
    #[serde(default)]
    pub summary: Option<String>,
    /// Recording length, read from the wav header (see
    /// `stt::wav_duration_ms`) — feeds the words-per-minute stat in
    /// Insights. `None` for entries predating this field, or if the wav
    /// header couldn't be read; never fatal to logging the entry itself.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Which pre-LLM deterministic passes actually fired on this dictation
    /// — "lists", "punctuation", "backtrack", "press_enter" (see
    /// `recording::transcribe_and_paste`). Casing/snippet/boilerplate
    /// aren't repeated here since `mode` already names those. Feeds the
    /// Insights feature-adoption checklist ("have you tried Backtrack?").
    #[serde(default)]
    pub features_used: Vec<String>,
    /// Word count of what was actually *spoken* (post punctuation/list/
    /// backtrack expansion, pre snippet/casing/boilerplate substitution) —
    /// deliberately not derived from `text` at read time, since `text` is
    /// the *delivered* content: for a snippet or boilerplate dictation
    /// that can be many more/fewer words than what the user said (a
    /// two-word snippet trigger expanding to a multi-line checklist would
    /// wildly inflate a words-per-minute stat computed off `text` alone).
    /// `None` for entries predating this field — Insights falls back to
    /// counting `text` for those, the best available approximation.
    #[serde(default)]
    pub spoken_words: Option<u32>,
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
/// Returns the timestamp it stored the entry under, so a caller that wants
/// to fill in `summary` afterward (see `set_entry_summary`) has a key to
/// find it again without re-reading the whole file first.
pub fn append_entry(
    app: &AppHandle,
    text: &str,
    app_name: Option<String>,
    mode: Option<String>,
    duration_ms: Option<u64>,
    features_used: Vec<String>,
    spoken_words: Option<u32>,
) -> u64 {
    let timestamp_ms = now_ms();
    let Ok(path) = history_path(app) else {
        return timestamp_ms;
    };
    let entry = HistoryEntry {
        timestamp_ms,
        text: text.to_string(),
        app_name,
        mode,
        summary: None,
        duration_ms,
        features_used,
        spoken_words,
    };
    let Ok(line) = serde_json::to_string(&entry) else {
        return timestamp_ms;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
        // The Settings window (if open) has no other way to know a new
        // entry landed — it only fetches on mount / explicit refresh.
        let _ = app.emit("history-updated", ());
    }
    timestamp_ms
}

/// Fills in a journal summary generated after the fact (see
/// `llm::summarize_for_journal`) for the entry stored under
/// `timestamp_ms`. A no-op if that entry has since been deleted/purged —
/// the background summarization call can finish after the user cleared
/// their history, and that shouldn't resurrect it.
pub fn set_entry_summary(app: &AppHandle, timestamp_ms: u64, summary: String) {
    let mut entries = read_all(app);
    let Some(entry) = entries.iter_mut().find(|e| e.timestamp_ms == timestamp_ms) else {
        return;
    };
    entry.summary = Some(summary);
    if write_all(app, &entries).is_ok() {
        let _ = app.emit("history-updated", ());
    }
}

/// All retained entries, oldest first, uncapped — unlike
/// `list_history_entries`, which truncates to `MAX_ENTRIES_RETURNED` for
/// the History list UI. Insights aggregation (total words, streaks, app
/// usage) needs the real count, not just the most recent page of it.
pub fn all_entries(app: &AppHandle) -> Vec<HistoryEntry> {
    read_all(app)
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
pub fn delete_history_entry(app: AppHandle, timestamp_ms: u64) {
    let entries = read_all(&app);
    let kept: Vec<HistoryEntry> = entries
        .into_iter()
        .filter(|e| e.timestamp_ms != timestamp_ms)
        .collect();
    let _ = write_all(&app, &kept);
}

#[tauri::command]
pub fn get_journal_enabled(app: AppHandle) -> bool {
    config::load(&app).journal_enabled
}

#[tauri::command]
pub fn set_journal_enabled(app: AppHandle, enabled: bool) {
    let mut cfg = config::load(&app);
    cfg.journal_enabled = enabled;
    let _ = config::save(&app, &cfg);
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
