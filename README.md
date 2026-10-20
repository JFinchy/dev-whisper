# Dev Whisper

Local, private dictation for developers on Apple Silicon. Hold a hotkey,
talk, and the text lands at your cursor, formatted for the app you're in:
prose in Slack, commands and `@file` tags in a terminal coding agent,
`snake_case` when you ask for it. Speech recognition (whisper.cpp on
Metal) and optional cleanup (Ollama) both run on your Mac; nothing is sent
anywhere.

- What it does today: [`FEATURES.md`](FEATURES.md)
- Where it's going: [`ROADMAP.md`](ROADMAP.md), [`BACKLOG.md`](BACKLOG.md),
  [`PRODUCT_SPEC.md`](PRODUCT_SPEC.md)
- How it's tested: [`TESTING.md`](TESTING.md)
- Cutting a release: [`RELEASING.md`](RELEASING.md)

## Install

**[Download Dev Whisper for Mac (Apple Silicon)](https://github.com/JFinchy/dev-whisper/releases/latest/download/Dev-Whisper.dmg)**:
open it and drag Dev Whisper into Applications. Always the newest
version; older ones are on the
[Releases](https://github.com/JFinchy/dev-whisper/releases) page. The app isn't notarized yet, so the first
launch needs right-click (Control-click) → **Open**.

On first launch, Settings shows a setup checklist until everything's in
place:

1. **Speech model**: download one under Voice (Base, ~57MB, is a good
   default).
2. **Pasting**: allow Dev Whisper under System Settings → Privacy &
   Security → Accessibility (the checklist's button opens that page).
   Not needed if you turn on copy-only delivery.
3. **Microphone**: macOS asks on the first recording.
4. **Optional, LLM cleanup**: install [Ollama](https://ollama.com) and
   download a small model from Settings → LLM.

Updates install themselves: the app checks GitHub on launch, shows a dot
on the widget's gear when a new version is out, and Settings → About
installs it. Update packages are signature-checked.

## Development

```sh
bun install
./scripts/download-model.sh   # Whisper base.en into src-tauri/models (checksum-verified)
bun run tauri dev             # the app, with hot reload
bun run dev:mock              # just the UI in a browser, on a fake backend (http://localhost:1430/?window=settings)
bun run test                  # typecheck + Rust tests + UI e2e
```

`dev:mock` runs the React UI against the in-memory backend in
`src/testing/mockBackend.ts`, so UI work doesn't need a Rust rebuild.
Append `?window=widget` or `?window=settings` to pick the window.

### Stable dev signing

macOS ties Accessibility and Input Monitoring grants to an app's code
signature, and every `cargo` build is signed with a new ad-hoc one, so
each rebuild used to lose those grants. `.cargo/config.toml` runs
`scripts/dev-sign-runner.sh`, which re-signs the dev binary with a fixed
local identity before launching it. One-time setup, in your own Terminal
(the trust step asks for your Mac password):

```sh
./scripts/setup-dev-signing.sh
```

Then run `bun run tauri dev`, click **Always Allow** if `codesign` asks to
use the key, and grant Accessibility to the dev app one last time.

Without the certificate the runner launches unsigned and says so. If
signing stalls on a keychain prompt, it gives up after 20 seconds and
launches unsigned rather than hanging.

### Project layout

| Path | What's there |
|---|---|
| `src-tauri/src/recording.rs` | The dictation flow: record → transcribe → format → deliver |
| `src-tauri/src/pipeline.rs` | Text formatting, pure and testable (lists, punctuation, backtrack, commands, modes, LLM) |
| `src-tauri/src/readiness.rs` | Setup checklist |
| `src-tauri/src/{audio,stt,llm,paste}.rs` | Mic capture, Whisper, Ollama, paste |
| `src-tauri/src/ambient.rs` | Hands-free "Dev Whisper, …" listening |
| `src/WidgetView.tsx` | The floating widget |
| `src/SettingsView.tsx`, `src/settings/` | Settings window, one file per page |
| `e2e/` | Playwright UI tests |
| `src-tauri/tests/fixtures/` | Golden dictation cases and real-audio cases |

Logs: `~/Library/Application Support/com.devwhisper.app/app.log` (also
Settings → Advanced → Logs).

## Known limitations

- **iPhone (Continuity) microphone** is unreliable: the stream sometimes
  opens but delivers no audio. The widget now flags this as "No sound from
  mic" within ~2 seconds; use a built-in, USB or Bluetooth mic for
  anything important.
- **Not notarized**, so Gatekeeper needs the right-click → Open step on
  first install.
