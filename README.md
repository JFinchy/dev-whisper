# Dev Whisper

Local, privacy-first dictation for developers. See `FEATURES.md` for
everything currently built, `PRODUCT_SPEC.md` for the product vision,
`ROADMAP.md`/`BACKLOG.md` for what's next, and `MANUAL_TESTS.md` for
things awaiting live-mic verification.

## Setup

```sh
bun install
./scripts/download-model.sh   # downloads the ggml whisper model (~57MB)
bun run tauri dev
```

The global push-to-talk hotkey doesn't need Accessibility permission (it
uses Carbon's `RegisterEventHotKey`, not an event tap), but pasting the
transcript does, since that's a simulated Cmd+V. macOS prompts for it on
first launch; microphone capture prompts separately on first recording.

**Dev-mode quirk**: each `cargo`/`tauri build` produces a freshly (ad-hoc)
signed binary, and macOS ties the Accessibility grant to that exact
signature. So after every rebuild, paste may silently stop working until
you re-grant it in System Settings > Privacy & Security > Accessibility —
remove the stale "Dev Whisper" entry and re-add it, or just toggle it off
and on. The app now detects this and shows an error instead of pasting
silently, which is what surfaces this. Signed release builds won't have
this problem.

The push-to-talk shortcut and Whisper model are configurable from the
Settings window (gear icon on the widget) and persist across restarts.

**Known limitation — iPhone Microphone (Continuity)**: selecting your
iPhone as the input device is unreliable. In testing, the stream would
sometimes open and play but deliver zero audio frames to the capture
callback for the entire recording (confirmed via `audio: first callback
received` logging — sometimes it never printed at all), while other
attempts with the identical device worked fine. This looks like a race in
Continuity's connection handshake interacting with `cpal`'s low-level
CoreAudio `AudioUnit` input path, which is a different code path than the
one Apple's own apps use (`AVAudioEngine`) — not a bug in this app's
start/stop lifecycle, which was verified clean (open → play → pause →
drop) in every case. `audio.rs` retries opening the device a few times
and falls back to the system default if it still won't open, which fixes
the "stale device handle right after selecting it" failure mode, but not
this "opens fine, delivers no data" one. If you want to use your iPhone
as the mic, expect to retry the recording a time or two; for anything
important, use a built-in/USB/Bluetooth mic instead.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
