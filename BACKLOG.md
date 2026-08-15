# Backlog

Tracking for known issues, bugs, and feature ideas that fall outside the
active Day One MVP scope. See `PRODUCT_SPEC.md` for the product vision and
build phases.

## Known Issues

- **iPhone Microphone (Continuity) unreliable** — intermittently captures
  zero audio frames even though the stream opens/plays/stops cleanly.
  Retry+fallback logic mitigates the "stale device handle" failure mode,
  not the "opens but delivers no data" one. See the README's Known
  Limitations section for the full diagnosis. Low priority — works fine
  with a normal mic, and iPhone-as-mic is a nice-to-have.
- **Dev rebuild breaks Accessibility permission** — each `cargo`/`tauri
  build` produces a differently-signed binary, so macOS treats it as a new
  app and the Accessibility grant (needed for paste) has to be re-granted
  after every rebuild during development. Only affects the local dev
  workflow, not signed release builds. Worth a stable local codesigning
  identity if this gets annoying enough.

## Feature Requests

- **App-aware modes, stage 2 (LLM-backed transformation)** — stage 1
  shipped: frontmost-app detection (`app_detect.rs`, `NSWorkspace`, no
  extra permission needed), a mode framework with built-in defaults for
  common apps (`modes.rs`), a Settings UI to add/edit per-app rules, and
  rule-based formatting for CLI mode (a couple of illustrative
  natural-language -> shell-command patterns, e.g. "git commit X" ->
  `git commit -m "X"`). Casual mode is currently a no-op — rule-based
  formatting can't meaningfully do "make this sound casual," it needs
  the LLM refinement item below. Once that exists, wire it in as
  Casual/Cli mode's actual transformation instead of (or alongside) the
  regex patterns.

- **LLM-based transcript refinement** — run the raw Whisper transcript
  through a local LLM before pasting, with a prompt that varies by
  mode/app (e.g. "reformat as a git commit command" vs. "clean up filler
  words but keep it casual"). This is the core of what makes SuperWhisper
  feel smarter than raw dictation, and it's also what
  `PRODUCT_SPEC.md`'s architecture already calls for ("LLM Refinement
  Layer: Ollama API / Llama.cpp") — not yet built; today we paste the raw
  Whisper output untouched.
  - Simplest path: call a locally-running Ollama instance's HTTP API
    (`localhost:11434`) with a mode-specific system prompt. Small lift —
    we already depend on `ureq` for HTTP; needs a graceful fallback to
    the raw transcript if Ollama isn't installed/running.
  - Heavier path: embed a small local LLM directly, same pattern as the
    `whisper-rs` integration (download a GGUF model, run local
    inference). Zero external dependencies, but comparable effort to
    redoing that Phase 3 work.
  - Either way, this adds real latency after transcription (an extra
    inference round-trip) — needs a "Refining…" UI state and probably an
    easy on/off toggle for when raw dictation is good enough.

- **Review SuperWhisper's feature set** — go through what SuperWhisper
  offers end-to-end and decide what's worth adopting beyond the two items
  above. Open research item, no defined scope yet.

## Research / Exploration

- **Single-speaker voice isolation** — reject background noise (music,
  other people talking, kids yelling) so only the primary user's voice
  reaches Whisper. Candidate approaches: voice activity detection (VAD)
  tuned against non-speech energy, speaker-embedding-based isolation
  (enroll the user's voice once, filter against it), or a local
  source-separation model (Demucs-style) ahead of Whisper. Meaningfully
  larger scope than the rest of the pipeline — needs its own model,
  enrollment UX, and a latency budget. Needs a dedicated research/design
  pass rather than a quick add-on.
