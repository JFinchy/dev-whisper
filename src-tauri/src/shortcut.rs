use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::config::{self, ShortcutConfig};

/// Named `PushToTalk...` to avoid clashing with
/// `tauri_plugin_global_shortcut::ShortcutState` (the press/release enum).
pub struct PushToTalkState {
    pub current: Mutex<ShortcutConfig>,
}

pub fn to_shortcut(cfg: &ShortcutConfig) -> Result<Shortcut, String> {
    let code = parse_code(&cfg.code).ok_or_else(|| format!("unsupported key: {}", cfg.code))?;

    let mut modifiers = Modifiers::empty();
    if cfg.meta {
        modifiers |= Modifiers::SUPER;
    }
    if cfg.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if cfg.alt {
        modifiers |= Modifiers::ALT;
    }
    if cfg.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if modifiers.is_empty() {
        return Err("shortcut needs at least one modifier key".to_string());
    }

    Ok(Shortcut::new(Some(modifiers), code))
}

#[tauri::command]
pub fn get_shortcut(state: State<PushToTalkState>) -> ShortcutConfig {
    state.current.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    new_cfg: ShortcutConfig,
    state: State<PushToTalkState>,
) -> Result<ShortcutConfig, String> {
    let new_shortcut = to_shortcut(&new_cfg)?;

    let mut current = state.current.lock().unwrap();
    let old_shortcut = to_shortcut(&current)?;

    if new_shortcut != old_shortcut {
        app.global_shortcut()
            .unregister(old_shortcut)
            .map_err(|e| e.to_string())?;
        if let Err(err) = app.global_shortcut().register(new_shortcut) {
            // best-effort restore so the app doesn't end up with no hotkey
            let _ = app.global_shortcut().register(old_shortcut);
            return Err(err.to_string());
        }
    }

    *current = new_cfg.clone();
    drop(current);

    let mut cfg = config::load(&app);
    cfg.shortcut = Some(new_cfg.clone());
    let _ = config::save(&app, &cfg);

    Ok(new_cfg)
}

/// Maps a JS `KeyboardEvent.code` string to the matching `Code` variant.
/// Covers the keys realistic for a push-to-talk shortcut; unrecognized
/// codes surface as an error in the UI rather than silently no-op-ing.
fn parse_code(code: &str) -> Option<Code> {
    use Code::*;
    Some(match code {
        "Space" => Space,
        "Tab" => Tab,
        "Escape" => Escape,
        "Enter" => Enter,
        "Backspace" => Backspace,
        "CapsLock" => CapsLock,
        "ArrowUp" => ArrowUp,
        "ArrowDown" => ArrowDown,
        "ArrowLeft" => ArrowLeft,
        "ArrowRight" => ArrowRight,
        "KeyA" => KeyA,
        "KeyB" => KeyB,
        "KeyC" => KeyC,
        "KeyD" => KeyD,
        "KeyE" => KeyE,
        "KeyF" => KeyF,
        "KeyG" => KeyG,
        "KeyH" => KeyH,
        "KeyI" => KeyI,
        "KeyJ" => KeyJ,
        "KeyK" => KeyK,
        "KeyL" => KeyL,
        "KeyM" => KeyM,
        "KeyN" => KeyN,
        "KeyO" => KeyO,
        "KeyP" => KeyP,
        "KeyQ" => KeyQ,
        "KeyR" => KeyR,
        "KeyS" => KeyS,
        "KeyT" => KeyT,
        "KeyU" => KeyU,
        "KeyV" => KeyV,
        "KeyW" => KeyW,
        "KeyX" => KeyX,
        "KeyY" => KeyY,
        "KeyZ" => KeyZ,
        "Digit0" => Digit0,
        "Digit1" => Digit1,
        "Digit2" => Digit2,
        "Digit3" => Digit3,
        "Digit4" => Digit4,
        "Digit5" => Digit5,
        "Digit6" => Digit6,
        "Digit7" => Digit7,
        "Digit8" => Digit8,
        "Digit9" => Digit9,
        "F1" => F1,
        "F2" => F2,
        "F3" => F3,
        "F4" => F4,
        "F5" => F5,
        "F6" => F6,
        "F7" => F7,
        "F8" => F8,
        "F9" => F9,
        "F10" => F10,
        "F11" => F11,
        "F12" => F12,
        "Backquote" => Backquote,
        "Minus" => Minus,
        "Equal" => Equal,
        "BracketLeft" => BracketLeft,
        "BracketRight" => BracketRight,
        "Backslash" => Backslash,
        "Semicolon" => Semicolon,
        "Quote" => Quote,
        "Comma" => Comma,
        "Period" => Period,
        "Slash" => Slash,
        _ => return None,
    })
}
