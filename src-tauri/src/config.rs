use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ShortcutConfig {
    pub meta: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub code: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            meta: true,
            ctrl: false,
            alt: false,
            shift: true,
            code: "Space".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub input_device: Option<String>,
    pub shortcut: Option<ShortcutConfig>,
    pub active_model: Option<String>,
    #[serde(default)]
    pub mode_rules: Vec<crate::modes::AppModeRule>,
    /// Developer-jargon terms biasing Whisper's recognition. Defaults to
    /// `stt::default_vocabulary()` both for fresh installs (via `impl
    /// Default`) and for configs saved before this field existed (via
    /// the serde default below) — never silently empty.
    #[serde(default = "crate::stt::default_vocabulary")]
    pub vocabulary: Vec<String>,
    /// How many days of transcript history to keep before auto-purging.
    /// Conservative default (not "forever") since dictated text can
    /// contain sensitive content.
    #[serde(default = "default_history_retention_days")]
    pub history_retention_days: u32,
    /// Which locally-pulled Ollama model to use for LLM refinement (see
    /// `llm.rs`). Only used for modes with `use_llm_refinement: true`.
    #[serde(default = "crate::llm::default_model")]
    pub llm_model: String,
    /// Last dragged position of the widget window (logical coordinates),
    /// so it reopens where the user left it instead of always
    /// re-centering. `None` until the user drags it at least once.
    #[serde(default)]
    pub widget_position: Option<(f64, f64)>,
    /// When true, transcripts are copied to the clipboard but never pasted
    /// via a simulated keystroke — for users who'd rather paste manually
    /// (e.g. Cmd+V themselves) or whose setup makes the synthetic keystroke
    /// unreliable. Also means Accessibility permission isn't needed.
    #[serde(default)]
    pub copy_only: bool,
    /// Which of the widget's display modes (minimal/compact/detailed) is
    /// active — see widget.rs.
    #[serde(default)]
    pub widget_mode: crate::widget::WidgetMode,
    /// When true, each pasted dictation gets a one-line LLM-generated
    /// summary attached in History (see `llm::summarize_for_journal`),
    /// turning the raw transcript log into a scannable work journal. Off
    /// by default — it's an extra background LLM call per dictation, and
    /// this app defaults background LLM/system activity to opt-in (see
    /// `copy_only`, autostart).
    #[serde(default)]
    pub journal_enabled: bool,
    /// Fires a `POST` with the delivered transcript to this URL after every
    /// successful paste — a generic primitive covering Notion/Slack/n8n/
    /// Zapier/Make.com/webhook.site, since all of them accept incoming
    /// webhooks natively (see `webhook.rs`). `None`/empty disables it.
    /// Off by default: sending dictated text off-device is a deliberate
    /// opt-in, matching `copy_only`/autostart/`journal_enabled`.
    #[serde(default)]
    pub webhook_url: Option<String>,
}

fn default_history_retention_days() -> u32 {
    30
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            input_device: None,
            shortcut: None,
            active_model: None,
            mode_rules: Vec::new(),
            vocabulary: crate::stt::default_vocabulary(),
            history_retention_days: default_history_retention_days(),
            llm_model: crate::llm::default_model(),
            widget_position: None,
            copy_only: false,
            widget_mode: crate::widget::WidgetMode::default(),
            journal_enabled: false,
            webhook_url: None,
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

pub fn load(app: &AppHandle) -> AppConfig {
    config_path(app)
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_config_round_trips() {
        let original = ShortcutConfig::default();
        let json = serde_json::to_string(&original).unwrap();
        let restored: ShortcutConfig = serde_json::from_str(&json).unwrap();
        assert!(restored == original);
    }

    #[test]
    fn app_config_round_trips_with_all_fields_set() {
        let original = AppConfig {
            input_device: Some("Test Mic".to_string()),
            shortcut: Some(ShortcutConfig::default()),
            active_model: Some("base.en".to_string()),
            mode_rules: vec![crate::modes::AppModeRule {
                bundle_id: "com.apple.Terminal".to_string(),
                app_name: "Terminal".to_string(),
                mode: crate::modes::Mode::Cli,
                stt_model: None,
                use_llm_refinement: false,
            }],
            ..AppConfig::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.input_device, original.input_device);
        assert_eq!(restored.active_model, original.active_model);
        assert!(restored.shortcut.is_some());
        assert_eq!(restored.mode_rules.len(), 1);
        assert_eq!(restored.mode_rules[0].bundle_id, "com.apple.Terminal");
    }

    #[test]
    fn missing_mode_rules_field_defaults_to_empty() {
        // Simulates loading a config.json saved before mode_rules existed
        // — #[serde(default)] on that field must keep old configs loadable.
        let json = r#"{"input_device":null,"shortcut":null,"active_model":null}"#;
        let restored: AppConfig = serde_json::from_str(json).unwrap();
        assert!(restored.mode_rules.is_empty());
    }

    #[test]
    fn bare_json_object_deserializes_to_all_defaults() {
        // serde's derive treats missing `Option<T>` fields as `None`
        // even without an explicit #[serde(default)], and `mode_rules`
        // has an explicit one — so a config.json from before any of
        // these fields existed still loads cleanly instead of needing
        // `load()`'s error fallback to paper over it.
        let restored: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(restored.input_device.is_none());
        assert!(restored.shortcut.is_none());
        assert!(restored.active_model.is_none());
        assert!(restored.mode_rules.is_empty());
        assert_eq!(restored.vocabulary, crate::stt::default_vocabulary());
        assert_eq!(restored.history_retention_days, 30);
        assert!(!restored.copy_only);
        assert_eq!(restored.widget_mode, crate::widget::WidgetMode::default());
        assert!(!restored.journal_enabled);
        assert!(restored.webhook_url.is_none());
    }
}
