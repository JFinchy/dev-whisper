# Features

Everything currently built and working, grouped by area. This is the "what
does the app actually do right now" reference — see `PRODUCT_SPEC.md` for
the original vision/pitch, and `ROADMAP.md`/`BACKLOG.md` for what's not
built yet.

## Core dictation loop

- **Push-to-talk global hotkey**, configurable from Settings, persists
  across restarts. Uses Carbon's `RegisterEventHotKey` so it works without
  Accessibility permission — only pasting the result needs that.
- **Double-tap Fn to start/stop recording** — an opt-in alternative to the
  hotkey above, matching macOS's own "press Fn twice" dictation gesture.
  Off by default (needs macOS's Input Monitoring permission, a separate
  grant from Accessibility); runs alongside the hotkey rather than
  replacing it.
- **Local speech-to-text** via whisper.cpp with Metal acceleration.
  Tiny/Base/Small (English) models selectable and downloadable from
  Settings, with per-model download progress.
- **Per-mode model switching** without a reload penalty — `WhisperEngine`
  keeps a small LRU pool (up to 3) of warm-loaded contexts, so a mode rule
  pinned to a different model doesn't repay the multi-second Metal shader
  compile on every recording that hits it.
- **Developer vocabulary seeding** — an editable list of dev-jargon terms
  (`kubectl`, `camelCase`, framework names, etc.) fed into Whisper's
  `initial_prompt` to bias recognition toward developer speech.
