//! "Is Dev Whisper actually able to work right now?" One list of checks,
//! shown as a setup checklist in Settings and as a "Finish setup" prompt
//! on the widget, so a fresh install (no model yet), a hotkey another app
//! owns, or a revoked Accessibility grant is visible before a dictation
//! fails, not after.
//!
//! `assess` is pure; `get_readiness` gathers the facts.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::audio::DeviceStatus;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    /// Works, but something the user set up isn't in effect.
    Warn,
    /// Dictation can't work until this is fixed.
    Error,
}

/// What the fix button does. The frontend maps each to a control.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    DownloadModel,
    GrantAccessibility,
    ChangeShortcut,
    ChooseMicrophone,
    StartOllama,
    PullLlmModel,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub id: &'static str,
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub fix: Option<Fix>,
}

/// Everything `assess` needs to know.
pub struct Facts {
    pub model_downloaded: bool,
    pub model_label: String,
    pub accessibility_trusted: bool,
    pub copy_only: bool,
    pub hotkey_error: Option<String>,
    pub device: Option<DeviceStatus>,
    /// Models some mode refines with; empty means the LLM isn't used.
    pub llm_models_needed: Vec<String>,
    /// Models Ollama has pulled, or `None` if Ollama isn't reachable.
    pub llm_models_pulled: Option<Vec<String>>,
}

pub fn assess(facts: &Facts) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(if facts.model_downloaded {
        ok("model", "Speech model ready", &facts.model_label)
    } else {
        Check {
            id: "model",
            level: Level::Error,
            title: "Download a speech model".into(),
            detail: format!(
                "{} isn't downloaded yet, so nothing can be transcribed.",
                facts.model_label
            ),
            fix: Some(Fix::DownloadModel),
        }
    });

    checks.push(if facts.copy_only {
        ok(
            "paste",
            "Copy-only mode",
            "Transcripts go to the clipboard; no permission needed.",
        )
    } else if facts.accessibility_trusted {
        ok(
            "paste",
            "Pasting allowed",
            "Accessibility permission is granted.",
        )
    } else {
        Check {
            id: "paste",
            level: Level::Error,
            title: "Allow pasting".into(),
            detail: "Dev Whisper needs Accessibility permission to paste where your cursor is. \
                     Turn it on in System Settings > Privacy & Security > Accessibility."
                .into(),
            fix: Some(Fix::GrantAccessibility),
        }
    });

    checks.push(match &facts.hotkey_error {
        None => ok(
            "hotkey",
            "Hotkey ready",
            "Press it anywhere to start dictating.",
        ),
        Some(err) => Check {
            id: "hotkey",
            level: Level::Error,
            title: "Pick a different hotkey".into(),
            detail: format!(
                "Your hotkey couldn't be registered ({err}). Another app may be using it."
            ),
            fix: Some(Fix::ChangeShortcut),
        },
    });

    checks.push(match &facts.device {
        Some(DeviceStatus::Preferred { using } | DeviceStatus::SystemDefault { using }) => {
            ok("microphone", "Microphone ready", using)
        }
        Some(DeviceStatus::Fallback { wanted, using }) => Check {
            id: "microphone",
            level: Level::Warn,
            title: format!("{wanted} isn't connected"),
            detail: format!("Recording with {using} until it's back."),
            fix: Some(Fix::ChooseMicrophone),
        },
        Some(DeviceStatus::Unavailable { .. }) | None => Check {
            id: "microphone",
            level: Level::Error,
            title: "No microphone".into(),
            detail: "Connect a microphone, or pick one in Settings.".into(),
            fix: Some(Fix::ChooseMicrophone),
        },
    });

    if !facts.llm_models_needed.is_empty() {
        checks.push(match &facts.llm_models_pulled {
            None => Check {
                id: "llm",
                level: Level::Warn,
                title: "Ollama isn't running".into(),
                detail: "Dictation still works, but without LLM cleanup until Ollama is running."
                    .into(),
                fix: Some(Fix::StartOllama),
            },
            Some(pulled) => {
                let missing: Vec<&str> = facts
                    .llm_models_needed
                    .iter()
                    .filter(|m| !pulled.iter().any(|p| p == *m))
                    .map(String::as_str)
                    .collect();
                if missing.is_empty() {
                    ok(
                        "llm",
                        "LLM cleanup ready",
                        &facts.llm_models_needed.join(", "),
                    )
                } else {
                    Check {
                        id: "llm",
                        level: Level::Warn,
                        title: format!("Download {}", missing.join(", ")),
                        detail: "A mode refines with this model, but Ollama doesn't have it yet; \
                                 those dictations paste without cleanup."
                            .into(),
                        fix: Some(Fix::PullLlmModel),
                    }
                }
            }
        });
    }

    checks
}

