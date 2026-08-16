use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// How much the floating widget shows. The window itself isn't
/// user-resizable (no titlebar, no drag handles — see tauri.conf.json), so
/// switching modes resizes it programmatically instead.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WidgetMode {
    /// Icon-only recording button. No status text, no settings button —
    /// errors surface as a color change plus a native tooltip on hover.
    /// Settings is still reachable via the tray menu.
    Minimal,
    /// The original pill: button, one line of status/flash text, settings
    /// gear. Default. Auto-grows temporarily (see WidgetView.tsx) when an
    /// error/status message is too long to fit on one line, instead of
    /// silently truncating it.
    Compact,
    /// Larger fixed panel: full status line, a persistent (not
    /// auto-dismissing) wrapped message area, and the active app-mode —
    /// for when you want the detail visible at a glance rather than
    /// digging through Settings > Logs after the fact.
    Detailed,
}

impl Default for WidgetMode {
    fn default() -> Self {
        WidgetMode::Compact
    }
}

impl WidgetMode {
    /// Logical-pixel size for the base state of each mode. Compact grows
    /// further at runtime (frontend-driven) while an error/status message
    /// is showing; this is its resting size.
    pub fn base_size(self) -> (f64, f64) {
        match self {
            WidgetMode::Minimal => (46.0, 46.0),
            WidgetMode::Compact => (220.0, 60.0),
            WidgetMode::Detailed => (320.0, 170.0),
        }
    }
}

pub fn apply_size(app: &AppHandle, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window("widget") {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }
}

#[tauri::command]
pub fn get_widget_mode(app: AppHandle) -> WidgetMode {
    crate::config::load(&app).widget_mode
}

#[tauri::command]
pub fn set_widget_mode(app: AppHandle, mode: WidgetMode) {
    let mut cfg = crate::config::load(&app);
    cfg.widget_mode = mode;
    let _ = crate::config::save(&app, &cfg);

    let (width, height) = mode.base_size();
    apply_size(&app, width, height);
    let _ = app.emit("widget-mode-changed", mode);
}

/// Lets the widget itself request a temporary size override (compact
/// mode's auto-grow for long messages) without going through the
/// persisted-mode path — this doesn't change the saved `widget_mode` or
/// notify other windows, it's purely "resize my own window right now".
#[tauri::command]
pub fn set_widget_size(app: AppHandle, width: f64, height: f64) {
    apply_size(&app, width, height);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_widget_mode_is_compact() {
        assert_eq!(WidgetMode::default(), WidgetMode::Compact);
    }

    #[test]
    fn serializes_as_lowercase_for_the_frontend() {
        assert_eq!(serde_json::to_string(&WidgetMode::Minimal).unwrap(), "\"minimal\"");
        assert_eq!(serde_json::to_string(&WidgetMode::Compact).unwrap(), "\"compact\"");
        assert_eq!(serde_json::to_string(&WidgetMode::Detailed).unwrap(), "\"detailed\"");
    }

    #[test]
    fn each_mode_has_a_distinct_base_size() {
        let sizes = [
            WidgetMode::Minimal.base_size(),
            WidgetMode::Compact.base_size(),
            WidgetMode::Detailed.base_size(),
        ];
        assert_ne!(sizes[0], sizes[1]);
        assert_ne!(sizes[1], sizes[2]);
        assert_ne!(sizes[0], sizes[2]);
    }
}