- **Deliver as paste or copy-only** — the transcript is copied to the
  clipboard and a simulated Cmd+V pastes it at the active cursor by
  default; a "copy only" toggle skips the simulated keystroke entirely
  (and doesn't need Accessibility permission on that path).

## App-aware modes & LLM refinement

- **Frontmost-app detection** (`NSWorkspace`) picks a formatting Mode
  (Plain / Casual / CLI) based on whichever app was focused when
  recording started, with built-in defaults for common apps (terminals →
  CLI, Slack/Messages/Discord → Casual) and user-defined per-app rules.
- **Rule-based CLI formatting** — natural-language-to-shell-command
  patterns for the CLI mode (e.g. "git commit update readme" →
  `git commit -m "update readme"`).
- **Real LLM refinement** via a locally-running Ollama instance, with a
  per-mode "Refine with LLM" toggle and mode-aware prompts. Casual mode
  defaults this on, since rule-based formatting can't meaningfully do
  "sound casual."
- **In-app Ollama model catalog** (Gemma, Llama, Phi, Qwen, Mistral) with
  one-click pull and download progress, plus autodiscovery of any model
  you'd already pulled outside the app.
- **Last-observed refinement latency** surfaced per model in the LLM
  picker, so you can judge which local models are actually fast enough
  before committing to one (a "thinking" reasoning model can take
  minutes to answer a trivial prompt otherwise).
- **App picker with real icons** — native icon extraction (`NSWorkspace`
  + `NSBitmapImageRep`) for the running-apps picker and mode-rule list,
  plus a curated list of common apps so you don't have to have the target
  app open to add a rule for it.

## Voice commands

- **Syntax & Casing Commands** — say "snake case error response handler"
  and get `error_response_handler` pasted directly. Supports snake,
  SCREAMING_SNAKE/constant, camelCase, PascalCase, kebab-case, and Title
  Case. Pure Rust string transformation, no LLM involved, so it's instant
  and works even if Ollama isn't running.
- **Boilerplate Generation** — say "generate boilerplate for a React
  component called UserCard with name and avatar props" and the request
  goes to the local LLM, with a longer timeout than cleanup refinement and
  automatic markdown-fence stripping (small models often wrap code in
  fences despite being told not to). Falls back to normal dictation if
  Ollama is unreachable, so a down LLM never eats your speech.
- **Append clipboard** — end a dictation with "append clipboard" (or
  "paste clipboard"/"insert clipboard") and whatever's currently on the
  system clipboard gets appended to what you said, so a copied stack
  trace, URL, or code snippet doesn't have to be read aloud. Appended raw,
  after mode formatting/LLM refinement, so it's pasted verbatim.

## History

- **Local transcript history** (JSONL), with configurable retention
  (7/30/90/365 days) and automatic purge of anything older on launch.
  Only logs transcripts that actually reached you — a failed paste or
  copy doesn't show up as a history entry.
- **Work journal** (opt-in, off by default) — each history entry can get a
  one-line LLM-generated summary in git-commit-subject style ("Refactor
  audio module to drop devices safely during recording"), generated in
  the background after the paste already happened so it never adds
  latency. Skips dictations under 6 words (not worth summarizing) and
  falls back to just the raw transcript if Ollama is unreachable.
- **Per-entry copy/delete** in the History settings section.
- **Full-text search** — a search box filters the entire retained history
  (not just the most recent 200 entries the list view normally loads), by
  transcript text, journal summary, or app name.
- **Reprocessing** — re-run a past transcript's *text* through a different
  Mode (and optionally the LLM refinement pass) without re-recording.
  Doesn't re-transcribe from audio — the raw recording is a transient temp
  file, never retained — so this reformats the saved text, the same
  formatting/refinement a live dictation goes through. Preview the result,
  then copy it or replace the stored entry with it.

## Widget & Settings UI

- **Three widget display modes** — Minimal (icon-only recording button),
  Compact (the default status pill), and Detailed (a larger panel with a
  persistent, non-truncating status/error area), switchable from Settings.
- **Live input level meter** while recording — a small animated bar
  visualizer (Compact/Detailed) or a pulsing record dot (Minimal), driven
  by the mic's real-time RMS level, so it's visibly obvious audio is
  actually being picked up rather than trusting a static "recording" dot.
- **Errors that don't get lost** — Compact mode auto-grows and holds
  error text on screen for 6 seconds instead of truncating it (this used
  to silently swallow the Accessibility-permission error).
- **Draggable widget** that remembers its position across restarts.
- **Settings window** (520px wide) covering device, shortcut, models,
  app modes, LLM, vocabulary, history, an in-app log viewer, and general
  preferences, all in one place.
- **In-app Voice Commands reference** — every cross-cutting trigger
  phrase (casing, punctuation, numbered lists, boilerplate, press enter,
  append clipboard) documented right in Settings with a say/get example,
  so unexpected output ("why did that get pasted in ALL_CAPS") has an
  obvious place to check instead of needing to ask.
- **In-app log viewer** — an in-memory ring buffer capturing what used to
  be terminal-only debug output (device lifecycle, mode/model resolution,
  LLM results), so you can see what happened without launching from a
  terminal.

## System integration

- **Auto-launch at login** — off by default (a privacy-first app
  shouldn't silently add itself as a login item), toggle in Settings.
- **Tray icon reflects recording state** — a red dot overlays the mic
  glyph while recording. The glyph itself is a custom mic-themed icon
  (replacing the default Tauri icon) rendered as a macOS template image
  so it auto-adapts to light/dark menu bars.
- **Custom app icon** — same mic-themed design, full color, for
  Dock/Finder/the About window.
- **Deep-link/CLI automation hooks** — `open devwhisper://start-recording`
  / `stop-recording` / `toggle-recording` for Raycast, Hammerspoon,
  Alfred, or plain shell scripts. Start/stop are idempotent, so a script
  doesn't need to track recording state itself.
- **Output webhook** — a configurable URL gets a JSON `POST`
  (`timestamp_ms`, `text`, `summary`, `app_name`, `mode`) after every
  delivered dictation, covering Notion/Slack/n8n/Zapier/Make.com/
  webhook.site the way MacWhisper's integrations do, without a bespoke
  client per service. Fires in a background thread so a slow/unreachable
  endpoint never adds latency to the paste, and failures are logged, not
  surfaced. Off by default, plus a "Send test event" button in Settings.
  Complements the deep-link hooks above, which only cover the *input*
  side.

## Reliability

- **Mic device retry+fallback** — retries opening a specific input
  device a few times before falling back to the system default, fixing
  the "stale device handle right after selecting it" failure mode.
- **Warm model pre-load on startup**, so the first real recording doesn't
  pay for Metal shader compilation during the user's first
  "transcribing…".
- **Setup checklist (readiness)** — one set of checks (speech model
  downloaded, paste permission, hotkey registered, microphone, Ollama and
  the models your modes use) shown at the top of Settings with a fix
  button each, and as "Finish setup" on the widget when something blocks
  dictation. Hidden once everything is ready.
- **Survives a taken hotkey** — if another app owns the shortcut, the app
  still starts (it used to fail to launch) and the checklist says so.
- **Single instance** — a second launch hands off to the running copy
  instead of starting another one that fights over the hotkey and mic.
- **Verified model downloads** — pinned to a fixed whisper.cpp revision,
  checked against SHA-256, with stall timeouts and no leftover partial
  files.
- **Refine only when it's needed** (on by default, Settings → LLM) —
  a clean transcript skips the LLM and pastes right away; filler words, a
  stutter or an unpunctuated run-on still go through it. Casual mode and
  modes with custom instructions always refine. Refinement used to add
  3-10s per dictation while mostly returning the text unchanged.
- **Two-stage progress in the widget** — "Transcribing 64%" in the theme
  accent, then "✦ Refining 40%" in violet with a shimmer and the model
  name on hover. Transcription progress comes from Whisper plus the
  model's learned speed on your Mac; refinement progress is exact, from
  Ollama's streamed answer. Minimal mode shows the same as a colored ring.
- **Paste as-is** — click the violet Refining bar, or press the hotkey
  plus Shift (⌘D → ⇧⌘D; plus Option if the hotkey already has Shift), to
  skip LLM cleanup. While recording, the shortcut stops and pastes without
  refining.
- **Your vocabulary reaches the LLM** — refinement is told your terms
  (Firestore, kubectl, project names), so a misheard one comes back
  spelled your way.
- **Bad LLM answers stop early** — a reply that opens with chatter
  ("Sure! Here's…") or runs far longer than what you said is cut off as
  it starts, and the unrefined text pastes.
- **Every speech model a mode uses is warmed at launch**, not just the
  default, so the first dictation in that mode isn't slow.
- **Recording timer** next to the level meter.
- **Faster LLM cleanup** — Ollama keeps the model loaded for 30 minutes,
  and recording start pre-warms every model a mode uses, so the load
  overlaps with you talking instead of being added after.
- **Nothing blocks the UI** — commands that do I/O (Ollama, history,
  insights, device enumeration) run off the main thread, so a slow Ollama
  or a History reprocess no longer freezes the widget, tray and hotkey.
- **Latency you can see** — every dictation logs
  `perf: stt=… llm=… deliver=… total=…`, history records the wait, and
  Insights shows the typical and slowest-1-in-10 time to paste.
- **Crash and log hygiene** — panics on any thread are written to
  `app.log` with a backtrace; `app.log` rotates at 5MB.
- **Failed settings changes are visible** — a save the backend rejects
  shows a toast instead of silently not sticking.
- **Content Security Policy** on the webview, verified by running the
  production build under it in the e2e suite.

## Dictation accuracy (2026-10-19)

Found by running real speech through Whisper in the audio e2e suite:

- **Casing commands work with real Whisper output** — Whisper often writes
  "snake case error response handler" as `snake_case_error_response_handler`
  (and "camel case user ID" as `camelCase user ID`); both are recognized.
- **Lists from real transcripts** — Whisper writes "One, … Two, …" as
  "1. … 2. …" on one line; those become a line-broken list. Numbers and
  ordinals mid-sentence ("one question about the two failing tests") no
  longer turn into a list. List items are capitalized.
- **Backtrack keeps your sentence** — "Let's meet at two, actually three"
  gives "Let's meet at three." (it used to leave only "three."). Longer
  corrections replace just the sentence they interrupt.
- **Spoken "period"/"question mark" capitalize the next sentence**, and
  "comma" misheard as "Kama" still becomes a comma.
- **"…, press enter."** no longer leaves a dangling comma.
- **Terminal file tags include paths** — `src/lib.rs` becomes
  `@src/lib.rs` for terminal coding agents.
