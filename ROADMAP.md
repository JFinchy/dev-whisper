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

## Now — quick wins

Small, bounded, high-value. Good next session to pick up.

- **Wire per-mode STT model switching** — `AppModeRule.stt_model` exists in
  the data model and Settings UI but doesn't actually switch
  `WhisperEngine`'s model yet. Needs a small pool of warm contexts (LRU of
  2-3 loaded models) so switching doesn't repay the multi-second Metal
  shader compile on every recording that hits a different-model rule.
- **Default Casual mode to LLM refinement** — its rule-based path is a
  no-op today (regex can't do "sound casual"); now that refinement is
  real, flip the default instead of leaving it manual.
- **Surface LLM speed in the model picker** — after the "thinking model"
  latency surprise, worth showing users which local models are actually
  fast enough for this use case rather than finding out the hard way.

## Next — developer-centric features LLM refinement now unlocks

These were in the original spec's core feature list but were blocked on
not having a real LLM refinement pipeline. That pipeline exists now.

- **Syntax & Casing Commands** — "snake case error response handler" →
  `error_response_handler`. A new prompt template + mode, similar shape to
  the existing CLI-mode refinement. Should be a fast add.
- **Boilerplate Generation** — "dictate boilerplate React functional
  component named UserProfile" → streams real code to the cursor. Needs a
  longer LLM timeout than cleanup (code gen takes longer than a 1-2s
  refinement pass) and probably its own mode.
- **The Local Config Agent** — voice-driven edits to `~/.zshrc`, Neovim
  `init.lua`, MCP settings. Different in kind from the above two: this
  needs filesystem *write* access driven by voice, so it needs its own
  safety design (confirmation step, diff preview before applying) rather
  than just a new prompt template. Not started.

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

## Future state (from `PRODUCT_SPEC.md` §5, unchanged)

The longer-term vision beyond push-to-talk dictation: continuous
full-duplex voice interaction, a Socratic "Rubber Ducking" mode, and local
TTS for a full conversational loop. Nothing started here yet — everything
above is prerequisite groundwork.
