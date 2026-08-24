# Dev Whisper

Local, privacy-first dictation for developers. See `FEATURES.md` for
everything currently built, `PRODUCT_SPEC.md` for the product vision, and
`ROADMAP.md`/`BACKLOG.md` for what's next.

## Install (the app itself, not the source)

There's no signed/notarized release yet — no DMG to download, no GitHub
Releases page. For now, "installing" means building it once and keeping
the resulting app; you don't need to touch this again after that.

```sh
bun install
bun run tauri build
```

That produces `Dev Whisper.app` in `src-tauri/target/release/bundle/macos/`,
plus a `Dev Whisper_0.1.0_aarch64.dmg` in
`src-tauri/target/release/bundle/dmg/` (drag-to-Applications, like any other
Mac app) — open the DMG and drag `Dev Whisper.app` into `/Applications`.

**First launch**: the app isn't notarized, so Gatekeeper will refuse to
open it normally ("Apple could not verify..."). Right-click (Control-click)
`Dev Whisper.app` in Applications and choose **Open** — this extra prompt
only happens once. macOS will then separately ask for Microphone access
(for recording) and Accessibility access (for pasting the transcript);
both are required for the app to actually work, not optional extras.

**Download a Whisper model**: a fresh install has no speech model on disk
yet (the `./scripts/download-model.sh` step below is a dev-only shortcut,
not something a real install runs). Open Settings — gear icon on the
floating widget — → Dictation → Models, and download one; Base (~57MB) is
a reasonable default. Nothing will transcribe until that finishes.

**Optional — LLM refinement**: install [Ollama](https://ollama.com) and
pull a small model (Settings → Dictation → LLM refinement offers one-click
downloads for a few small models) if you want app-aware cleanup/tone
refinement. The app works without it, just with plainer/unrefined output.

## Dev setup

For working on the app itself — runs from source with hot reload instead
of building a standalone `.app`.

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
