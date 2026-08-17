# Signal-chain Settings, theme picker & widget flyout

Branch: `JFinchy/greenling`
Files changed: `src-tauri/src/theme.rs` (new), `src-tauri/src/config.rs`,
`src-tauri/src/lib.rs`, `src-tauri/src/recording.rs`, `src/theme.ts` (new),
`src/SettingsView.tsx`, `src/WidgetView.tsx`, `src/App.css`

## What to verify

Three related changes, all UI-facing:

1. Settings is restructured from one flat 520px scrolling column into a
   "signal chain" — five connected nodes (Input → Recognition → Mode →
   Refinement → Output) plus a detached "App" node, click a node to expand
   its settings inline. The window is now 880×720 (min 760×560) instead of
   520×680.
2. A 4-way color theme picker (Terminal/Signal/Quiet/Palette) lives in the
   App node, defaults to Terminal, persists across restarts, and updates
   the widget's accent color live via a `theme-changed` event without
   restarting either window.
3. The floating widget's compact mode grows a hover flyout below the pill
   with two real, wired controls: Mic Mode (Standard/Voice Isolation —
   same backend field as the old Settings toggle) and a one-shot "next
   dictation" mode override (Plain/Casual/CLI) that takes priority over
   the auto-detected app-mode for exactly one dictation, then clears
   itself.

Automated coverage: `cargo test --lib` (80 tests, including new
`theme::tests` and the updated `config::tests` round-trip) and `bun run
build` (tsc + vite) both pass, but nothing automated drives the actual
window resize, the click-to-expand nodes, the live theme swap, or hovering
the real floating widget — that's what this pass is for.

## Steps

### Signal chain layout
1. From repo root: `bun run tauri dev`. Look for the mic icon in the menu
   bar (not a window).
2. Tray icon → **Open Settings…**. Confirm the window opens noticeably
   wider than before (five node cards should fit on one row without
   wrapping) and shows five connected nodes — Input, Recognition, Mode,
   Refinement, Output — plus a separate "App" node off to the side.
3. Click each node in turn. Confirm it expands a drawer below the chain
   with the right settings inside, and that everything that used to work
   still works:
   - **Input** → Microphone device picker, push-to-talk shortcut capture.
   - **Recognition** → Whisper model list (download/activate), Voice
     Isolation toggle, Vocabulary chips.
   - **Mode** → app rules list (add/edit/remove, per-rule model + LLM
     refinement toggle).
   - **Refinement** → Ollama model catalog (download/activate, latency).
   - **Output** → Copy-only toggle, Output webhook (URL + test send),
     History (list, retention, work journal toggle, per-entry copy/delete).
   - **App** → Launch at login, Widget style picker, theme picker (next
     section), Logs (expand/refresh/clear).
4. Click the currently-open node again and confirm the drawer collapses.

### Theme picker
5. In the **App** node, click through all four themes (Terminal → Signal
   → Quiet → Palette). Confirm the chain nodes' border/icon color and the
   drawer's accent color change immediately, and the selected row shows a
   checkmark.
6. Quit and relaunch the app (or just reopen Settings) — confirm the last
   theme picked is still selected (persisted, not reset to Terminal).
7. With Settings open, glance at the floating widget while switching
   themes in step 5 — its recording-dot glow and gear-hover color should
   update live (the widget stays dark-glass; only the accent shifts).

### Widget hover flyout
8. Make sure the widget is in **Compact** mode (App node → Widget style).
9. Hover the mouse over the floating widget pill (don't click). Confirm it
   grows downward to reveal "Mic mode" (Standard/Voice Isolation) and
   "Next dictation" (Plain/Casual/CLI) rows, and shrinks back to normal
   when the mouse leaves.
10. Click "Voice Isolation" in the flyout, then open Settings → Recognition
    and confirm the Voice Isolation toggle is now on (same underlying
    setting, driven from two places).
11. Hover the widget again, click "CLI" under Next dictation. Trigger a
    dictation in an app that would normally resolve to Plain or Casual
    (e.g. dictate into TextEdit or Notes) and say something like "git
    commit update readme". Confirm it pastes as
    `git commit -m "update readme"` (CLI formatting), even though TextEdit
    isn't a CLI app.
12. Hover the widget again afterward — confirm "Next dictation" is back to
    no selection (the override is one-shot and clears itself after use).
13. Hover the widget, click "CLI" again to select it, then click "CLI" a
    second time — confirm it deselects (click-to-clear before ever
    recording), and a subsequent dictation uses the normal auto-detected
    mode.

## Pass criteria

All of steps 3, 5-7, and 9-13 behave as described. Nothing that worked in
the old flat Settings layout should be missing or broken in the new one —
this is a restructure, not a feature cut.
