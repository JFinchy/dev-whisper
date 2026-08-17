/// "Append clipboard" — say a trailing trigger phrase and whatever's
/// currently on the system clipboard gets appended to the dictated text,
/// so a copied error message, URL, or code snippet doesn't have to be
/// read aloud. Detected the same way as `punctuation::extract_press_enter`:
/// a trailing control phrase, stripped before the rest of the pipeline
/// (casing/boilerplate/mode/LLM) sees the transcript, with the actual
/// clipboard read happening later, once `formatted` is final, so the
/// clipboard text is appended raw rather than run through mode formatting
/// or LLM refinement.
///
/// A few synonyms rather than one fixed phrase — same reasoning as
/// `punctuation.rs`'s multiple aliases per symbol (e.g. "period"/"full
/// stop"): nobody remembers a single arbitrary cue word reliably.
const TRIGGERS: &[&str] = &["append clipboard", "paste clipboard", "insert clipboard"];

/// Returns `(text_with_trigger_removed, should_append_clipboard)`. Mirrors
/// `punctuation::extract_press_enter`'s shape exactly, generalized to
/// several trigger phrases instead of one.
pub fn try_extract_trigger(text: &str) -> (String, bool) {
    let trimmed = text.trim();
    let without_trailing_punct = trimmed.trim_end_matches(['.', '!', '?']).trim_end();
    let lower = without_trailing_punct.to_lowercase();

    for phrase in TRIGGERS {
        if lower == *phrase {
            return (String::new(), true);
        }

        // `to_lowercase()` on these ASCII phrases doesn't change byte
        // length, so slicing `without_trailing_punct` at `prefix.len()`
        // stays aligned with what matched in `lower` — same technique as
        // `syntax::try_apply_casing_command`.
        if let Some(prefix) = lower.strip_suffix(phrase) {
            if prefix.is_empty() || prefix.ends_with(' ') {
                let remaining = without_trailing_punct[..prefix.len()].trim_end();
                return (remaining.to_string(), true);
            }
        }
    }

    (text.to_string(), false)
}

/// `None` covers both an unreadable clipboard (locked, another app holding
/// it) and a non-text clipboard (an image, a file reference) — either way
/// there's nothing sensible to append, and the caller falls back to
/// leaving `formatted` unchanged rather than pasting a Rust error string.
pub fn read_clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok().filter(|s| !s.is_empty())
}

/// Appends clipboard text to `formatted`, joined with a space unless
/// `formatted` is empty (the trigger was the entire utterance), in which
/// case the clipboard content stands alone with no leading space.
pub fn append(formatted: &str, clipboard_text: &str) -> String {
    if formatted.is_empty() {
        clipboard_text.to_string()
    } else {
        format!("{formatted} {clipboard_text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_a_trailing_trigger_phrase() {
        assert_eq!(
            try_extract_trigger("here's the error append clipboard"),
            ("here's the error".to_string(), true)
        );
    }

    #[test]
    fn recognizes_all_three_synonyms() {
        for phrase in TRIGGERS {
            assert_eq!(
                try_extract_trigger(&format!("some text {phrase}")),
                ("some text".to_string(), true)
            );
        }
    }

    #[test]
    fn tolerates_whispers_trailing_punctuation() {
        assert_eq!(
            try_extract_trigger("here's the error append clipboard."),
            ("here's the error".to_string(), true)
        );
    }

    #[test]
    fn trigger_as_the_entire_utterance() {
        assert_eq!(try_extract_trigger("paste clipboard"), (String::new(), true));
        assert_eq!(try_extract_trigger("paste clipboard."), (String::new(), true));
    }

    #[test]
    fn does_not_match_mid_sentence() {
        assert_eq!(
            try_extract_trigger("append clipboard to the document please"),
            ("append clipboard to the document please".to_string(), false)
        );
    }

    #[test]
    fn does_not_false_positive_on_unrelated_text() {
        assert_eq!(
            try_extract_trigger("just a normal sentence"),
            ("just a normal sentence".to_string(), false)
        );
    }

    #[test]
    fn requires_a_word_boundary_before_the_trigger() {
        // "reclipboard" ends with "clipboard" but isn't a real trigger —
        // "insert clipboard" only matches with a preceding space (or as
        // the entire utterance).
        assert_eq!(
            try_extract_trigger("please reinsert clipboard"),
            ("please reinsert clipboard".to_string(), false)
        );
    }

    #[test]
    fn empty_string_round_trips() {
        assert_eq!(try_extract_trigger(""), (String::new(), false));
    }

    #[test]
    fn append_joins_with_a_space() {
        assert_eq!(append("here's the error", "TypeError: x is undefined"), "here's the error TypeError: x is undefined");
    }

    #[test]
    fn append_with_empty_formatted_has_no_leading_space() {
        assert_eq!(append("", "TypeError: x is undefined"), "TypeError: x is undefined");
    }
}
