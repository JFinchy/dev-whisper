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
  workflow, not signed release builds. Actually bit a real session
  (2026-08-16): after ~8 rebuilds in one night, paste silently stopped
  working — the error was real (`transcribe_and_paste` correctly emitted
  it) but the widget was too small and truncated to show it, so it looked
  like a silent failure instead of an obvious permission prompt. Fixed the
  *symptom* — the widget now auto-grows and holds error text on screen for
  6s instead of truncating it (see `widget.rs`, WidgetView.tsx) — but the
  root cause (re-signing churn) is still open. Worth a stable local
  codesigning identity now that it's demonstrably annoying enough.
- **"Thinking" LLM models are too slow for refinement by default** — the
  default Ollama model picked up on this machine (`qwen3.5:4b`) is a
  reasoning/"thinking" model that burns many seconds on internal
  chain-of-thought before answering even a trivial prompt (a plain
  `ollama run` "say hi" took minutes). Fixed by passing `"think": false`
  in the `/api/generate` request (`llm.rs`), which Ollama respects for
  models that support toggling it — with that, refinement is ~1-2s. If
  a user's chosen Ollama model doesn't support the `think` flag at all,
  this has no effect and slow models will still be slow. Last-observed
  latency per model is now surfaced in the Settings LLM picker (shipped
  2026-08-15) so users can judge which local models are fast enough.
- ~~**Flaky test: `cargo test --lib` concurrent whisper-context
  loading**~~ (fixed 2026-08-15) — `stt::tests::transcribes_without_panicking`
  and `stt::tests::transcribe_with_model_override_switches_contexts` both
  load a real ggml model onto Metal; cargo test runs tests in parallel by
  default, and running both at once reproduced a real (not just
  app-contention-related) `"Failed to create a new whisper context"`
  failure. Fixed by serializing the two tests with a shared
  `WHISPER_CONTEXT_LOAD_LOCK` mutex in `stt.rs`'s test module — 5/5 clean
  runs after the fix, 1 failure in 2 runs before it.

## Feature Requests

- **App-aware modes + LLM refinement** (shipped 2026-08-15) —
  frontmost-app detection (`app_detect.rs`, `NSWorkspace`), a mode
  framework with built-in defaults for common apps (`modes.rs`), a
  Settings UI to add/edit per-app rules including a live running-apps
  picker (no need to switch to the target app first), rule-based CLI
  formatting (a few illustrative natural-language -> shell-command
  patterns), and real LLM refinement via a locally-running Ollama
  instance (`llm.rs`) with a per-mode "Refine with LLM" toggle and a
  "Refining…" UI state. Verified end-to-end against a live Ollama
  instance, not just built blind — see the Known Issues note below on
  "thinking" models for a real gotcha this surfaced.
  - **Not yet wired**: `AppModeRule.stt_model` (per-mode Whisper model
    override) is in the data model and Settings UI, but doesn't
    actually switch `WhisperEngine`'s active model yet. Naively calling
    `set_model()` on every recording that hits a different-model rule
    would reload the whisper context and repay the multi-second Metal
    shader compile each time (see the "pre-warm" work from earlier in
    this project) — needs a small pool of warm contexts (e.g. an LRU of
    2-3 loaded models) rather than a blind wire-up.
  - Casual mode's rule-based path is still a no-op (regex can't
    meaningfully do "sound casual") — now that LLM refinement exists,
    Casual mode should default `use_llm_refinement` to true instead of
    leaving it manual.

