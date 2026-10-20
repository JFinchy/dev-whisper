# Stable dev signing + crash-proof logging

Branch: `main` (2026-10-07)
Files changed: `scripts/dev-sign-runner.sh` (new), `.cargo/config.toml`,
`src-tauri/src/logging.rs`, `src-tauri/src/lifecycle.rs`, `src-tauri/src/lib.rs`

## What to verify

Dev builds are signed with a local "Dev Whisper Local Signing" identity
so macOS keeps Accessibility/Input Monitoring across rebuilds. Logging no
longer panics when the Tauri CLI exits, which was what killed the dev app
silently. Everything but one end-to-end paste is already verified
(below).

## Already verified (2026-10-12)

- Dev build signed with the stable identity:
  `codesign -d -r- src-tauri/target/debug/tauri-app` →
  `identifier "com.devwhisper.app.dev" and certificate leaf ...`
- Accessibility survived a rebuild: after a code change and re-sign,
  `app.log` showed `accessibility: trusted=true` with no prompt. (The dev
  build is listed as "Dev Whisper" in Accessibility, not "tauri-app". No
  manual add needed.)
- App survives its Tauri CLI exiting: heartbeats continued, and SIGTERM
  logged `lifecycle: received signal 15, exiting`.

## Steps (remaining)

1. Click into a text field (Notes, or any editor).
2. Press **Cmd+D**, say "testing dev signing", press **Cmd+D** again.
3. The words paste into the field.

## Pass criteria

The dictation pastes with no permission prompt.
