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

/// Full-text search across the *entire* retained history, not just the
/// most recent `MAX_ENTRIES_RETURNED` — the whole point is reaching back
/// further than the normal list view shows. A linear case-insensitive
/// substring scan rather than a real index: history is bounded by the
/// retention window (365 days max), so even a busy user's file is at most
/// a few thousand short lines — an index would be solving a problem this
/// app doesn't have yet.
#[tauri::command]
pub fn search_history_entries(app: AppHandle, query: String) -> Vec<HistoryEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return list_history_entries(app);
    }

    let mut entries: Vec<HistoryEntry> =
        read_all(&app).into_iter().filter(|e| entry_matches(e, &query)).collect();
    entries.reverse(); // most recent first
    entries.truncate(MAX_ENTRIES_RETURNED);
    entries
}

/// Pulled out of `search_history_entries` so the matching logic is
/// testable without a real `AppHandle` — `query_lower` is expected to
/// already be lowercased and non-empty (the caller handles the "empty
/// query = show everything" case before this ever runs).
fn entry_matches(entry: &HistoryEntry, query_lower: &str) -> bool {
    entry.text.to_lowercase().contains(query_lower)
        || entry.summary.as_deref().is_some_and(|s| s.to_lowercase().contains(query_lower))
        || entry.app_name.as_deref().is_some_and(|a| a.to_lowercase().contains(query_lower))
}

/// Re-runs `text` through a different Mode's formatting (and, if
/// requested, LLM refinement) without touching the stored entry — the
/// caller decides whether to just copy the result or persist it via
/// `update_history_entry_text`. Doesn't re-transcribe from audio: the raw
/// recording is a transient temp file (see `audio::write_wav`), never
/// retained past the original transcription, so "reprocessing" here means
/// re-formatting the saved transcript, not re-running Whisper.
#[tauri::command]
pub fn reprocess_history_text(
    app: AppHandle,
    text: String,
    mode: crate::modes::Behavior,
    use_llm_refinement: bool,
) -> Result<String, String> {
    let formatted = crate::modes::apply_mode(mode, &text);
    if use_llm_refinement {
        let cfg = config::load(&app);
        crate::llm::refine(mode, &formatted, &cfg.llm_model, None)
    } else {
        Ok(formatted)
    }
}

/// Overwrites a stored entry's text in place — used to persist the result
/// of `reprocess_history_text` rather than leaving it as a one-off copy.
/// Clears any existing journal summary, since it was written for the old
/// text and would otherwise silently describe content that's no longer
/// there.
#[tauri::command]
pub fn update_history_entry_text(app: AppHandle, timestamp_ms: u64, text: String) {
    let mut entries = read_all(&app);
    let Some(entry) = entries.iter_mut().find(|e| e.timestamp_ms == timestamp_ms) else {
        return;
    };
    entry.text = text;
    entry.summary = None;
    if write_all(&app, &entries).is_ok() {
        let _ = app.emit("history-updated", ());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, app_name: Option<&str>, summary: Option<&str>) -> HistoryEntry {
        HistoryEntry {
            timestamp_ms: 0,
            text: text.to_string(),
            app_name: app_name.map(str::to_string),
            mode: None,
            summary: summary.map(str::to_string),
            duration_ms: None,
            features_used: Vec::new(),
            spoken_words: None,
        }
    }

    #[test]
    fn matches_on_text() {
        let e = entry("call the CLIENT at three", None, None);
        assert!(entry_matches(&e, "client")); // entry text gets lowercased even though the query already is
        assert!(!entry_matches(&e, "invoice"));
    }

    #[test]
    fn matches_on_summary() {
        let e = entry("fixed the bug in the parser", None, Some("Fix parser bug"));
        assert!(entry_matches(&e, "parser bug"));
    }

    #[test]
    fn matches_on_app_name() {
        let e = entry("some text", Some("Terminal"), None);
        assert!(entry_matches(&e, "terminal"));
    }

    #[test]
    fn does_not_match_when_summary_and_app_name_are_absent() {
        let e = entry("some text", None, None);
        assert!(!entry_matches(&e, "missing"));
    }

    #[test]
    fn empty_query_never_reaches_entry_matches_but_would_match_everything_if_it_did() {
        // Documents the assumption entry_matches relies on: callers filter
        // out the empty-query case themselves (see search_history_entries).
        let e = entry("anything", None, None);
        assert!(entry_matches(&e, ""));
    }
}
