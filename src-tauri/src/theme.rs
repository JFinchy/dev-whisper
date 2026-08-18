use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Visual theme for the Settings window, and the accent color the floating
/// widget borrows for its non-recording highlights (mic-mode selection,
/// gear hover, the quick-actions flyout). Purely cosmetic — no field here
/// changes app behavior, only `config::AppConfig`'s other fields do that.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Dark graphite, amber accent, monospace-forward. Default — this is a
    /// local developer tool first, not a consumer dictation app.
    Terminal,
    /// Warm paper, teal accent, editorial hero.
    Signal,
    /// Light, restrained blue accent, no decoration.
    Quiet,
    /// Near-black, coral accent.
    Palette,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Terminal
    }
}

#[tauri::command]
pub fn get_theme(app: AppHandle) -> Theme {
    crate::config::load(&app).theme
}

#[tauri::command]
pub fn set_theme(app: AppHandle, theme: Theme) {
    let mut cfg = crate::config::load(&app);
    cfg.theme = theme;
    let _ = crate::config::save(&app, &cfg);
    // Settings and the widget are separate windows — this is how the
    // widget picks up an accent change made in Settings without a restart.
    let _ = app.emit("theme-changed", theme);
}

/// Settings window navigation shape — independent of `Theme` (color) and
/// purely about how sections are organized/reached. Both read the same
/// section components; only the chrome around them differs.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Left sidebar with one page of content on the right. Default — most
    /// section content is reachable in a single click.
    Sidebar,
    /// Horizontal pipeline of nodes (Input/Recognition/Mode/Refinement/
    /// Output/App); clicking a node expands a drawer beneath it.
    Chain,
}

impl Default for Layout {
    fn default() -> Self {
        Layout::Sidebar
    }
}

#[tauri::command]
pub fn get_layout(app: AppHandle) -> Layout {
    crate::config::load(&app).layout
}

#[tauri::command]
pub fn set_layout(app: AppHandle, layout: Layout) {
    let mut cfg = crate::config::load(&app);
    cfg.layout = layout;
    let _ = crate::config::save(&app, &cfg);
    let _ = app.emit("layout-changed", layout);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_terminal() {
        assert_eq!(Theme::default(), Theme::Terminal);
    }

    #[test]
    fn serializes_as_lowercase_for_the_frontend() {
        assert_eq!(serde_json::to_string(&Theme::Terminal).unwrap(), "\"terminal\"");
        assert_eq!(serde_json::to_string(&Theme::Signal).unwrap(), "\"signal\"");
        assert_eq!(serde_json::to_string(&Theme::Quiet).unwrap(), "\"quiet\"");
        assert_eq!(serde_json::to_string(&Theme::Palette).unwrap(), "\"palette\"");
    }

    #[test]
    fn default_layout_is_sidebar() {
        assert_eq!(Layout::default(), Layout::Sidebar);
    }

    #[test]
    fn layout_serializes_as_lowercase_for_the_frontend() {
        assert_eq!(serde_json::to_string(&Layout::Sidebar).unwrap(), "\"sidebar\"");
        assert_eq!(serde_json::to_string(&Layout::Chain).unwrap(), "\"chain\"");
    }
}
