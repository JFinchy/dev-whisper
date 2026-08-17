# Voice commands (named punctuation, numbered lists, press enter)

Branch: `JFinchy/undine` (merged into `main` before the manual-testing-inbox
convention existed, so this is being added retroactively rather than
having traveled with the original merge)
Files changed: `src-tauri/src/punctuation.rs` (new), `src-tauri/src/paste.rs`,
`src-tauri/src/config.rs`, `src-tauri/src/recording.rs`, `src/SettingsView.tsx`

## What to verify

Three deterministic (no-LLM) pre-processing passes on the raw transcript,
each with its own automated unit tests but no live-microphone coverage:
named punctuation words → literal characters, spoken numbered lists →
real `1. / 2. ...` lists, and a "press enter" trailing phrase → a
simulated Enter keystroke. All three run before casing/boilerplate/mode
formatting.

## Steps

1. `bun run tauri dev`, tray icon → mic should be idle.
2. **Named punctuation**: dictate into any text field: "hello comma world
   period new line this is a test". Confirm it pastes as:
   ```
   hello, world.
   this is a test
   ```
3. **Numbered lists**: dictate "one buy milk two walk the dog three call
   mom". Confirm it pastes as a real numbered list:
   ```
   1. buy milk
   2. walk the dog
   3. call mom
   ```
4. **Press enter — off by default**: open Settings → confirm the "press
   enter" toggle exists and is **off**. Dictate "test message press
   enter" into a chat-style input (e.g. Messages, Slack). Confirm the
   literal words "press enter" get pasted as text (since the feature is
   off) — no Enter should fire.
5. **Press enter — enabled**: turn the toggle on in Settings. Repeat the
   same dictation. Confirm "press enter" is stripped from the pasted
   text and Enter actually fires (message sends / newline advances).
6. **Press enter — copy-only interaction**: with "press enter" still on,
   also enable "Copy only" in Settings. Dictate "test press enter" again
   — confirm text is copied to the clipboard but Enter does **not**
   fire (copy-only means no synthetic keystrokes at all).
7. **Press-enter-only utterance**: with "press enter" on and copy-only
   off, dictate *just* "press enter" with nothing else. Confirm: nothing
   is pasted/copied (clipboard isn't clobbered with an empty string),
   Enter still fires, and no new entry appears in Settings → History.

## Pass criteria

All six behaviors in steps 2-7 match exactly, with no regression to a
plain dictation with none of these trigger phrases (say something
ordinary and confirm it pastes completely unchanged).
