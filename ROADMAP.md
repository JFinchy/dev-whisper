# Roadmap

What's left, in priority order. See `FEATURES.md` for everything already
built, `PRODUCT_SPEC.md` for the product vision, and `BACKLOG.md` for full
detail behind each item here (known issues, research findings, effort
estimates). Shipped work moves out of this file into `FEATURES.md` instead
of lingering here struck through — this file is meant to answer "what's
left," not "what's the history."

## Now — from competitive research (SuperWhisper 2026-08-16, broader landscape 2026-08-17)

Ranked by leverage; see `BACKLOG.md` for the full research writeups. #2
and #3 bumped up on 2026-08-17 based on explicit user interest, not just
research-leverage ranking — both now have a full implementation design in
`BACKLOG.md`, not just a one-line idea.

1. **Direct coding-agent integration** (Claude Code/Cursor/etc. plugins,
   a coding-agent panel, hook support) — the developer-specific angle
   SuperWhisper itself is now leaning into; local/private is our edge
   over their approach.
2. **Output-side workflow automation** — a generic configurable webhook
   fired after each delivered dictation, covering Notion/Slack/n8n/
   Zapier/Make.com the way MacWhisper's integrations do, without us
   maintaining a bespoke client per service. Complements the deep-link
   hooks already shipped, which only cover the *input* side
   (starting/stopping a recording via script). Off by default — sending
   dictated text off-device is a deliberate opt-in, not a default.
3. **Snippet library** — a spoken cue expands to a saved block of text
   (a PR checklist, a calendar link, standard onboarding instructions),
   distinct from the vocabulary editor (which is about recognition
   accuracy, not insertion). Seen on Wispr Flow. Detected the same way
   as the shipped Syntax & Casing Commands / Boilerplate Generation —
   pure, fast, no LLM needed to look up a saved trigger phrase.
4. **Selected-text/on-screen context feeding LLM refinement** — extends
   shipped app-aware refinement and boilerplate generation with real code
   context, not just app identity. VoiceInk already ships this (via
   SelectedTextKit) — validated as achievable, not speculative.
5. **Realtime streaming transcript display** — partial results shown
   while still speaking, not just after release. Real UX upgrade to the
   shipped push-to-talk loop.
6. **History reprocessing + full-text search** — history is currently
   append-only with no way to re-run a past recording through a
   different mode or search it.

## In testing

- **Isolated Voice mode, phase 1** — Settings toggle + energy-gate
  filtering so background noise doesn't reach Whisper; merged to `main`
  for real-world testing before phase 2 (voice enrollment +
  speaker-embedding isolation, so a second person talking can actually be
  rejected, not just quiet noise) is built. See `BACKLOG.md` for the full
  design and what phase 2 needs.

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

- **Command Mode** (Talon-inspired) — a second, opt-in always-listening
  mode alongside today's push-to-talk dictation: short spoken phrases
  mapped to actions (keystrokes, shell commands, our own deep-link hooks)
  instead of formatted text. See `COMMAND_MODE.md` for the full
  feature-by-feature breakdown with tasks/subtasks — this is bigger than
  everything else in this file combined, mainly because it needs a new
  continuous-listening/VAD audio pipeline, not an extension of the
  existing explicit start/stop one. Not started.
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
