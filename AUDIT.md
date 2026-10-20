# Production readiness audit — 2026-10-19

A staff-level pass over the whole app: backend, frontend, dictation
quality, testing, release tooling and docs. Most findings are fixed in the
same pass. What's left is at the bottom, in priority order.

## Summary

The core was sound: a clean capture state machine, careful device
handling, good logging, thoughtful comments. What made it feel DIY:

- **Correctness bugs in everyday dictation** that only real audio
  exposes. Spoken casing commands never fired, and spoken lists never
  broke into lines. Backtrack deleted the start of your sentence, and
  numbers in normal sentences turned into lists.
- **No automated end-to-end testing.** Unit tests passed while the above
  was broken, and only the release workflow ran in CI.
- **Failure modes that were silent or fatal.** Settings saves that failed
  silently, a taken hotkey stopping the app from starting, unverified model
  downloads, and main-thread I/O able to freeze the whole app.
- **No first-run guidance.** A fresh install couldn't transcribe, and
  nothing said why until a dictation failed.

## Findings and fixes

### Dictation quality (found by the new real-audio suite)

| Finding | Fix |
|---|---|
| Whisper writes "snake case error response handler" as `snake_case_error_response_handler`, so casing commands never matched | `syntax.rs` re-spells `_`/`-`/camelCase forms before matching |
| Whisper writes "One, … Two, …" as "1. … 2. …" on one line; list detection only knew number words | Digit markers at item boundaries (`punctuation.rs`) |
| "I have one question about the two failing tests." became a numbered list | In punctuated text, markers must sit at a boundary |
| "Let's meet at two, actually three" pasted "three." | Short corrections swap words; long ones replace only their sentence (`backtrack.rs`) |
| "Hello comma world period this is" → "…world. this is" | Capitalize after a spoken sentence end |
| "comma" heard as "Kama" | Alias |
| "…, press enter." left a dangling comma | Stripped |
| `src/lib.rs` didn't get an `@` tag for terminal agents | Relative paths tagged |

Mean word error rate on the audio suite went from 0.633 to 0.033.

### Reliability and security

| Finding | Severity | Fix |
|---|---|---|
| Every Tauri command was sync, which runs it on the **main thread**: a History "reprocess with LLM" froze the widget, tray and hotkey for up to 30s; a hung Ollama froze Settings | High | I/O commands are `#[tauri::command(async)]`; Ollama probes time out at 3s |
| A hotkey owned by another app made `setup()` return an error: **the app wouldn't start** | High | Registration failure is recorded and shown in the setup checklist; `set_shortcut` copes with an unregistered old hotkey |
| `"csp": null`: no Content Security Policy | High | Strict CSP; the e2e suite runs the production build under it |
| Model downloads from `resolve/main`, unverified, no timeouts, `.part` left behind on failure | High | Pinned revision, SHA-256 checked, connect/read timeouts, cleanup |
| No single-instance guard: two copies could fight over the hotkey, mic and config | Medium | `tauri-plugin-single-instance` |
| Panics on background threads vanished (stderr isn't visible in release) | Medium | Panic hook writes them to `app.log` with a backtrace |
| `app.log` grew without bound (412KB and climbing) | Low | Rotates at 5MB |
| Dragging the widget did a config read+write per frame | Low | Debounced |
| Ollama unloaded the model after 5 minutes idle, so the next dictation paid a cold load | Medium (latency) | `keep_alive: 30m`, plus a prewarm when recording starts |

### Product and UX

| Finding | Fix |
|---|---|
| Fresh install: no model, no guidance | Setup checklist in Settings with a fix button per problem; "Finish setup" on the widget |
| 57 settings failures only `console.error`'d, so a failed save looked like it stuck | Toasts with a plain-language message |
| Widget actions (start recording, switch mic) failed silently | The widget shows the error |
| Latency was invisible | `perf:` log line per dictation, latency stored in history, "Time to paste" in Insights |
| `index.html` title "Tauri + React + Typescript"; Vite/Tauri template icons | Fixed, removed |
| Ollama link used `target="_blank"`, which does nothing in a Tauri webview | Opens in the browser |

### Engineering

| Finding | Fix |
|---|---|
| `process_transcript` was ~300 lines mixing formatting with side effects | Formatting extracted to pure `pipeline.rs`, with the LLM behind a trait |
| `SettingsView.tsx` was one 2,900-line file | Split into `src/settings/` (one file per page, shared icons and layouts) |
| Widget window loaded the whole Settings bundle | Settings lazy-loaded |
| 188 rustfmt diffs, 8 clippy warnings | Clean; CI enforces `-D warnings` |
| Version bump = edit 3 files by hand; a miss silently breaks updates | `scripts/bump-version.sh`, CI runs `check-versions.sh` |
| README described a pre-release, pre-signing world | Rewritten; dev signing scripted (`setup-dev-signing.sh`) |
| Package metadata "A Tauri App" / authors "you" | Fixed (binary name left alone: signing and permission grants depend on it) |

## Testing added

See [`TESTING.md`](TESTING.md).

- **Golden dictation suite**: 35 spoken-input → delivered-output cases
  through the real pipeline with a scripted LLM.
- **Real-audio e2e**: macOS `say` → Whisper → pipeline, with a WER and
  latency report usable for model comparisons.
- **UI e2e**: 17 Playwright tests in WebKit on an in-memory backend,
  covering the dictation lifecycle, mic switching and fallback, the silent
  mic, setup states, error surfacing, and every Settings page. 4 run
  again under the production CSP.
- **CI**: Rust (fmt, clippy, tests), audio e2e, UI e2e, version check.

Totals: 253 Rust tests (plus the audio suite), 21 Playwright runs, all
green.

## Remaining, in priority order

1. **Verify the hardening in the built app**: CSP inside the real webview
   especially (`manual-testing-inbox/production-hardening.md`).
2. **Notarization.** Needs an Apple Developer account ($99/yr). Without it
   every install needs right-click → Open, and an in-place update may
   re-trigger Gatekeeper. The biggest remaining step from "my app" to
   "anyone's app".
3. **Deep links can start a recording from any web page** that navigates
   to `devwhisper://start-recording` (browsers show a confirmation
   prompt, and the widget and tray show recording). Consider an "Allow
   automation links" setting, on by default only for people who use
   Raycast/Hammerspoon hooks.
4. **Microphone permission isn't in the checklist.** It would need
   `AVCaptureDevice.authorizationStatus` (objc2 AVFoundation bindings). For
   now, the "no sound" detection catches it after the fact.
5. **Streaming partial transcripts** (ROADMAP "Now" #4): the most visible
   remaining gap against SuperWhisper/Wispr.
6. **Main bundle is 424KB (125KB gzipped)** even with Settings split off;
   worth a bundle analysis if widget launch time ever matters.
7. **`tauri-plugin-single-instance` is on 2.4.x** (2.5.x needs a newer
   Tauri); update alongside the next Tauri bump.
