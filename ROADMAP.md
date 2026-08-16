# Roadmap

Where things stand and what's next, in priority order. See `PRODUCT_SPEC.md`
for the product vision and `BACKLOG.md` for the full detail behind every
item here (known issues, research findings, effort estimates).

## Shipped

The Day One MVP (push-to-talk → local Whisper → paste) plus a full round of
iteration on top of it: mic/shortcut/model settings, app-aware modes with
real LLM refinement via Ollama, a vocabulary editor, transcript history,
an app-icon picker with a curated common-apps list, and an in-app Ollama
model catalog (Gemma, Llama, Phi, Qwen, Mistral) with download progress.

Also shipped since: per-mode STT model switching backed by a small LRU of
warm whisper contexts (`stt.rs`); Casual mode defaulting to real LLM
refinement; last-observed LLM refinement latency surfaced per model in the
LLM settings picker; Syntax & Casing Commands (`syntax.rs`, pure Rust, no
LLM needed); and Boilerplate Generation (`boilerplate.rs` +
`llm::generate_boilerplate`) — "generate boilerplate for a React
component called UserCard with name and avatar props" sends the request to
the local LLM and pastes the generated code, with a longer (60s) timeout
than cleanup refinement and markdown-fence stripping since small models
often wrap code in fences despite being told not to. Also shipped:
auto-launch at login (off by default), a tray icon that reflects recording
state, a "copy only" toggle that skips the simulated paste, and an in-app
log viewer.

Also shipped (2026-08-16): three widget display modes (minimal/compact/
detailed, `widget.rs` + WidgetView.tsx) with a Settings picker, and a fix
for error messages getting silently truncated in the old fixed-size
widget (compact now auto-grows and holds errors on screen for 6s); a
wider Settings window (520px, was 380px); and a real app icon + menu-bar
tray glyph (mic-themed, `icons/icon.png` + `icons/tray-icon-template.png`)
replacing the default Tauri icon, with the tray glyph rendered as a
macOS template image so it auto-adapts to light/dark menu bars.

## Next — developer-centric features LLM refinement now unlocks

- **The Local Config Agent** — voice-driven edits to `~/.zshrc`, Neovim
  `init.lua`, MCP settings. Different in kind from the two shipped above:
  this needs filesystem *write* access driven by voice, so it needs its
  own safety design (confirmation step, diff preview before applying)
  rather than just a new prompt template. Not started — deliberately
  deferred, see BACKLOG.md.

## Later — redesign & research-gated

- **Full Modes UI redesign** to match SuperWhisper: named user-created
  presets (not a fixed Plain/Casual/CLI enum), per-mode shortcuts, a
  categorized app picker, sidebar navigation instead of one long
  scrolling Settings window. The backend groundwork (`app_detect.rs`,
  `modes.rs`, `llm.rs`, `history.rs`) is solid; this is now a UI/data-model
  redesign, not new backend work.
- **Parakeet as an alternative STT model** — researched, not implemented.
  `parakeet-rs` carries the hard parts, but Apple Silicon acceleration is
  unresolved (CoreML flagged unstable by the crate's own maintainer).
  Recommended as a validate-first spike on real hardware before building
  any Settings/download UI for it.
- **Single-speaker voice isolation** — reject background music/other
  talkers so only the primary user's voice reaches Whisper. Needs its own
  research/design pass (VAD vs. speaker-embedding vs. source separation).
  Still nothing implemented as of 2026-08-16 — recommended as its own
  focused session rather than folded into unrelated UI work.

## Future state (from `PRODUCT_SPEC.md` §5, unchanged)

The longer-term vision beyond push-to-talk dictation: continuous
full-duplex voice interaction, a Socratic "Rubber Ducking" mode, and local
TTS for a full conversational loop. Nothing started here yet — everything
above is prerequisite groundwork.
