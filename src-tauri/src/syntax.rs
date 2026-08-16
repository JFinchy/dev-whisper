/// "Syntax & Casing Commands" from the original product spec: dictating
/// "snake case error response handler" outputs `error_response_handler`
/// directly. Pure string transformation, deliberately not LLM-backed —
/// casing is a mechanical operation, not something that benefits from an
/// LLM's judgment the way "clean up this dictation" does, and doing it
/// locally means it works instantly and regardless of whether Ollama is
/// even running.
///
/// Detected from the transcript itself (a leading casing directive
/// phrase), so it applies across every app/mode rather than being gated
/// behind a specific Mode — a developer might want `snake_case` output
/// while dictating into any app, not just ones assigned to a particular
/// mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaseStyle {
    Snake,
    ScreamingSnake,
    Camel,
    Pascal,
    Kebab,
    Title,
}

/// Longest phrases first isn't required for correctness here (none of
/// these are prefixes of each other), but the matching logic below picks
/// the longest match regardless, so future additions stay safe even if
/// that stops being true.
const DIRECTIVES: &[(&str, CaseStyle)] = &[
    ("screaming snake case", CaseStyle::ScreamingSnake),
    ("constant case", CaseStyle::ScreamingSnake),
    ("snake case", CaseStyle::Snake),
    ("camel case", CaseStyle::Camel),
    ("pascal case", CaseStyle::Pascal),
    ("kebab case", CaseStyle::Kebab),
    ("title case", CaseStyle::Title),
];

/// Returns `Some(transformed)` if `text` starts with a recognized casing
/// directive and has content after it to transform; `None` otherwise
/// (meaning: not a casing command, fall through to normal mode handling).
pub fn try_apply_casing_command(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();

    let (phrase, style) = DIRECTIVES
        .iter()
        .filter(|(phrase, _)| lower.starts_with(phrase))
        .max_by_key(|(phrase, _)| phrase.len())?;

    // `to_lowercase()` on ASCII directive words doesn't change byte length,
    // so slicing the original-casing `trimmed` at `phrase.len()` stays
    // aligned with what we matched in `lower`.
    let rest = trimmed[phrase.len()..].trim_start_matches([':', ',']).trim();
    let rest = rest.trim_end_matches('.');

    let words = extract_words(rest);
    if words.is_empty() {
        return None;
    }

    Some(apply_case_style(*style, &words))
}

fn extract_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn apply_case_style(style: CaseStyle, words: &[String]) -> String {
    match style {
        CaseStyle::Snake => words.join("_"),
        CaseStyle::ScreamingSnake => words.join("_").to_uppercase(),
        CaseStyle::Kebab => words.join("-"),
        CaseStyle::Title => words.iter().map(|w| capitalize(w)).collect::<Vec<_>>().join(" "),
        CaseStyle::Pascal => words.iter().map(|w| capitalize(w)).collect(),
        CaseStyle::Camel => words
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { w.clone() } else { capitalize(w) })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_from_spec_example() {
        assert_eq!(
            try_apply_casing_command("Snake case error response handler"),
            Some("error_response_handler".to_string())
        );
    }

    #[test]
    fn camel_case() {
        assert_eq!(
            try_apply_casing_command("camel case error response handler"),
            Some("errorResponseHandler".to_string())
        );
    }

    #[test]
    fn pascal_case() {
        assert_eq!(
            try_apply_casing_command("pascal case error response handler"),
            Some("ErrorResponseHandler".to_string())
        );
    }

    #[test]
    fn kebab_case() {
        assert_eq!(
            try_apply_casing_command("kebab case error response handler"),
            Some("error-response-handler".to_string())
        );
    }

    #[test]
    fn screaming_snake_case() {
        assert_eq!(
            try_apply_casing_command("screaming snake case max retry count"),
            Some("MAX_RETRY_COUNT".to_string())
        );
    }

    #[test]
    fn constant_case_alias_for_screaming_snake() {
        assert_eq!(
            try_apply_casing_command("constant case max retry count"),
            Some("MAX_RETRY_COUNT".to_string())
        );
    }

    #[test]
    fn title_case() {
        assert_eq!(
            try_apply_casing_command("title case error response handler"),
            Some("Error Response Handler".to_string())
        );
    }

    #[test]
    fn is_case_insensitive_and_tolerates_trailing_period() {
        assert_eq!(
            try_apply_casing_command("SNAKE CASE user profile."),
            Some("user_profile".to_string())
        );
    }

    #[test]
    fn tolerates_a_colon_after_the_directive() {
        assert_eq!(
            try_apply_casing_command("snake case: user profile"),
            Some("user_profile".to_string())
        );
    }

    #[test]
    fn non_directive_text_returns_none() {
        assert_eq!(try_apply_casing_command("git commit update readme"), None);
        assert_eq!(try_apply_casing_command("just a normal sentence"), None);
    }

    #[test]
    fn directive_with_nothing_after_it_returns_none() {
        assert_eq!(try_apply_casing_command("snake case"), None);
        assert_eq!(try_apply_casing_command("snake case."), None);
    }

    #[test]
    fn strips_punctuation_within_the_phrase() {
        assert_eq!(
            try_apply_casing_command("snake case user's profile, please"),
            Some("user_s_profile_please".to_string())
        );
    }

    #[test]
    fn empty_string_returns_none() {
        assert_eq!(try_apply_casing_command(""), None);
    }
}
