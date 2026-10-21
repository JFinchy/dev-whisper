# Names from your project

Branch: `worktree-agent-ab1d06cdde738ffb2`
Files changed: `src-tauri/src/names.rs` (new), `src-tauri/src/project.rs` (new),
`pipeline.rs`, `recording.rs`, `stt.rs`, `app_detect.rs`, `config.rs`,
`src/settings/sections/vocabulary.tsx`

## What to verify

In a terminal or editor, spoken code names become the project's real names:
"fix the paste text function in the paste file" pastes
`fix the paste_text function in the @src-tauri/src/paste.rs`. Unit tests use
fake name lists; only a human can confirm that the frontmost terminal or
editor resolves to the right repo, and that real Whisper output matches.

## Steps

1. `bun run tauri dev` (or the installed app). Settings -> Vocabulary: "Use
   names from your project" is checked.
2. Open Terminal (or iTerm/Warp), `cd` into a repo (try this one), and start
   Claude Code or stay at the shell. Make sure the terminal's mode is CLI.
3. Hold the shortcut and say: "Fix the paste text function in the paste
   file." Expect `Fix the paste_text function in the @src-tauri/src/paste.rs`
   (the `@` and full path are for CLI mode).
4. Say: "Look at file tagging dot rs." Expect `@src-tauri/src/file_tagging.rs`.
5. Say: "Just paste text into the box." Expect it unchanged (no identifier
   without a code word like "function" beside it).
6. Open two terminal tabs in different repos, type something in the second,
   and dictate: the second tab's repo names are used. (Terminal.app, iTerm.)
7. Open the repo in VS Code or Cursor, dictate into its editor or terminal
   pane: "the paste text function" -> `paste_text`. If the window title is
   unreadable (no Screen Recording permission), VS Code and Cursor still work
   through their saved open-folder state; Zed and JetBrains need the title.
8. Settings -> Vocabulary now reads "Last project: <repo> · N names".
9. Dictate into Slack or Messages: nothing is substituted.
10. Uncheck the setting, dictate step 3 again: unchanged. In `app.log` look
    for `project:` lines: counts and timings only, no file or function names.
11. First dictation in a big repo: no added delay (indexing runs while you
    speak); compare the `perf:` lines against a dictation outside a repo.

## Pass criteria

Steps 3-5 and 8-10 as described, step 6 picks the right tab's repo, and the
`project:` log lines never contain a name (the existing `modes:` and `llm:`
lines still print the transcript text itself, as before).
