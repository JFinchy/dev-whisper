# Dictation accuracy fixes from the real-audio e2e suite

Branch: `main` (built directly on main, 2026-10-19)
Files changed: `syntax.rs`, `punctuation.rs`, `backtrack.rs`,
`file_tagging.rs`, `pipeline.rs` (new), `e2e_audio.rs` (new),
`tests/fixtures/*.json`

## What to verify

These fixes came from feeding synthesized speech through Whisper, and the
golden and audio suites cover them. Your own voice and mic are what
matter, though: real speech has different pauses, and Whisper punctuates
it differently.

## Steps

1. Say "snake case error response handler" → `error_response_handler`.
   Say "camel case user profile ID" → `userProfileId`.
2. Say, pausing between items: "One, fix the tests. Two, ship it." → a
   two-line numbered list. Say "I have one question about the two failing
   tests." → stays one sentence.
3. Say "Let's meet at two, actually three." → "Let's meet at three."
   Say "I fixed the build. We should use Postgres, actually let's go with
   SQLite." → the first sentence survives.
4. Say "Hello comma world period this is a test" → "Hello, world. This is
   a test".
5. With "press enter" on: "Run the tests, press enter." → "Run the tests"
   and Enter, no trailing comma.
6. In a terminal (Claude Code): "check src slash lib dot rs" and "check
   lib.rs" → whichever comes out as a file name gets an `@`.
7. **Refine only when needed** (Settings → LLM, on by default), in a mode
   with LLM refinement (your Terminal mode): dictate a clean sentence.
   It pastes with no violet "Refining" stage and `app.log` shows
   `transcript already clean, skipping`. Dictate "um, so, check the the
   logs": the violet "✦ Refining N%" stage appears, the hover tooltip
   names the model, and the percentage climbs smoothly to near 100% as
   the text lands.
8. **Paste as-is**: in Terminal mode, dictate something with "um" in it.
   While the violet bar shows, click it: the unrefined text pastes at
   once and `app.log` shows `skipped by the user`. Dictate again and press
   the hotkey plus Shift (⇧⌘D for ⌘D) while still talking: recording
   stops and it pastes with no violet stage.
9. **Vocabulary**: add a project name to Vocabulary, then say a sentence
   using it with "um" (so it refines). The LLM output spells it your way.
10. **Transcription progress**: a 10-20s dictation shows "Transcribing N%"
   in amber, rising steadily; the recording timer counts while you talk.
11. If anything comes out wrong, copy the `whisper: raw transcript=` line
   from `app.log` into `src-tauri/tests/fixtures/pipeline_cases.json` as a
   new case (see TESTING.md).

## Pass criteria

Steps 1–10 behave as described with your own voice.
