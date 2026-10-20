# Backtrack (spoken self-correction)

Branch: `main` (`bf6500f`, 2026-08-17; never queued for a manual pass)
Files changed: `src-tauri/src/backtrack.rs` (new), `src-tauri/src/recording.rs`

## What to verify

"…, actually …" and "scratch that" correct what came just before: a short
correction (up to 3 words) swaps that many words, a longer one replaces
the sentence it interrupts. (Reworked 2026-10-19; it used to discard
everything before the trigger.) Unit tests cover the text rules; real speech decides
whether Whisper produces the comma that "actually" depends on.

## Steps

1. Dictate "Let's meet at two, actually three." Pastes "Let's meet at
   three." (Whisper may write the numbers as digits.)
2. Dictate "Send it to Sam scratch that send it to Alex." Pastes "Send it
   to Alex." Dictate "I fixed the build. The tests are flaky, scratch
   that, they all pass now." Pastes "I fixed the build. They all pass now."
3. Dictate "Actually I think it's fine" (no preceding comma). Pastes
   unchanged.
4. Dictate "well, actually, I think it's fine". Known false positive:
   expect only the tail to paste. Note whether this happens often enough
   in normal speech to be annoying.

## Pass criteria

Steps 1–3 as described. For step 4, record how often it misfires in real
use rather than pass/fail.
