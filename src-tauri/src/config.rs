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

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub input_device: Option<String>,
    pub shortcut: Option<ShortcutConfig>,
    pub active_model: Option<String>,
    #[serde(default)]
    pub mode_rules: Vec<crate::modes::AppModeRule>,
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
            }],
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
    }
}
