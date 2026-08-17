# Features

Everything currently built and working, grouped by area. This is the "what
does the app actually do right now" reference — see `PRODUCT_SPEC.md` for
the original vision/pitch, and `ROADMAP.md`/`BACKLOG.md` for what's not
built yet.

## Core dictation loop

- **Push-to-talk global hotkey**, configurable from Settings, persists
  across restarts. Uses Carbon's `RegisterEventHotKey` so it works without
  Accessibility permission — only pasting the result needs that.
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

## Widget & Settings UI

- **Three widget display modes** — Minimal (icon-only recording button),
  Compact (the default status pill), and Detailed (a larger panel with a
  persistent, non-truncating status/error area), switchable from Settings.
- **Errors that don't get lost** — Compact mode auto-grows and holds
  error text on screen for 6 seconds instead of truncating it (this used
  to silently swallow the Accessibility-permission error).
- **Draggable widget** that remembers its position across restarts.
- **Settings window** (520px wide) covering device, shortcut, models,
  app modes, LLM, vocabulary, history, an in-app log viewer, and general
  preferences, all in one place.
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
