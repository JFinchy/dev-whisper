# Dev Whisper

Local, privacy-first dictation for developers. See `PRODUCT_SPEC.md` for the
full product spec and build plan.

## Setup

```sh
bun install
./scripts/download-model.sh   # downloads the ggml whisper model (~57MB)
bun run tauri dev
```

The global push-to-talk hotkey (Cmd+Shift+Space) and paste simulation
require macOS Accessibility permission; microphone capture requires
Microphone permission. macOS will prompt for both on first use.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