fn ok(id: &'static str, title: &str, detail: &str) -> Check {
    Check {
        id,
        level: Level::Ok,
        title: title.into(),
        detail: detail.into(),
        fix: None,
    }
}

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

#[cfg(not(target_os = "macos"))]
fn accessibility_trusted() -> bool {
    true
}

// Off the main thread: it probes Ollama over HTTP.
#[tauri::command(async)]
pub fn get_readiness(app: AppHandle) -> Vec<Check> {
    let cfg = crate::config::load(&app);
    let recording = app.state::<crate::recording::RecordingState>();
    let model_id = recording.whisper.active_model_id();
    let llm_models_needed = crate::llm::models_in_use(&cfg);
    let facts = Facts {
        model_downloaded: crate::models::resolve_model_path(&app, &model_id).is_some(),
        model_label: crate::models::label_for(&model_id),
        accessibility_trusted: accessibility_trusted(),
        copy_only: cfg.copy_only,
        hotkey_error: app
            .state::<crate::shortcut::PushToTalkState>()
            .error
            .lock()
            .unwrap()
            .clone(),
        device: recording.audio.device_status(),
        llm_models_pulled: if llm_models_needed.is_empty() {
            None
        } else {
            crate::llm::ollama_models()
        },
        llm_models_needed,
    };
    assess(&facts)
}

/// Opens System Settings at the Accessibility list, where the user flips
/// Dev Whisper on. macOS gives apps no way to grant it themselves.
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> Facts {
        Facts {
            model_downloaded: true,
            model_label: "Base (English)".into(),
            accessibility_trusted: true,
            copy_only: false,
            hotkey_error: None,
            device: Some(DeviceStatus::Preferred {
                using: "AIRHUG 28".into(),
            }),
            llm_models_needed: vec![],
            llm_models_pulled: None,
        }
    }

    fn level(checks: &[Check], id: &str) -> Option<Level> {
        checks.iter().find(|c| c.id == id).map(|c| c.level)
    }

    #[test]
    fn healthy_setup_is_all_ok_and_skips_the_llm_check() {
        let checks = assess(&healthy());
        assert!(checks
            .iter()
            .all(|c| c.level == Level::Ok && c.fix.is_none()));
        assert_eq!(level(&checks, "llm"), None);
    }

    #[test]
    fn fresh_install_needs_a_model() {
        let checks = assess(&Facts {
            model_downloaded: false,
            ..healthy()
        });
        let model = checks.iter().find(|c| c.id == "model").unwrap();
        assert_eq!(model.level, Level::Error);
        assert_eq!(model.fix, Some(Fix::DownloadModel));
    }

    #[test]
    fn missing_accessibility_only_matters_when_pasting() {
        let untrusted = Facts {
            accessibility_trusted: false,
            ..healthy()
        };
        assert_eq!(level(&assess(&untrusted), "paste"), Some(Level::Error));
        let copy_only = Facts {
            copy_only: true,
            ..untrusted
        };
        assert_eq!(level(&assess(&copy_only), "paste"), Some(Level::Ok));
    }

    #[test]
    fn hotkey_conflict_and_mic_problems_are_reported() {
        let checks = assess(&Facts {
            hotkey_error: Some("already registered".into()),
            device: Some(DeviceStatus::Fallback {
                wanted: "AIRHUG 28".into(),
                using: "MacBook Pro Microphone".into(),
            }),
            ..healthy()
        });
        assert_eq!(level(&checks, "hotkey"), Some(Level::Error));
        assert_eq!(level(&checks, "microphone"), Some(Level::Warn));
        assert_eq!(
            level(
                &assess(&Facts {
                    device: None,
                    ..healthy()
                }),
                "microphone"
            ),
            Some(Level::Error)
        );
    }

    #[test]
    fn llm_check_distinguishes_ollama_down_from_model_missing() {
        let needs = |pulled: Option<Vec<String>>| {
            assess(&Facts {
                llm_models_needed: vec!["qwen3.5:4b".into()],
                llm_models_pulled: pulled,
                ..healthy()
            })
            .into_iter()
            .find(|c| c.id == "llm")
            .unwrap()
        };
        assert_eq!(needs(None).fix, Some(Fix::StartOllama));
        assert_eq!(needs(Some(vec![])).fix, Some(Fix::PullLlmModel));
        assert_eq!(needs(Some(vec!["qwen3.5:4b".into()])).level, Level::Ok);
    }
}
