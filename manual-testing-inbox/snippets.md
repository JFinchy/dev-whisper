# Snippet library

Branch: `main` (`28f41d7`, 2026-08-17; never queued for a manual pass)
Files changed: `src-tauri/src/snippets.rs` (new), `src-tauri/src/config.rs`,
`src-tauri/src/recording.rs`, `src/SettingsView.tsx`

## What to verify

Saying a saved trigger phrase as the *entire* dictation pastes its saved
text block instead (four defaults: "pr checklist", "standup update", "bug
report template", "commit message template"). Unit tests cover matching;
nothing has exercised it with real speech or the Settings editor.

## Steps

1. `bun run tauri dev`. Dictate just "PR checklist" into a text field. The
   five-item `- [ ]` checklist pastes, not the words.
2. Dictate "standup update." (with Whisper's trailing period) — still
   expands.
3. Dictate "here is the PR checklist for today" — pastes as ordinary text
   (a trigger only fires as the whole utterance).
4. Settings → Vocabulary → Snippets: add a snippet (trigger "my address", any body),
   dictate it, confirm it expands. Edit, then delete it; confirm each
   change takes effect on the next dictation and survives a restart.
5. Dictate a snippet trigger into a CLI-mode app (Terminal/Orca): the body
   pastes unchanged, not rewritten by the LLM, and keeps its line breaks.

## Pass criteria

Steps 1–5 as described; History records these with mode `snippet`.
