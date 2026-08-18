/// Snippet library, from the 2026-08-17 competitive research (Wispr
/// Flow's "Flow for Developers": a spoken cue expands to a saved block of
/// text, e.g. "PR Checklist" or "Environment Setup"). Distinct from the
/// vocabulary editor (`stt::default_vocabulary`), which biases Whisper's
/// *recognition* of a term — this controls what gets *inserted* for one.
///
/// Pure, fast, pre-LLM detection step — same shape as the already-shipped
/// Syntax & Casing Commands (`syntax.rs`) and Boilerplate Generation
/// (`boilerplate.rs`): deterministic, instant, and doesn't depend on
/// Ollama being up.
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Snippet {
    pub trigger: String,
    pub body: String,
}

/// Ships a handful of real, useful dev snippets out of the box rather than
/// an empty list — same reasoning as `stt::default_vocabulary()`: the
/// feature should already be useful on a fresh install, not something a
/// user has to populate from scratch before it does anything. Editable/
/// deletable like any other snippet once loaded into config.
pub fn default_snippets() -> Vec<Snippet> {
    vec![
        Snippet {
            trigger: "pr checklist".to_string(),
            body: "- [ ] Tests pass locally\n\
- [ ] Added/updated tests for the change\n\
- [ ] Self-reviewed the diff\n\
- [ ] Updated docs/comments if behavior changed\n\
- [ ] No leftover debug prints or commented-out code"
                .to_string(),
        },
        Snippet {
            trigger: "standup update".to_string(),
            body: "Yesterday: \nToday: \nBlockers: ".to_string(),
        },
        Snippet {
            trigger: "bug report template".to_string(),
            body: "Steps to reproduce:\n1. \n\nExpected:\n\nActual:\n\nEnvironment: ".to_string(),
        },
        Snippet {
            trigger: "commit message template".to_string(),
            body: "Summary:\n\nWhy:\n\nTesting:".to_string(),
        },
    ]
}

/// Case-insensitive, whole-utterance match against a configured trigger —
/// not a prefix match like casing commands, since a trigger like "standup
/// update" is meant to be spoken as a complete, deliberate cue, not a
/// directive with content trailing it ("standup update for the api team"
/// should NOT match). Tolerant of Whisper's own trailing sentence
/// punctuation, same as the other pre-LLM passes.
pub fn try_expand(text: &str, snippets: &[Snippet]) -> Option<String> {
    let trimmed = text.trim().trim_end_matches(['.', '!', '?']).trim();
    if trimmed.is_empty() {
        return None;
    }
    snippets
        .iter()
        .find(|s| s.trigger.trim().eq_ignore_ascii_case(trimmed))
        .map(|s| s.body.clone())
}

#[tauri::command]
pub fn get_snippets(app: AppHandle) -> Vec<Snippet> {
    crate::config::load(&app).snippets
}

/// Full-list replace, mirroring `recording::set_vocabulary` — simpler than
/// a keyed add/update/remove trio since a snippet's trigger (its only
/// natural key) is itself user-editable, and the Settings UI already
/// manages the whole list client-side.
#[tauri::command]
pub fn set_snippets(app: AppHandle, snippets: Vec<Snippet>) {
    let mut cfg = crate::config::load(&app);
    cfg.snippets = snippets;
    let _ = crate::config::save(&app, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Snippet> {
        vec![
            Snippet { trigger: "pr checklist".to_string(), body: "- [ ] Tests pass".to_string() },
            Snippet { trigger: "standup update".to_string(), body: "Yesterday:\nToday:".to_string() },
        ]
    }

    #[test]
    fn expands_an_exact_trigger_match() {
        assert_eq!(try_expand("pr checklist", &sample()), Some("- [ ] Tests pass".to_string()));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(try_expand("PR Checklist", &sample()), Some("- [ ] Tests pass".to_string()));
    }

    #[test]
    fn tolerates_whispers_trailing_sentence_punctuation() {
        assert_eq!(try_expand("pr checklist.", &sample()), Some("- [ ] Tests pass".to_string()));
        assert_eq!(try_expand("standup update?", &sample()), Some("Yesterday:\nToday:".to_string()));
    }

    #[test]
    fn does_not_match_a_trigger_used_mid_sentence() {
        assert_eq!(try_expand("can you give me the pr checklist please", &sample()), None);
    }

    #[test]
    fn does_not_match_an_unconfigured_phrase() {
        assert_eq!(try_expand("something else entirely", &sample()), None);
    }

    #[test]
    fn empty_snippet_list_never_matches() {
        assert_eq!(try_expand("pr checklist", &[]), None);
    }

    #[test]
    fn blank_transcript_never_matches() {
        assert_eq!(try_expand("   ", &sample()), None);
    }

    #[test]
    fn default_snippets_are_non_empty_with_unique_triggers() {
        let defaults = default_snippets();
        assert!(!defaults.is_empty());
        let mut triggers: Vec<String> = defaults.iter().map(|s| s.trigger.to_lowercase()).collect();
        triggers.sort();
        triggers.dedup();
        assert_eq!(triggers.len(), defaults.len());
    }
}
