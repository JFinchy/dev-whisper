# Roadmap

What's left, in priority order. See `FEATURES.md` for everything already
built, `PRODUCT_SPEC.md` for the product vision, and `BACKLOG.md` for full
detail behind each item here (known issues, research findings, effort
estimates). Shipped work moves out of this file into `FEATURES.md` instead
of lingering here struck through — this file is meant to answer "what's
left," not "what's the history."

## In progress — always-on foundation (agreed 2026-10-05)

Groundwork for always-listening dictation, in build order. Extends
`BACKLOG.md`'s "Ambient/hands-free listening mode" entry; a wake phrase is
required before ambient speech pastes, since auto-pasting every detected
utterance would eventually type a side conversation into an editor.

- [x] Persistent `app.log`, 30s heartbeat, signal + exit logging (to
  diagnose the dev process dying silently)
- [x] Capture state machine (`capture.rs`) replacing the `is_recording`
  flag; every trigger (hotkey, double-tap Fn, tray, deep link) goes
  through one request API, transitions/rejections logged
- [x] Device status: picked mic missing → keep falling back, but surface
  it (event + widget pill), log only on change
- [x] Codebase bug review pass (fix what's found)
- [x] Silero VAD (already in linked `sherpa-onnx`) for ambient speech
  detection
- [x] Pre-roll buffer (1s) + silence endpointing (700ms, 30s cap)
- [x] Ambient mode: speaker-gated (existing voice enrollment), wake
  phrase required before anything pastes, paste guard (same frontmost
  app), green tray dot + widget label, voice sleep/wake, 10-min
  auto-sleep
- [x] Pause while another app is using the mic (calls, incl. browser
  Meet), via CoreAudio's per-process input list; resumes on its own
- [x] Ambient metrics in `app.log` (segments, speaker rejects, wake hits)
- [x] Terminal paste review: CLI-mode LLM no longer synthesizes shell
  commands (root cause of every bad terminal paste), refinement output is
  logged and sanity-checked, file tags survive refinement, line breaks
  survive file tagging, "press enter" waits for async terminal pastes
- [ ] Manual verification: enroll, enable, tune VAD/voice-match thresholds
  from `app.log` metrics (needs real speech, see manual-testing-inbox)

## Production readiness (from `AUDIT.md`, 2026-10-19)

The 2026-10-19 audit pass is done (see `AUDIT.md` and `FEATURES.md`).
Still open, in order:

1. Verify the hardening in the built app (`manual-testing-inbox/production-hardening.md`)
2. Notarization (needs an Apple Developer account)
3. Opt-out for deep links starting a recording
4. Microphone permission in the setup checklist

## Now — from competitive research (SuperWhisper 2026-08-16, broader landscape 2026-08-17)

Ranked by leverage; see `BACKLOG.md` for the full research writeups. #2
and #3 bumped up on 2026-08-17 based on explicit user interest, not just
research-leverage ranking — both now have a full implementation design in
`BACKLOG.md`, not just a one-line idea.

1. **Direct coding-agent integration** (Claude Code/Cursor/etc. plugins,
   a coding-agent panel, hook support) — the developer-specific angle
   SuperWhisper itself is now leaning into; local/private is our edge
   over their approach.
2. **Snippet library** — a spoken cue expands to a saved block of text
   (a PR checklist, a calendar link, standard onboarding instructions),
   distinct from the vocabulary editor (which is about recognition
   accuracy, not insertion). Seen on Wispr Flow. Detected the same way
   as the shipped Syntax & Casing Commands / Boilerplate Generation —
   pure, fast, no LLM needed to look up a saved trigger phrase.
3. **Selected-text/on-screen context feeding LLM refinement** — extends
   shipped app-aware refinement and boilerplate generation with real code
   context, not just app identity. VoiceInk already ships this (via
   SelectedTextKit) — validated as achievable, not speculative.
4. **Realtime streaming transcript display** — partial results shown
   while still speaking, not just after release. Real UX upgrade to the
   shipped push-to-talk loop.

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
