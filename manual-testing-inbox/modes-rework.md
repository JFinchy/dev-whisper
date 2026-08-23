# Modes rework: named, multi-app modes with per-mode LLM selection

Branch: `main`
Files changed: `src-tauri/src/modes.rs`, `src-tauri/src/config.rs`,
`src-tauri/src/llm.rs`, `src-tauri/src/recording.rs`,
`src-tauri/src/insights.rs`, `src-tauri/src/lib.rs`,
`src/SettingsView.tsx`, `src/WidgetView.tsx`, `src/App.css`

## What to verify

"App Modes" used to be one rule per app (one app -> one of Plain/Casual/CLI,
with a global on/off LLM toggle). It's now a flat list of named, fully
editable **modes** — each with its own list of assigned apps, its own
Whisper model override, and its own LLM refinement choice (off / the
global default model / a specific pinned model). Ships with 4 modes by
default: Default (fallback), Voice to Text, Messaging (Messages/Slack/
Discord pre-assigned), and CLI (Terminal/iTerm/kitty/WezTerm/Warp
pre-assigned).

This changes `config.json`'s schema. **Important**: this machine's real
config had one existing rule (Orca -> CLI, `base.en`, LLM on) — it's
already been migrated (config.json's `modes.CLI.apps` now includes Orca,
`stt_model: "base.en"`, `llm_refinement: "global"`, and `mode_rules` is
now empty). Confirm that migration looks right before doing anything else
below, since it only runs once.

## Steps

1. `bun run tauri dev`, open Settings → Dictation → Modes.
2. Confirm 4 mode cards render: Default, Voice to Text, Messaging, CLI —
   and that CLI shows Terminal/iTerm/kitty/WezTerm/Warp **and Orca** as
   assigned apps, with its Whisper-model dropdown on `base.en` and its LLM
   dropdown on "Refine with global LLM".
3. Rename a mode (edit the name field) — confirm it persists after
   reloading Settings.
4. Add a new mode via "+ Add mode", assign an app to it via "+ Add app",
   change its Behavior/Whisper-model/LLM-model dropdowns, and confirm all
   of it persists across an app restart.
5. Delete a non-default mode — confirm its apps silently fall through to
   Default on the next dictation (check Settings → Logs after dictating
   from one of those apps).
6. Confirm Default has no delete button (it's the fallback and shouldn't
   be removable), but every other mode does.
7. Dictate from an app assigned to a mode with a specific LLM model
   pinned — check Settings → Logs / Ollama activity to confirm that
   specific model actually got used, not the global default.
8. Hover the widget (Compact mode). Confirm "Next dictation" now shows
   one full-width row per mode (not the old cramped single row of 3
   truncated pills) and the flyout grows tall enough to fit all of them
   without clipping. Clicking one still works as a one-off override for
   exactly the next recording. Add a mode with a long name and confirm it
   still reads fine (ellipsis, not a broken layout) and the flyout resizes
   to fit the new row count.
9. Open Settings → History, reprocess an old transcript through a
   different raw behavior (Plain/Casual/CLI) — confirm this still works
   unchanged (it intentionally still operates on the 3 raw behaviors, not
   named modes).

## Pass criteria

The real Orca migration looks correct (step 2), modes are fully
add/rename/delete/reassign-able and everything persists (steps 3-6),
per-mode LLM model selection actually changes which model gets called
(step 7), the widget flyout reflects the live mode list without breaking
its layout (step 8), and History reprocessing is unaffected (step 9).
