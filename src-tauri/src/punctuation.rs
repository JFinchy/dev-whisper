/// Named punctuation commands, from the Smart Formatting parity backlog
/// item (see BACKLOG.md, researched against Wispr Flow's docs). Say
/// "period", "open paren", "new line", etc. and get the literal character
/// instead of the spoken word. Pure string transformation, same rationale
/// as the casing commands in `syntax.rs`: mechanical, not something that
/// benefits from an LLM's judgment, so it works instantly and regardless
/// of whether Ollama is running.
///
/// Deliberately cross-cutting like casing commands — applied to every
/// transcript before mode formatting or LLM refinement, not gated behind
/// a specific mode.
///
/// Deliberately out of scope here (see BACKLOG.md for the full list):
/// context-aware capitalization/spacing, trailing-period-by-app removal,
/// and ambiguous open/close pairs like bare "angle bracket". Also skips
/// bare "at" as an alias for "at sign" — unlike Wispr, which can afford
/// the false-positive rate given cloud-side context, a bare four-letter
/// common word is too risky to treat as a command in a local, contextless
/// pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spacing {
    /// Glues to the end of whatever precedes it; a space is expected
    /// after (period, comma, ...).
    Trailing,
    /// No space on either side — concatenates directly into the
    /// surrounding words (underscore, slash, ...).
    Glued,
    /// A space before, nothing after — opens a bracket-like construct.
    Opening,
    /// Nothing before, a space after — closes a bracket-like construct.
    Closing,
    /// Replaces with `n` newlines, consuming any surrounding spaces.
    LineBreak(usize),
}

/// `(phrase, replacement, spacing)`. Phrases are matched case-insensitively
/// against whitespace-separated tokens with their own punctuation stripped
/// (so a Whisper-inserted trailing period on the last token of an
/// utterance doesn't block a match). Checked longest-phrase-first at each
/// position, same reasoning as `syntax.rs`'s `DIRECTIVES` table.
const COMMANDS: &[(&str, &str, Spacing)] = &[
    ("period", ".", Spacing::Trailing),
    ("full stop", ".", Spacing::Trailing),
    ("comma", ",", Spacing::Trailing),
    ("question mark", "?", Spacing::Trailing),
    ("exclamation point", "!", Spacing::Trailing),
    ("exclamation mark", "!", Spacing::Trailing),
    ("colon", ":", Spacing::Trailing),
    ("semicolon", ";", Spacing::Trailing),
    ("percent sign", "%", Spacing::Trailing),
    ("percentage symbol", "%", Spacing::Trailing),
    ("ellipsis", "…", Spacing::Trailing),
    ("dash", "-", Spacing::Glued),
    ("em dash", "—", Spacing::Glued),
    ("em-dash", "—", Spacing::Glued),
    ("emdash", "—", Spacing::Glued),
    ("apostrophe", "'", Spacing::Glued),
    ("single quote", "'", Spacing::Glued),
    ("asterisk", "*", Spacing::Glued),
    ("star", "*", Spacing::Glued),
    ("ampersand", "&", Spacing::Glued),
    ("slash", "/", Spacing::Glued),
    ("forward slash", "/", Spacing::Glued),
    ("backslash", "\\", Spacing::Glued),
    ("underscore", "_", Spacing::Glued),
    ("tilde", "~", Spacing::Glued),
    ("at sign", "@", Spacing::Glued),
    ("at symbol", "@", Spacing::Glued),
    ("plus", "+", Spacing::Glued),
    ("plus sign", "+", Spacing::Glued),
    ("minus", "-", Spacing::Glued),
    ("minus sign", "-", Spacing::Glued),
    ("negative", "-", Spacing::Glued),
    ("equals", "=", Spacing::Glued),
    ("equals sign", "=", Spacing::Glued),
    ("trademark", "™", Spacing::Glued),
    ("tm", "™", Spacing::Glued),
    ("registered trademark", "®", Spacing::Glued),
    ("copyright", "©", Spacing::Glued),
    ("copyright symbol", "©", Spacing::Glued),
    ("degree sign", "°", Spacing::Glued),
    ("degree symbol", "°", Spacing::Glued),
    ("degrees celsius", "°C", Spacing::Glued),
    ("degrees centigrade", "°C", Spacing::Glued),
    ("degrees fahrenheit", "°F", Spacing::Glued),
    ("degrees f", "°F", Spacing::Glued),
    ("hashtag", "#", Spacing::Opening),
    ("hash", "#", Spacing::Opening),
    ("open parenthesis", "(", Spacing::Opening),
    ("open paren", "(", Spacing::Opening),
    ("close parenthesis", ")", Spacing::Closing),
    ("close paren", ")", Spacing::Closing),
    ("new paragraph", "\n\n", Spacing::LineBreak(2)),
    ("new line", "\n", Spacing::LineBreak(1)),
    ("next line", "\n", Spacing::LineBreak(1)),
    ("line break", "\n", Spacing::LineBreak(1)),
];

const QUOTE_PHRASE: &str = "quotation mark";

/// Longest phrase in `COMMANDS`/`QUOTE_PHRASE`, in words — bounds how far
/// ahead `match_at` looks.
const MAX_PHRASE_WORDS: usize = 2;

