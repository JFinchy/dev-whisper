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

- **Redesign Modes UI to match SuperWhisper** — SuperWhisper's mode system
  (see screenshots shared 2026-08-15) is meaningfully more complete than
  our stage-1 "each app maps to one of 3 fixed built-in modes" model:
  - Modes are **named, user-created presets** ("Voice to text", "Default"),
    not fixed built-ins. Each preset has: Language, Voice Model, its own
    keyboard shortcut to start a recording directly in that mode, an
    "Activate for apps" assignment, and an "Advanced settings" section
    (presumably per-mode prompt/formatting overrides).
  - "Activate for apps" uses a proper categorized picker (Mail,
    Messaging, AI chat, Text editing, Coding, Terminal, Browsers, Social
    media, Design) with app icons, rather than our bare bundle-ID list.
  - A left sidebar nav (Home, Modes, Vocabulary, Configuration, Sound,
    Models library, History) — **Vocabulary** is its own section too:
    user-editable dictionary/jargon list, vs. our hardcoded
    `DEV_VOCAB_PROMPT` constant in `stt.rs`.
  - This effectively supersedes the current Settings UI for modes (the
    Rust-side groundwork — `app_detect.rs` frontmost detection,
    `modes.rs` resolution logic — is still the right foundation; this is
    mainly a data-model and UI expansion: named presets instead of a
    fixed enum, per-mode shortcuts, multi-app assignment per mode instead
    of one-mode-per-app).

- **Transcript history** — SuperWhisper's "History" section: store past
  transcripts locally, browsable/scrollable in-app (a new view, not
  server logs). Needs:
  - A local store (SQLite via `rusqlite`, or a simple append-only
    JSONL file) recording timestamp, transcript text, which app/mode it
    was captured for.
  - A scrollable history view — likely its own settings-window tab
    alongside Device/Shortcut/Models/App modes.
  - **User-configurable retention** (number of days to keep), with
    automatic purge of anything older, run on startup or on a timer.
  - Worth flagging: dictated text can contain sensitive content
    (passwords read aloud, private messages, etc.), so this needs a
    visible "Clear history" action and should probably default to a
    conservative retention window rather than "forever," even though
    storage is local-only and matches the app's privacy-first framing.

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