- **Redesign Modes UI to match SuperWhisper** — SuperWhisper's mode system
  (see screenshots shared 2026-08-15) is still meaningfully more complete
  than what we have, even after the 2026-08-15 additions above:
  - Modes are **named, user-created presets** ("Voice to text", "Default"),
    not fixed built-ins — ours is still "each app maps to one Mode enum
    value" (Plain/Casual/Cli), not arbitrary named presets with their own
    Language/Voice Model/keyboard shortcut/"Advanced settings".
  - "Activate for apps" uses a proper categorized picker (Mail,
    Messaging, AI chat, Text editing, Coding, Terminal, Browsers, Social
    media, Design) with app icons — we now have a running-apps picker
    (shipped 2026-08-15) but it's a flat list, not categorized/iconed.
  - A left sidebar nav (Home, Modes, Vocabulary, Configuration, Sound,
    Models library, History) — we now have Vocabulary and History as
    their own Settings sections (shipped 2026-08-15), just not as a
    proper sidebar-navigated app, everything's stacked in one scrolling
    Settings window.
  - Still the right call to defer the full redesign: the underlying
    pieces (`app_detect.rs`, `modes.rs`, `llm.rs`, `history.rs`) are
    solid groundwork; this is now purely a UI/IA redesign, not new
    backend work.

- **Transcript history** (shipped 2026-08-15) — local JSONL store
  (`history.rs`), a scrollable History section in Settings, and
  user-configurable retention (7/30/90/365 days) with auto-purge on
  startup. Only logs transcripts that actually got pasted (a failed
  paste doesn't show up as a history entry). Defaults to 30 days, not
  "forever," since dictated text can contain sensitive content.

- **Vocabulary editor** (shipped 2026-08-15) — replaced the hardcoded
  `DEV_VOCAB_PROMPT` constant with a user-editable term list, persisted
  in config and fed into Whisper's `initial_prompt` at transcribe time.

- **Review SuperWhisper's feature set** — go through what SuperWhisper
  offers end-to-end and decide what's worth adopting beyond the items
  above. Open research item, no defined scope yet.

- **Parakeet as an alternative STT model** (researched 2026-08-15) —
  NVIDIA's Parakeet ASR as a second model option alongside Whisper.
  Findings from a dedicated research pass:
  - NVIDIA only ships Parakeet in NeMo/PyTorch format — no official ONNX
    export. But the community has already done the hard part:
    [`istupakov/parakeet-tdt-0.6b-v3-onnx`](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx)
    on Hugging Face has working ONNX exports (incl. int8-quantized), and
    [`parakeet-rs`](https://github.com/altunenes/parakeet-rs) (crates.io,
    383 GitHub stars, 63k+ downloads, actively maintained) is a real Rust
    crate wrapping `ort` (ONNX Runtime bindings) that already implements
    Parakeet's feature extraction and RNNT/TDT decoding — the parts that
    would otherwise be the expensive part of a second backend.
  - Apple Silicon acceleration is the open question: the `parakeet-rs`
    maintainer's own README flags CoreML as *unstable* for this model
    graph and recommends the WebGPU execution provider (Metal-backed,
    but newer/less proven than whisper.cpp's direct Metal path) or CPU.
    One anecdotal claim that CPU alone beats whisper.cpp's Metal path on
    an M3 — plausible given Parakeet's architecture, but needs our own
    benchmark before it's a decision input, not a data point from one
    person's machine.
  - Effort verdict: this is a real second inference backend (different
    feature extraction, tokenizer, decoding — not just "download another
    GGUF"), plus a second native runtime dependency (ONNX Runtime dylibs
    need bundling/signing/notarizing in the `.app`, unlike whisper.cpp's
    static lib) and multi-file model management (encoder + decoder_joint
    + vocab, hundreds of MB–2GB). But it's *not* build-from-scratch —
    `parakeet-rs` carries the hard parts, meaningfully lowering the
    bar versus rolling our own NeMo/ONNX integration.
  - Risks: single-maintainer crate (supply-chain/longevity), CoreML
    instability means the "accelerated" story needs reframing to
    WebGPU/CPU, and third-party ONNX conversions have to track NVIDIA's
    upstream churn (a v2→v3 model revision already happened).
  - **Recommendation: not implemented, no code written.** Tractable as a
    validate-first spike against `parakeet-rs` directly (confirm
    real-world speed/accuracy on our hardware) before committing to
    maintaining it as a shipped option — premature to build the full
    Settings/download UI plumbing before that spike says it's worth it.

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
