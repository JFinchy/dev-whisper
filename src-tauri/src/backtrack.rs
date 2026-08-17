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
/// When a trigger is found, everything before and including it is
/// discarded — "X, actually Y" becomes "Y", not some partial diff between
/// X and Y the way Wispr's fancier version behaves (e.g. their own "at 2
/// actually 3" example keeps "at" and only swaps the number — this pass
/// has no way to do that kind of partial correction deterministically).
/// If multiple triggers appear, the last one wins, so repeated
/// self-corrections collapse to the final, most-recent one.
pub fn try_backtrack(text: &str) -> String {
    let lower = text.to_lowercase();

    let comma_actually = find_last(&lower, ", actually");
    let scratch_that = find_last(&lower, "scratch that");

    let trigger_end = match (comma_actually, scratch_that) {
        (Some((cs, ce)), Some((ss, se))) => Some(if cs > ss { ce } else { se }),
        (Some((_, ce)), None) => Some(ce),
        (None, Some((_, se))) => Some(se),
        (None, None) => None,
    };

    let Some(trigger_end) = trigger_end else {
        return text.to_string();
    };

    // `to_lowercase()` on these ASCII trigger phrases doesn't change byte
    // length, so `trigger_end` stays aligned with the original-casing
    // `text` — same technique as `syntax::try_apply_casing_command`.
    let remainder = text[trigger_end..].trim_start_matches([',', '.', ' ']).trim();

    if remainder.is_empty() {
        // Nothing to replace with (e.g. "scratch that" was the last thing
        // said) — leave the text alone rather than discarding everything.
        return text.to_string();
    }

    remainder.to_string()
}

/// Rightmost case-insensitive match of `needle` in the already-lowercased
/// `lower`. Returns `(start, end)` byte offsets.
fn find_last(lower: &str, needle: &str) -> Option<(usize, usize)> {
    lower.rmatch_indices(needle).next().map(|(start, m)| (start, start + m.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_that_discards_everything_before_it() {
        assert_eq!(
            try_backtrack("call the client at three scratch that four"),
            "four"
        );
    }

    #[test]
    fn comma_actually_discards_everything_before_it() {
        assert_eq!(try_backtrack("let's do coffee at two, actually three"), "three");
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
        assert_eq!(try_backtrack("just a normal sentence"), "just a normal sentence");
    }

    #[test]
    fn trigger_with_nothing_after_it_is_left_alone() {
        assert_eq!(try_backtrack("wait, scratch that"), "wait, scratch that");
    }

    #[test]
    fn repeated_corrections_keep_only_the_last_one() {
        assert_eq!(
            try_backtrack("the meeting is at two, actually three, actually four"),
            "four"
        );
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(try_backtrack("book it for monday SCRATCH THAT tuesday"), "tuesday");
    }

    #[test]
    fn empty_string_round_trips() {
        assert_eq!(try_backtrack(""), "");
    }
}
