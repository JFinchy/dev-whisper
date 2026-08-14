# Dev Whisper

Local, privacy-first dictation for developers. See `PRODUCT_SPEC.md` for the
full product spec and build plan.

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

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
