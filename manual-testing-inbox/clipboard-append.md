# Append clipboard

Branch: `JFinchy/hippocamp`
Files changed: `src-tauri/src/clipboard.rs` (new), `src-tauri/src/lib.rs`,
`src-tauri/src/recording.rs`, `src/SettingsView.tsx`, `FEATURES.md`

## What to verify

End a dictation with "append clipboard" (or "paste clipboard"/"insert
clipboard") and whatever's currently on the system clipboard gets appended
to the transcript, raw — after mode formatting/LLM refinement, not through
it. Automated coverage (`cargo test --lib clipboard::`) checks the trigger-
phrase extraction and the append logic against plain strings, but nothing
automated exercises a real clipboard read or a real dictation end to end —
that's what this pass is for. Also new: a "Voice Commands" reference
section in Settings documenting this and the other trigger phrases
(casing, punctuation, boilerplate, press enter) — worth a glance too since
it's new UI, not just new backend behavior.

## Steps

1. From repo root: `bun run tauri dev`. Look for the mic icon in the menu
   bar (not a window).
2. Copy some text outside the app — e.g. select an error message in a
   browser and Cmd+C.
3. Push-to-talk and say something like "here's the error append clipboard".
   Confirm the pasted result is your spoken text, a space, then the exact
   copied text — not reformatted, not run through the LLM even if the
   active app's Mode has LLM refinement on.
4. Repeat with the other two trigger phrases ("paste clipboard", "insert
   clipboard") to confirm all three work.
5. Say only the trigger phrase alone (e.g. just "paste clipboard", nothing
   else). Confirm the clipboard content pastes by itself, with no stray
   leading space.
6. Clear the clipboard (or copy something like a screenshot — non-text) and
   trigger the phrase again. Confirm the dictated text still pastes
   normally with nothing appended (check Settings → Logs for a
   `clipboard: append triggered but clipboard was empty or unreadable`
   line), and confirm nothing crashes.
7. Say the trigger phrase mid-sentence (e.g. "append clipboard to the
   document please") and confirm it does *not* fire — it should be treated
   as ordinary dictated text, not a command, since the trigger only matches
   at the very end of the utterance.
8. Open Settings and confirm the new **Voice Commands** section (right
   below General) renders correctly — all five groups plus the punctuation
   grid — and that the "Append clipboard" entry is in there.

## Pass criteria

Steps 3-5 all append the real clipboard text correctly with correct
spacing; step 6 degrades gracefully with no crash; step 7 confirms no
false-positive mid-sentence; step 8 confirms the new Settings section
renders without layout issues.
