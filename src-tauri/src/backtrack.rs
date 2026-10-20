/// "Backtrack" from the Smart Formatting parity backlog item — self-
/// correct mid-dictation with a trigger phrase instead of stopping and
/// restarting. Deliberately narrower than Wispr Flow's version, which
/// also catches a natural restatement with no trigger word at all via
/// full-context LLM judgment; that fuzzier case is out of scope for a
/// pure deterministic pass (partially covered instead by the "fix filler
/// words, false starts" instruction already in `llm.rs`'s refinement
/// prompts, when a mode has LLM refinement on).
///
/// Must run *after* `punctuation::expand_punctuation` — the "actually"
/// trigger requires a literal preceding comma, which either comes from an
/// explicit spoken "comma" already expanded by that pass, or from
/// Whisper's own natural comma insertion at a speech pause.
///
/// Trigger design, and why "actually" needs the comma gate:
/// - **"scratch that"** fires unconditionally — nobody says this phrase
///   by accident, it's only ever a correction cue.
/// - **", actually"** (comma required immediately before) fires only
///   then, not on a bare "actually" — "actually" is far too common a
///   word in ordinary speech ("I actually enjoyed it") to treat as a
///   correction trigger on its own; requiring the preceding comma (a
///   real pause) cuts most of that false-positive rate. Not all of it:
///   "well, actually, I think it's fine" is a common hedge that still has
///   the comma and will still misfire. Accepted residual risk, same
///   shape as the other documented rough edges in `punctuation.rs`.
///
/// What a trigger corrects:
/// - A short correction (up to `SHORT_CORRECTION_WORDS` words) replaces the
///   same number of words right before the trigger: "meet at two,
///   actually three" becomes "meet at three", the way Wispr Flow does it.
///   Spoken corrections are almost always this shape: the wrong value
///   restated in the same slot.
/// - A longer correction is a restatement, and replaces the sentence it
///   interrupts. Earlier sentences are kept, so correcting one detail
///   never throws away the rest of the dictation.
///
/// Repeated corrections resolve left to right ("two, actually three,
/// actually four" ends on four).
pub fn try_backtrack(text: &str) -> String {
    let lower = text.to_lowercase();

    let comma_actually = find_last(&lower, ", actually");
    let scratch_that = find_last(&lower, "scratch that");

    let trigger = match (comma_actually, scratch_that) {
        (Some(c), Some(s)) => Some(if c.0 > s.0 { c } else { s }),
        (c, s) => c.or(s),
    };

    let Some((trigger_start, trigger_end)) = trigger else {
        return text.to_string();
    };

    // `to_lowercase()` on these ASCII trigger phrases doesn't change byte
    // length, so the offsets stay aligned with the original-casing
    // `text` — same technique as `syntax::try_apply_casing_command`.
    let remainder = text[trigger_end..]
        .trim_start_matches([',', '.', ' '])
        .trim();

    if remainder.is_empty() {
        // Nothing to replace with (e.g. "scratch that" was the last thing
        // said) — leave the text alone rather than discarding everything.
        return text.to_string();
    }

    // Earlier corrections first, so this one applies to their result.
    let before = try_backtrack(&text[..trigger_start]);
    let before = before.trim_end_matches([',', '.', ' ']);
    apply_correction(before, remainder)
}

/// Corrections at most this long replace that many trailing words; longer
/// ones replace the whole interrupted sentence.
const SHORT_CORRECTION_WORDS: usize = 3;

fn apply_correction(before: &str, correction: &str) -> String {
    // The interrupted sentence starts after the last sentence break.
    let sentence_start = before
        .char_indices()
        .rev()
        .find(|&(i, c)| {
            c == '\n' || (matches!(c, '.' | '!' | '?') && before[i + 1..].starts_with(' '))
        })
        .map_or(0, |(i, c)| i + c.len_utf8());
    let (kept, sentence) = before.split_at(sentence_start);
    let sentence_words: Vec<&str> = sentence.split_whitespace().collect();
    let correction_words = correction.split_whitespace().count();

    let corrected =
        if correction_words <= SHORT_CORRECTION_WORDS && sentence_words.len() > correction_words {
            let head = &sentence_words[..sentence_words.len() - correction_words];
            format!("{} {correction}", head.join(" "))
        } else {
            let starts_upper = sentence.trim_start().starts_with(char::is_uppercase);
            if starts_upper || (kept.is_empty() && sentence.is_empty()) {
                capitalize_first(correction)
            } else {
                correction.to_string()
            }
        };

    let kept = kept.trim_end();
    if kept.is_empty() {
        corrected
    } else {
        let separator = if kept.ends_with('\n') { "" } else { " " };
        format!("{kept}{separator}{corrected}")
    }
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Rightmost case-insensitive match of `needle` in the already-lowercased
/// `lower`. Returns `(start, end)` byte offsets.
fn find_last(lower: &str, needle: &str) -> Option<(usize, usize)> {
    lower
        .rmatch_indices(needle)
        .next()
        .map(|(start, m)| (start, start + m.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_that_swaps_the_corrected_word() {
        assert_eq!(
            try_backtrack("call the client at three scratch that four"),
            "call the client at four"
        );
    }

    #[test]
    fn comma_actually_swaps_the_corrected_word() {
        assert_eq!(
            try_backtrack("let's do coffee at two, actually three"),
            "let's do coffee at three"
        );
        assert_eq!(
            try_backtrack("Let's meet at two, actually three."),
            "Let's meet at three."
        );
    }

    #[test]
    fn multi_word_short_correction_replaces_as_many_words() {
        assert_eq!(
            try_backtrack("Call me at 5 pm, actually 6 pm."),
            "Call me at 6 pm."
        );
    }

    #[test]
    fn long_correction_replaces_only_the_interrupted_sentence() {
        assert_eq!(
            try_backtrack(
                "I fixed the build. We should use Postgres, actually let's go with SQLite for now."
            ),
            "I fixed the build. Let's go with SQLite for now."
        );
        assert_eq!(
            try_backtrack("The tests are flaky, scratch that, the tests all pass now."),
            "The tests all pass now."
        );
    }

    #[test]
    fn correction_as_long_as_the_sentence_replaces_it() {
        assert_eq!(
            try_backtrack("the red one, actually the blue one"),
            "the blue one"
        );
    }

    #[test]
    fn bare_actually_with_no_comma_is_left_alone() {
        assert_eq!(
            try_backtrack("I actually enjoyed the movie"),
            "I actually enjoyed the movie"
        );
    }

    #[test]
    fn no_trigger_round_trips_unchanged() {
        assert_eq!(
            try_backtrack("just a normal sentence"),
            "just a normal sentence"
        );
    }

    #[test]
    fn trigger_with_nothing_after_it_is_left_alone() {
        assert_eq!(try_backtrack("wait, scratch that"), "wait, scratch that");
    }

    #[test]
    fn repeated_corrections_keep_only_the_last_one() {
        assert_eq!(
            try_backtrack("the meeting is at two, actually three, actually four"),
            "the meeting is at four"
        );
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            try_backtrack("book it for monday SCRATCH THAT tuesday"),
            "book it for tuesday"
        );
    }

    #[test]
    fn empty_string_round_trips() {
        assert_eq!(try_backtrack(""), "");
    }
}
