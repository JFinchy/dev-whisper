# Roadmap

What's left, in priority order. See `FEATURES.md` for everything already
built, `PRODUCT_SPEC.md` for the product vision, and `BACKLOG.md` for full
detail behind each item here (known issues, research findings, effort
estimates). Shipped work moves out of this file into `FEATURES.md` instead
of lingering here struck through — this file is meant to answer "what's
left," not "what's the history."

## Now — from competitive research (SuperWhisper 2026-08-16, broader landscape 2026-08-17)

Ranked by leverage; see `BACKLOG.md` for the full research writeups.

1. **Direct coding-agent integration** (Claude Code/Cursor/etc. plugins,
   a coding-agent panel, hook support) — the developer-specific angle
   SuperWhisper itself is now leaning into; local/private is our edge
   over their approach.
2. **Selected-text/on-screen context feeding LLM refinement** — extends
   shipped app-aware refinement and boilerplate generation with real code
   context, not just app identity. VoiceInk already ships this (via
   SelectedTextKit) — validated as achievable, not speculative.
3. **Realtime streaming transcript display** — partial results shown
   while still speaking, not just after release. Real UX upgrade to the
   shipped push-to-talk loop.
4. **Output-side workflow automation** — route a finished transcript or
   journal summary to Notion/Slack/a custom webhook, the way MacWhisper
   does. Complements the deep-link hooks we already shipped, which only
   cover the *input* side (starting/stopping a recording via script).
5. **Snippet library** — a spoken cue expands to a saved block of text
   (a PR checklist, a calendar link, standard onboarding instructions),
   distinct from the vocabulary editor (which is about recognition
   accuracy, not insertion). Seen on Wispr Flow.
6. **History reprocessing + full-text search** — history is currently
   append-only with no way to re-run a past recording through a
   different mode or search it.

## In testing

- **Isolated Voice mode** — Settings toggle filtering a recording down to
  the primary user's voice before Whisper transcribes it. Both phases now
  built: phase 1's energy-gate background-noise filtering (merged, tested)
  and phase 2's voice enrollment + speaker-embedding cosine-similarity
  masking (sherpa-onnx + a bundled WeSpeaker model), which can actually
  reject a second person talking, not just quiet noise. Needs manual
  end-to-end verification (real enrollment, background-voice rejection,
  threshold tuning) before it's considered done. See `BACKLOG.md` for the
  full design.

## Next — needs its own design pass before implementation

- **The Local Config Agent** — voice-driven edits to `~/.zshrc`, Neovim
  `init.lua`, MCP settings. Needs filesystem *write* access driven by
  voice, so it needs a safety design (confirmation step, diff preview
  before applying) rather than just a new prompt template. Not started —
  deliberately deferred, see `BACKLOG.md`.
- **Stable local codesigning identity** — each dev rebuild produces a
  differently-signed binary, so macOS revokes the Accessibility grant
  every time, which has now caused a real incident (paste silently
  stopped working — see `BACKLOG.md`). Fix is a self-signed certificate
  used consistently for dev builds; touches Keychain trust settings, so
  better done live than unsupervised.

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

## Future state (from `PRODUCT_SPEC.md` §5, unchanged)

The longer-term vision beyond push-to-talk dictation: continuous
full-duplex voice interaction, a Socratic "Rubber Ducking" mode, and local
TTS for a full conversational loop. Nothing started here yet — everything
above is prerequisite groundwork.
