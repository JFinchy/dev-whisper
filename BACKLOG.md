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

- **Isolated Voice mode** (phase 2 built, in testing, 2026-08-17) — a
  Settings toggle that filters a recording down to the primary user's
  voice before Whisper transcribes it. Runs post-capture (once, on the
  fully-recorded buffer, right before transcription) rather than
  live/streaming, matching the app's push-to-talk architecture and
  avoiding the latency floor a continuous-inference approach would need.
  New `isolate.rs`: an RMS energy gate with hysteresis (separate
  enter/exit thresholds so it doesn't chatter at the boundary) and
  ~150ms hangover padding, zero-fills everything outside the detected
  voiced ranges rather than trimming — keeps the buffer's timeline
  continuous for whisper.cpp and lets a fully-masked clip degrade into
  the existing "no speech detected" path. `stt.rs`'s
  `transcribe_with_model` was split into sample-loading +
  `transcribe_samples()` to give this a hook point between the two.
  - **Phase 1 (merged, energy-gate only)**: 7 unit tests
    (silence/tone/tone-silence-tone boundary cases, exact-zeroing +
    length-preservation for masking).
  - **Phase 2 (built 2026-08-17)**: voice enrollment + speaker-embedding
    cosine-similarity masking, auto-selected over the energy gate once a
    voice is enrolled. Added `sherpa-onnx = "1.13.5"` (official k2-fsa
    Rust crate, Apache-2.0, statically linked — confirmed it resolves
    and builds clean, no dylib bundling/signing risk unlike a raw `ort`
    dependency) plus a bundled `wespeaker_en_voxceleb_resnet34_LM.onnx`
    (25.3MB, Apache-2.0, downloaded from the k2-fsa/sherpa-onnx GitHub
    release and confirmed live) speaker-embedding model as a Tauri
    `resources` entry, CPU inference (no CoreML — cheap enough on CPU
    that chasing CoreML's flagged instability, per the Parakeet entry
    below, isn't worth it here). New `voice_isolation.rs`:
    `start_voice_enrollment`/`stop_voice_enrollment`/
    `get_voice_enrollment_status` commands, reusing the same
    `AudioHandle` dictation uses, guarded by a new
    `RecordingPurpose` mutex on `RecordingState` so a hotkey press
    mid-enrollment can't misfire `transcribe_and_paste` on the
    enrollment clip. Enrollment requires ≥3s of actual detected speech
    (via the phase-1 energy gate) before it'll compute an embedding;
    persists to `voice_profile.json` (own file, mirrors `history.rs`'s
    pattern), tagged with a `model_id` so a future embedding-model swap
    can force re-enrollment instead of silently comparing incompatible
    vectors. `isolate::apply`'s embedding path scores each voiced range
    (widening short ones with context first) via cosine similarity
    against the enrolled profile (starting threshold 0.5, needs
    empirical tuning during manual verification) and fails open — a
    scoring hiccup keeps the segment rather than silently dropping real
    speech. Settings UI now shows enrollment status + Enroll/Re-enroll/
    Stop controls, reusing `ModelsSection`'s `listen()` event pattern.
    9 new unit tests (cosine-similarity accept/reject cases,
    `widen_for_scoring` bounds) plus one gated real-model smoke test
    (loads the actual bundled `.onnx`, asserts a 256-dim embedding on a
    synthetic tone — passed). Full suite: 86 tests green, `tsc --noEmit`
    and `bun run build` both clean.
  - **Not yet done**: manual end-to-end verification — needs a live mic
    and a second real/played voice, not something automated tests alone
    can check. See `MANUAL_TESTS.md` for the checklist.
  - **Scope note**: this deliberately revisits the 2026-08-16 SuperWhisper
    research's call to leave speaker diarization/meeting-notes out of
    scope. Multi-speaker "meeting mode" (labeling Speaker 1/2/3 segments)
    is still explicitly not part of this work — single-speaker isolation
    only. Meeting mode would need its own diarization-model spike as a
    separate future effort.

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

- **Review SuperWhisper's feature set** (researched 2026-08-16) — full
  gap analysis against superwhisper.com's homepage + changelog. Headline
  finding: SuperWhisper is *hybrid local+cloud* (GPT-5/Claude/Gemini/Grok
  as optional refinement backends, Cohere/ElevenLabs as optional cloud
  STT) — it is not local-only, which validates rather than undercuts Dev
  Whisper's privacy-first positioning; cloud-model parity is explicitly
  not a goal here.
  - **Worth adopting, ranked:**
    1. Direct coding-agent integration (Claude Code/Cursor/etc. plugins,
       a coding-agent panel, hook support) — SuperWhisper is leaning into
       exactly Dev Whisper's developer-specific angle; we're better
       positioned to do it locally.
    2. Realtime streaming transcript display (partial results while still
       speaking, not just after release) — real UX upgrade to the
       shipped push-to-talk loop.
    3. ~~Deep-link/CLI hooks to start/stop recording~~ (shipped
       2026-08-16) — `open devwhisper://start-recording` /
       `stop-recording` / `toggle-recording`, idempotent start/stop, see
       ROADMAP.md.
    4. Selected-text/on-screen context feeding LLM refinement — extends
       the shipped app-aware refinement and boilerplate generation with
       real code context, not just app identity.
    5. History reprocessing + full-text search — history is currently
       append-only with no way to re-run a past recording through a
       different mode or search it.
  - **Explicitly skipped as out of scope**: cloud/BYOK AI models
    (contradicts local-only premise), cross-platform support (macOS-only
    by design), enterprise/SOC2/billing features, speaker diarization +
    meeting-notes mode (meeting-assistant territory, not core dictation),
    stats-sharing/growth gimmicks, bulk file/video transcription,
    100+-language translation, theme/cosmetic polish.
  - Filler-word removal, vocabulary CSV import, and mouse-button
    push-to-talk triggers are noted as small, low-priority QoL items if
    ever picked up.

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

- **Competitive landscape: dictation/Whisper tools beyond SuperWhisper**
  (researched 2026-08-17, via live browsing — WebSearch/WebFetch were down
  platform-wide, real infra outage, not fabricated data) — covered Wispr
  Flow, MacWhisper, VoiceInk, and Talon Voice as a different-category
  reference point.
  - **Landscape**: two real pricing/architecture clusters exist, not one.
    *Cloud/hybrid subscription*: Wispr Flow ($0 free tier capped at
    2,000 words/week, $12/user/mo Pro, Enterprise; SOC2/HIPAA/ISO 27001;
    cloud-processed) and SuperWhisper (hybrid, see prior entry above).
    *Local one-time-purchase*: MacWhisper (free + €64 lifetime Pro,
    local Whisper/Parakeet models, optional bring-your-own-key cloud AI
    layer) and VoiceInk (100% offline, open source GPLv3, $29-69
    lifetime tiered by device count, no subscription at all). Dev
    Whisper is local *and* fully free — a combination none of the four
    match.
  - **VoiceInk is our closest architectural peer**: whisper.cpp-based,
    app-aware "Modes" that auto-apply settings per app/URL (same concept
    as our `modes.rs`), personal dictionary, global push-to-talk
    shortcuts, launch-at-login — all things we also have. Meaningful
    gaps where *they're* ahead: on-screen/selected-text context
    awareness is already shipped (via a library called SelectedTextKit,
    a real macOS API for reading text selections) — validates our own
    unbuilt "Now" item #3 (selected-text context feeding LLM
    refinement) is both achievable and worth prioritizing, and points at
    a concrete implementation path. They've also already shipped
    Parakeet model support (relevant data point for our
    validate-first-spike item, though doesn't resolve our own
    CoreML-instability concern since their acceleration path is
    unconfirmed) and a built-in conversational "AI Assistant" mode,
    a real step toward PRODUCT_SPEC.md §5's "Rubber Ducking" vision.
  - **Wispr Flow has a dedicated developer product page**
    ("Flow for Developers"): recognizes dev terms/camelCase/snake_case,
    tags files in Cursor/Windsurf to bring code context into AI prompts,
    and has a **snippet library** — spoken cue expands to a saved block
    of text (PR checklist, calendar link, onboarding instructions). This
    is a new pattern, distinct from a vocabulary list (which is about
    recognition accuracy, not insertion) — not currently tracked
    anywhere in our roadmap.
  - **MacWhisper has CLI control plus output-side workflow automation**:
    "Control MacWhisper from the CLI. Hook it up in your agent or
    scripting workflows," and auto-uploads transcripts to Notion,
    Zapier, Obsidian, n8n, Make.com, or a custom webhook. Our deep-link
    hooks (shipped 2026-08-16) cover the *input* side (start/stop
    recording via script); nothing covers routing a finished
    transcript/journal-summary *out* to another tool — a real gap this
    surfaced, not previously tracked.
  - **Talon Voice** is a different category worth naming explicitly, not
    a feature-parity competitor: full hands-free computer control (voice
    + eye tracking + noise clicks), Python-scriptable, free/donation —
    the actual incumbent tool serious RSI-affected/hands-free-coding
    developers already use. Not dictation-with-formatting like the rest
    of this list; not worth chasing feature parity with, but worth
    knowing it's what that specific audience segment compares us
    against.
  - **New, previously untracked findings to fold into ROADMAP.md**:
    output-side workflow automation (Notion/webhook/Slack routing for
    transcripts), and a snippet library (spoken-cue text expansion,
    separate from the vocabulary editor).
