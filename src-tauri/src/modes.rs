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
    /// Whisper model id (matching `models::CATALOG`) to use for this app,
    /// overriding the globally-active model. `None` = use whatever's
    /// globally active.
    #[serde(default)]
    pub stt_model: Option<String>,
    /// Forward-looking toggle for the LLM refinement pipeline (see
    /// BACKLOG.md) — not yet wired to an actual LLM call.
    #[serde(default)]
    pub use_llm_refinement: bool,
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

pub struct ResolvedSettings {
    pub mode: Mode,
    /// Per-mode Whisper model override, settable in Settings — not yet
    /// wired to actually switch WhisperEngine's active model. Doing that
    /// naively would reload the whisper context (and repay the multi-
    /// second Metal shader compile) on every recording that hits a
    /// different-model rule, so it needs a real design pass (e.g. a small
    /// LRU of warm contexts) rather than a blind wire-up. See BACKLOG.md.
    #[allow(dead_code)]
    pub stt_model: Option<String>,
    pub use_llm_refinement: bool,
}

/// Full resolution (mode + per-mode overrides), used by the recording
/// pipeline. `resolve_mode` below is a thin wrapper kept for callers (and
/// tests) that only care about the mode.
pub fn resolve_settings(bundle_id: Option<&str>, rules: &[AppModeRule]) -> ResolvedSettings {
    let Some(bundle_id) = bundle_id else {
        return ResolvedSettings {
            mode: Mode::Plain,
            stt_model: None,
            use_llm_refinement: false,
        };
    };
    if let Some(rule) = rules.iter().find(|r| r.bundle_id == bundle_id) {
        return ResolvedSettings {
            mode: rule.mode,
            stt_model: rule.stt_model.clone(),
            use_llm_refinement: rule.use_llm_refinement,
        };
    }
    let mode = BUILTIN_DEFAULTS
        .iter()
        .find(|(id, _)| *id == bundle_id)
        .map(|(_, mode)| *mode)
        .unwrap_or(Mode::Plain);
    ResolvedSettings {
        mode,
        stt_model: None,
        use_llm_refinement: false,
    }
}

/// Thin convenience wrapper around `resolve_settings` for callers (mainly
/// tests) that only care about the mode.
#[allow(dead_code)]
pub fn resolve_mode(bundle_id: Option<&str>, rules: &[AppModeRule]) -> Mode {
    resolve_settings(bundle_id, rules).mode
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
            stt_model: None,
            use_llm_refinement: false,
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
pub fn set_mode_rule(
    app: AppHandle,
    bundle_id: String,
    app_name: String,
    mode: Mode,
    stt_model: Option<String>,
    use_llm_refinement: bool,
) {
    eprintln!("modes: set_mode_rule bundle_id={bundle_id} app_name={app_name} mode={mode:?} stt_model={stt_model:?} use_llm_refinement={use_llm_refinement}");
    let mut cfg = config::load(&app);
    cfg.mode_rules.retain(|r| r.bundle_id != bundle_id);
    cfg.mode_rules.push(AppModeRule {
        bundle_id,
        app_name,
        mode,
        stt_model,
        use_llm_refinement,
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
pub struct RunningAppPayload {
    pub bundle_id: String,
    pub name: String,
}

/// Lists currently-running apps for the "browse running apps" picker in
/// Settings, so adding a mode rule doesn't require switching to the target
/// app first. NSWorkspace requires the main thread; the command's own
/// thread blocks on a channel waiting for that dispatch to run.
#[tauri::command]
pub fn list_running_apps(app: AppHandle) -> Vec<RunningAppPayload> {
    let own_bundle_id = app.config().identifier.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let dispatched = app.run_on_main_thread(move || {
        let apps = crate::app_detect::list_running_apps(&own_bundle_id);
        let _ = tx.send(apps);
    });
    if dispatched.is_err() {
        return Vec::new();
    }
    rx.recv()
        .unwrap_or_default()
        .into_iter()
        .map(|info| RunningAppPayload {
            bundle_id: info.bundle_id,
            name: info.name,
        })
        .collect()
}
