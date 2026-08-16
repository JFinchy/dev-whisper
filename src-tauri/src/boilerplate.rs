/// "Boilerplate Generation" from the product spec's stretch goals:
/// dictating "generate boilerplate for a React component called UserCard
/// with name and avatar props" sends the request straight to the local LLM
/// for code generation instead of running it through normal mode
/// formatting. Detected the same way as syntax.rs's casing commands — a
/// leading trigger phrase in the transcript — so it works in any app/mode
/// rather than being gated behind a specific one. Unlike casing commands
/// this needs an LLM (generating real code isn't a mechanical string
/// transform), so it's implemented as a request extractor here plus
/// `llm::generate_boilerplate` for the actual generation.
const DIRECTIVES: &[&str] = &[
    "generate boilerplate for",
    "generate boilerplate",
    "boilerplate for",
    "scaffold code for",
    "generate code for",
];

/// Returns the code-generation request (the remainder of the transcript
/// after the trigger phrase) if `text` starts with a recognized
/// boilerplate directive and has content after it; `None` otherwise
/// (meaning: not a boilerplate command, fall through to normal handling).
pub fn try_extract_request(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();

    let phrase = DIRECTIVES
        .iter()
        .filter(|phrase| lower.starts_with(**phrase))
        .max_by_key(|phrase| phrase.len())?;

    // Same alignment trick as syntax.rs: `to_lowercase()` on these ASCII
    // directive phrases doesn't change byte length, so slicing the
    // original-casing `trimmed` at `phrase.len()` stays aligned with what
    // matched in `lower`.
    let rest = trimmed[phrase.len()..].trim_start_matches([':', ',']).trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_generate_boilerplate_for() {
        assert_eq!(
            try_extract_request(
                "generate boilerplate for a React component called UserCard with name and avatar props"
            ),
            Some("a React component called UserCard with name and avatar props".to_string())
        );
    }

    #[test]
    fn matches_boilerplate_for() {
        assert_eq!(
            try_extract_request("boilerplate for an Express GET route on slash users"),
            Some("an Express GET route on slash users".to_string())
        );
    }

    #[test]
    fn matches_generate_code_for() {
        assert_eq!(
            try_extract_request("generate code for a binary search function in Python"),
            Some("a binary search function in Python".to_string())
        );
    }

    #[test]
    fn matches_scaffold_code_for() {
        assert_eq!(
            try_extract_request("scaffold code for a Rust struct named Point with x and y"),
            Some("a Rust struct named Point with x and y".to_string())
        );
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            try_extract_request("GENERATE BOILERPLATE FOR a Python class named Dog"),
            Some("a Python class named Dog".to_string())
        );
    }

    #[test]
    fn tolerates_a_colon_after_the_directive() {
        assert_eq!(
            try_extract_request("generate boilerplate: a Go HTTP handler"),
            Some("a Go HTTP handler".to_string())
        );
    }

    #[test]
    fn non_directive_text_returns_none() {
        assert_eq!(try_extract_request("git commit update readme"), None);
        assert_eq!(try_extract_request("just a normal sentence"), None);
    }

    #[test]
    fn directive_with_nothing_after_it_returns_none() {
        assert_eq!(try_extract_request("generate boilerplate"), None);
        assert_eq!(try_extract_request("boilerplate for"), None);
    }

    #[test]
    fn empty_string_returns_none() {
        assert_eq!(try_extract_request(""), None);
    }

    #[test]
    fn longest_matching_directive_wins() {
        // "generate boilerplate for" and "generate boilerplate" both match
        // the prefix — the longer, more specific one should be preferred so
        // "for" isn't left dangling at the start of the request.
        assert_eq!(
            try_extract_request("generate boilerplate for a queue"),
            Some("a queue".to_string())
        );
    }
}