fn normalize(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Tries to match a command phrase starting at `tokens[i]`, longest first.
/// Returns `(words_consumed, replacement, spacing)`.
fn match_at(tokens: &[&str], i: usize, quote_open: &mut bool) -> Option<(usize, &'static str, Spacing)> {
    for len in (1..=MAX_PHRASE_WORDS.min(tokens.len() - i)).rev() {
        let phrase = tokens[i..i + len]
            .iter()
            .map(|t| normalize(t))
            .collect::<Vec<_>>()
            .join(" ");

        if phrase == QUOTE_PHRASE {
            let spacing = if *quote_open { Spacing::Opening } else { Spacing::Closing };
            *quote_open = !*quote_open;
            return Some((len, "\"", spacing));
        }

        if let Some((_, replacement, spacing)) = COMMANDS.iter().find(|(p, _, _)| *p == phrase) {
            return Some((len, replacement, *spacing));
        }
    }
    None
}

fn push_word(out: &mut String, word: &str, suppress_space: &mut bool) {
    if !out.is_empty() && !*suppress_space {
        out.push(' ');
    }
    out.push_str(word);
    *suppress_space = false;
}

fn push_replacement(out: &mut String, replacement: &str, spacing: Spacing, suppress_space: &mut bool) {
    match spacing {
        Spacing::Trailing => {
            out.push_str(replacement);
            *suppress_space = false;
        }
        Spacing::Glued => {
            out.push_str(replacement);
            *suppress_space = true;
        }
        Spacing::Opening => {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(replacement);
            *suppress_space = true;
        }
        Spacing::Closing => {
            out.push_str(replacement);
            *suppress_space = false;
        }
        Spacing::LineBreak(n) => {
            while out.ends_with(' ') {
                out.pop();
            }
            for _ in 0..n {
                out.push('\n');
            }
            *suppress_space = true;
        }
    }
}

/// Expands named punctuation commands in `text` into their literal
/// characters. Words that don't match any command pass through unchanged.
/// Always returns a value (unlike `syntax::try_apply_casing_command`) —
/// there's no "not a command" fallback needed, since a transcript with no
/// punctuation commands in it round-trips as itself (modulo whitespace
/// normalization).
pub fn expand_punctuation(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut out = String::new();
    let mut suppress_space = false;
    let mut quote_open = true;
    let mut i = 0;

    while i < tokens.len() {
        if let Some((consumed, replacement, spacing)) = match_at(&tokens, i, &mut quote_open) {
            push_replacement(&mut out, replacement, spacing, &mut suppress_space);
            i += consumed;
        } else {
            push_word(&mut out, tokens[i], &mut suppress_space);
            i += 1;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_punctuation_glues_to_preceding_word() {
        assert_eq!(
            expand_punctuation("I can't wait to see you exclamation point Let's meet at seven period"),
            "I can't wait to see you! Let's meet at seven."
        );
    }

    #[test]
    fn comma_and_colon() {
        assert_eq!(
            expand_punctuation("dear team comma here's the plan colon ship it"),
            "dear team, here's the plan: ship it"
        );
    }

    #[test]
    fn glued_symbols_for_dev_use_cases() {
        // `Glued` never inserts a space on either side, even when a symbol
        // opens a new "word" (like `~` starting a path) rather than joining
        // two existing ones — that distinction needs context this pass
        // doesn't have. Known rough edge, not a bug: "cd tilde slash
        // projects" -> "cd~/projects", not "cd ~/projects".
        assert_eq!(expand_punctuation("cd tilde slash dot config"), "cd~/dot config");
        assert_eq!(expand_punctuation("import from src slash lib"), "import from src/lib");
        assert_eq!(expand_punctuation("x equals sign y"), "x=y");
        assert_eq!(expand_punctuation("user at sign example"), "user@example");
        assert_eq!(expand_punctuation("max underscore retry underscore count"), "max_retry_count");
    }

    #[test]
    fn open_and_close_parenthesis() {
        assert_eq!(
            expand_punctuation("call foo open paren a comma b close paren"),
            "call foo (a, b)"
        );
    }

    #[test]
    fn hashtag_is_opening_only() {
        assert_eq!(expand_punctuation("check out hashtag coding today"), "check out #coding today");
    }

    #[test]
    fn quotation_marks_toggle_open_and_close() {
        assert_eq!(
            expand_punctuation("she said quotation mark hello quotation mark"),
            "she said \"hello\""
        );
    }

    #[test]
    fn new_line_and_new_paragraph() {
        assert_eq!(expand_punctuation("reading club new line should be tomorrow"), "reading club\nshould be tomorrow");
        assert_eq!(
            expand_punctuation("first section new paragraph second section"),
            "first section\n\nsecond section"
        );
    }

    #[test]
    fn no_commands_round_trips_with_normalized_whitespace() {
        assert_eq!(expand_punctuation("just a normal sentence"), "just a normal sentence");
    }

    #[test]
    fn empty_string_returns_empty() {
        assert_eq!(expand_punctuation(""), "");
    }

    #[test]
    fn tolerates_whisper_inserted_trailing_punctuation_on_command_words() {
        // Whisper's own sentence-end punctuation can land glued to the
        // last token (e.g. "period." instead of "period"); normalization
        // strips it before matching so the command still fires.
        assert_eq!(expand_punctuation("wrap it up period."), "wrap it up.");
    }
}
