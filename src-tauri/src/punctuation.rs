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
    // Not `split_whitespace` — `expand_lists` may run first and glues its
    // `\n` directly onto the preceding word with no space (e.g.
    // "report\n2."), and `split_whitespace` would swallow that embedded
    // newline as a delimiter and drop it on rejoin. `tokenize` below
    // treats '\n' as its own hard-delimited token instead, so a command
    // word sitting right before a list break (e.g. "...period\n2.") still
    // gets recognized on its own rather than being glued into one
    // unmatchable blob.
    let tokens: Vec<&str> = tokenize(text);
    let mut out = String::new();
    let mut suppress_space = false;
    let mut quote_open = true;
    let mut i = 0;

    while i < tokens.len() {
        if tokens[i] == "\n" {
            push_replacement(&mut out, "\n", Spacing::LineBreak(1), &mut suppress_space);
            i += 1;
        } else if let Some((consumed, replacement, spacing)) = match_at(&tokens, i, &mut quote_open) {
            push_replacement(&mut out, replacement, spacing, &mut suppress_space);
            i += consumed;
        } else {
            push_word(&mut out, tokens[i], &mut suppress_space);
            i += 1;
        }
    }

    out
}

/// Splits on spaces like `split_whitespace`, except a literal `\n` is
/// always its own token rather than being absorbed as a delimiter — see
/// the comment in `expand_punctuation` for why that distinction matters.
fn tokenize(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = text;

    loop {
        rest = rest.trim_start_matches(' ');
        if rest.is_empty() {
            break;
        }
        if let Some(stripped) = rest.strip_prefix('\n') {
            tokens.push("\n");
            rest = stripped;
            continue;
        }
        let end = rest.find([' ', '\n']).unwrap_or(rest.len());
        tokens.push(&rest[..end]);
        rest = &rest[end..];
    }

    tokens
}

const CARDINAL_WORDS: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
    "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
    "twenty",
];

const ORDINAL_WORDS: &[&str] = &[
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
    "tenth", "eleventh", "twelfth", "thirteenth", "fourteenth", "fifteenth", "sixteenth",
    "seventeenth", "eighteenth", "nineteenth", "twentieth",
];

/// Scans `tokens` for a strictly consecutive run of `family` words starting
/// at index 0 ("one"/"first"), in order but not necessarily adjacent.
/// Returns `(token_index, 1-based_value)` for each marker found.
fn find_sequence(tokens: &[&str], family: &[&str]) -> Vec<(usize, usize)> {
    let mut positions = Vec::new();
    let mut expected = 1usize;
    for (i, tok) in tokens.iter().enumerate() {
        if expected - 1 >= family.len() {
            break;
        }
        if normalize(tok) == family[expected - 1] {
            positions.push((i, expected));
            expected += 1;
        }
    }
    positions
}

/// Turns a spoken numbered list ("one... two...", "first... second...")
/// into a real numbered list with line breaks. Requires at least two
/// markers found in strictly consecutive order starting at 1/first — a
/// lone "one" or "first" is far too common a word on its own to treat as
/// a list start. If both families produce a run, the longer one wins.
///
/// Known false positive, accepted: an ordinary sentence using "one" and
/// "two" as plain numbers ("one item for two dollars") also matches —
/// there's no surrounding-context signal available to rule that out in a
/// deterministic, non-LLM pass. Same tradeoff as the symbol-spacing rough
/// edge in `expand_punctuation`.
///
/// Doesn't capitalize item text or punctuate the lead-in with a colon the
/// way Wispr's (context-aware, LLM-assisted) version does — out of scope
/// for a pure deterministic pass, see BACKLOG.md.
pub fn expand_lists(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return String::new();
    }

    let cardinal = find_sequence(&tokens, CARDINAL_WORDS);
    let ordinal = find_sequence(&tokens, ORDINAL_WORDS);
    let positions = if ordinal.len() > cardinal.len() { ordinal } else { cardinal };

    if positions.len() < 2 {
        return tokens.join(" ");
    }

    let mut out = String::new();
    let lead_in_end = positions[0].0;
    if lead_in_end > 0 {
        out.push_str(&tokens[..lead_in_end].join(" "));
    }

    for (idx, &(pos, value)) in positions.iter().enumerate() {
        let item_end = positions.get(idx + 1).map(|(p, _)| *p).unwrap_or(tokens.len());
        let item_text = tokens[pos + 1..item_end].join(" ");
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{value}. {item_text}"));
    }

    out
}

