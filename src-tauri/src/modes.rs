use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::config;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(bundle_id: &str, mode: Mode) -> AppModeRule {
        AppModeRule {
            bundle_id: bundle_id.to_string(),
            app_name: bundle_id.to_string(),
            mode,
        }
    }

    #[test]
    fn no_bundle_id_resolves_to_plain() {
        assert_eq!(resolve_mode(None, &[]), Mode::Plain);
    }

    #[test]
    fn unknown_app_with_no_rules_resolves_to_plain() {
        assert_eq!(resolve_mode(Some("com.example.unknown"), &[]), Mode::Plain);
    }

    #[test]
    fn builtin_default_applies_when_no_user_rule() {
        assert_eq!(resolve_mode(Some("com.apple.Terminal"), &[]), Mode::Cli);
        assert_eq!(resolve_mode(Some("com.tinyspeck.slackmacgap"), &[]), Mode::Casual);
    }

    #[test]
    fn user_rule_overrides_builtin_default() {
        let rules = vec![rule("com.apple.Terminal", Mode::Casual)];
        assert_eq!(resolve_mode(Some("com.apple.Terminal"), &rules), Mode::Casual);
    }

    #[test]
    fn user_rule_applies_to_apps_without_a_builtin_default() {
        let rules = vec![rule("com.example.myapp", Mode::Cli)];
        assert_eq!(resolve_mode(Some("com.example.myapp"), &rules), Mode::Cli);
    }

    #[test]
    fn plain_and_casual_pass_transcript_through_unchanged() {
        let text = "  Hello,  world.  ";
        assert_eq!(apply_mode(Mode::Plain, text), text);
        assert_eq!(apply_mode(Mode::Casual, text), text);
    }

    #[test]
    fn cli_mode_formats_git_commit() {
        assert_eq!(
            apply_mode(Mode::Cli, "git commit update readme"),
            "git commit -m \"update readme\""
        );
        // Case-insensitive prefix match.
        assert_eq!(
            apply_mode(Mode::Cli, "Git Commit fix the bug."),
            "git commit -m \"fix the bug\""
        );
    }

    #[test]
    fn cli_mode_formats_mkdir_and_cd() {
        assert_eq!(apply_mode(Mode::Cli, "make directory src"), "mkdir src");
        assert_eq!(
            apply_mode(Mode::Cli, "change directory to src"),
            "cd src"
        );
    }

    #[test]
    fn cli_mode_passes_through_unmatched_text() {
        assert_eq!(
            apply_mode(Mode::Cli, "this doesn't match any pattern"),
            "this doesn't match any pattern"
        );
    }

    #[test]
    fn mode_serializes_as_snake_case() {
        assert_eq!(serde_json::to_string(&Mode::Cli).unwrap(), "\"cli\"");
        assert_eq!(serde_json::to_string(&Mode::Casual).unwrap(), "\"casual\"");
        assert_eq!(serde_json::to_string(&Mode::Plain).unwrap(), "\"plain\"");
    }
}

#[tauri::command]
pub fn get_mode_rules(app: AppHandle) -> Vec<AppModeRule> {
    config::load(&app).mode_rules
}

#[tauri::command]
pub fn set_mode_rule(app: AppHandle, bundle_id: String, app_name: String, mode: Mode) {
    eprintln!("modes: set_mode_rule bundle_id={bundle_id} app_name={app_name} mode={mode:?}");
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
