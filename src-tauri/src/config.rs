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