/// Detects a trailing "press enter" and strips it, so the caller can paste
/// the remaining text and then simulate an Enter keystroke (see
/// `paste::press_enter`). Returns `(remaining_text, should_press_enter)`.
///
/// Only fires at the very end of the dictation — "press enter" said
/// mid-sentence stays in the text untouched, matching Wispr's own scoping
/// (a user narrating a sentence that happens to contain those words
/// shouldn't have it eaten). Tolerates Whisper's own trailing sentence
/// punctuation after "enter" (e.g. "...press enter." still matches). If
/// "press enter" is the *entire* utterance, returns an empty string but
/// still signals `true` — nothing to paste, but Enter still fires.
///
/// Doesn't replicate Wispr's context-aware auto-punctuation (their own
/// docs example shows "hello world press enter." becoming "Hello world."
/// with an inserted period) — out of scope for a pure deterministic pass,
/// same as everything else in this module.
pub fn extract_press_enter(text: &str) -> (String, bool) {
    const PHRASE: &str = "press enter";

    let trimmed = text.trim();
    let without_trailing_punct = trimmed.trim_end_matches(['.', '!', '?']).trim_end();
    let lower = without_trailing_punct.to_lowercase();

    if lower == PHRASE {
        return (String::new(), true);
    }

    // `to_lowercase()` on this ASCII phrase doesn't change byte length, so
    // `prefix.len()` stays aligned with the original-casing string below —
    // same technique as `syntax::try_apply_casing_command`.
    if let Some(prefix) = lower.strip_suffix(PHRASE) {
        if prefix.is_empty() || prefix.ends_with(' ') {
            let remaining = without_trailing_punct[..prefix.len()].trim_end();
            return (remaining.to_string(), true);
        }
    }

    (text.to_string(), false)
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

    #[test]
    fn cardinal_list_from_wispr_example() {
        assert_eq!(
            expand_lists("my top goals this week are one finish the report two send the presentation"),
            "my top goals this week are\n1. finish the report\n2. send the presentation"
        );
    }

    #[test]
    fn ordinal_list() {
        assert_eq!(
            expand_lists("first check the logs second restart the service third verify"),
            "1. check the logs\n2. restart the service\n3. verify"
        );
    }

    #[test]
    fn longer_family_wins_when_both_present() {
        // Only "one" appears from the cardinal family (run length 1, not
        // enough to count), while first/second/third form a valid run —
        // ordinal wins outright, not just on a length comparison.
        assert_eq!(
            expand_lists("first grab one coffee second then get to work third done"),
            "1. grab one coffee\n2. then get to work\n3. done"
        );
    }

    #[test]
    fn a_single_marker_is_not_a_list() {
        assert_eq!(expand_lists("i only need one thing"), "i only need one thing");
    }

    #[test]
    fn no_markers_round_trips() {
        assert_eq!(expand_lists("just a normal sentence"), "just a normal sentence");
    }

    #[test]
    fn documented_false_positive_two_plain_numbers() {
        // Accepted limitation: this isn't a list, but the deterministic
        // pass has no way to tell without more context. See the doc
        // comment on `expand_lists`.
        assert_eq!(
            expand_lists("i bought one item for two dollars"),
            "i bought\n1. item for\n2. dollars"
        );
    }

    #[test]
    fn composes_with_punctuation_expansion_without_losing_line_breaks() {
        // expand_lists glues its `\n` directly onto the preceding word
        // with no space, so this exercises `expand_punctuation`'s
        // `tokenize` helper, which treats that embedded newline as its
        // own token instead of silently dropping it.
        let listed = expand_lists("todo items are one fix bug period two write tests period");
        assert_eq!(
            expand_punctuation(&listed),
            "todo items are\n1. fix bug.\n2. write tests."
        );
    }

    #[test]
    fn press_enter_at_end_is_stripped_and_signaled() {
        assert_eq!(
            extract_press_enter("hello world press enter"),
            ("hello world".to_string(), true)
        );
    }

    #[test]
    fn press_enter_tolerates_whisper_trailing_punctuation() {
        assert_eq!(
            extract_press_enter("hello world press enter."),
            ("hello world".to_string(), true)
        );
        assert_eq!(
            extract_press_enter("hello world. press enter."),
            ("hello world.".to_string(), true)
        );
    }

    #[test]
    fn press_enter_as_entire_utterance_pastes_nothing() {
        assert_eq!(extract_press_enter("press enter"), (String::new(), true));
        assert_eq!(extract_press_enter("Press enter."), (String::new(), true));
    }

    #[test]
    fn press_enter_mid_sentence_is_left_alone() {
        assert_eq!(
            extract_press_enter("please press enter the building code"),
            ("please press enter the building code".to_string(), false)
        );
    }

    #[test]
    fn no_press_enter_returns_unchanged() {
        assert_eq!(
            extract_press_enter("just a normal sentence"),
            ("just a normal sentence".to_string(), false)
        );
    }
}
