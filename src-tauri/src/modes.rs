use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, State};

use crate::app_detect::AppInfo;
use crate::config;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// No transformation — pastes the raw Whisper transcript.
    Plain,
    /// Reserved for casual/conversational apps (Slack, Messages). No
    /// rule-based transform yet; this is the hook for LLM refinement
    /// (see BACKLOG.md) — today it behaves the same as Plain.
    Casual,
    /// Terminal apps: a handful of illustrative natural-language ->
    /// shell-command patterns. Intentionally narrow — general NL-to-CLI
    /// translation needs the LLM refinement pipeline, not regex.
    Cli,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppModeRule {
    pub bundle_id: String,
    pub app_name: String,
    pub mode: Mode,
}

/// Bundle IDs this app knows a sensible default mode for out of the box.
/// User rules (persisted in config) always take priority over these.
const BUILTIN_DEFAULTS: &[(&str, Mode)] = &[
    ("com.apple.Terminal", Mode::Cli),
    ("com.googlecode.iterm2", Mode::Cli),
    ("net.kovidgoyal.kitty", Mode::Cli),
    ("com.github.wez.wezterm", Mode::Cli),
    ("dev.warp.Warp-Stable", Mode::Cli),
    ("com.apple.MobileSMS", Mode::Casual),
    ("com.tinyspeck.slackmacgap", Mode::Casual),
    ("com.hnc.Discord", Mode::Casual),
];

pub struct ModesState {
    /// Snapshot of the frontmost app taken right before the settings
    /// window opens (by then focus has already moved to Dev Whisper),
    /// so the UI can offer "add a rule for the app you just came from".
    pub last_frontmost: Mutex<Option<AppInfo>>,
}

pub fn resolve_mode(bundle_id: Option<&str>, rules: &[AppModeRule]) -> Mode {
    let Some(bundle_id) = bundle_id else {
        return Mode::Plain;
    };
    if let Some(rule) = rules.iter().find(|r| r.bundle_id == bundle_id) {
        return rule.mode;
    }
    BUILTIN_DEFAULTS
        .iter()
        .find(|(id, _)| *id == bundle_id)
        .map(|(_, mode)| *mode)
        .unwrap_or(Mode::Plain)
}

pub fn apply_mode(mode: Mode, transcript: &str) -> String {
    match mode {
        Mode::Plain | Mode::Casual => transcript.to_string(),
        Mode::Cli => format_as_cli(transcript),
    }
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    if text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(text[prefix.len()..].trim())
    } else {
        None
    }
}

fn format_as_cli(text: &str) -> String {
    let trimmed = text.trim().trim_end_matches('.');

    if let Some(message) = strip_prefix_ci(trimmed, "git commit ") {
        return format!("git commit -m \"{message}\"");
    }
    if let Some(dir) = strip_prefix_ci(trimmed, "make directory ") {
        return format!("mkdir {dir}");
    }
    if let Some(dir) = strip_prefix_ci(trimmed, "change directory to ") {
        return format!("cd {dir}");
    }

    trimmed.to_string()
}

#[tauri::command]
pub fn get_mode_rules(app: AppHandle) -> Vec<AppModeRule> {
    config::load(&app).mode_rules
}

#[tauri::command]
pub fn set_mode_rule(app: AppHandle, bundle_id: String, app_name: String, mode: Mode) {
    let mut cfg = config::load(&app);
    cfg.mode_rules.retain(|r| r.bundle_id != bundle_id);
    cfg.mode_rules.push(AppModeRule {
        bundle_id,
        app_name,
        mode,
    });
    let _ = config::save(&app, &cfg);
}

#[tauri::command]
pub fn remove_mode_rule(app: AppHandle, bundle_id: String) {
    let mut cfg = config::load(&app);
    cfg.mode_rules.retain(|r| r.bundle_id != bundle_id);
    let _ = config::save(&app, &cfg);
}

#[derive(Serialize)]
pub struct FrontmostAppPayload {
    pub bundle_id: String,
    pub name: String,
}

#[tauri::command]
pub fn get_last_frontmost_app(state: State<ModesState>) -> Option<FrontmostAppPayload> {
    state
        .last_frontmost
        .lock()
        .unwrap()
        .as_ref()
        .map(|info| FrontmostAppPayload {
            bundle_id: info.bundle_id.clone(),
            name: info.name.clone(),
        })
}
