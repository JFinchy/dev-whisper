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
10. Back on a mode's LLM dropdown: confirm already-pulled Ollama models
    show a "✓" prefix and not-yet-pulled catalog models show a "⬇" prefix
    with their size (e.g. "⬇ Mistral (7B) — larger, more capable —
    4.1GB, not installed"). Click the ⓘ next to the dropdown and confirm
    the install-cost explainer popover opens/closes correctly (click the
    button again, or click away, to close it).
11. Pick a mode's LLM refinement to a model that isn't downloaded yet —
    confirm a "isn't installed yet (X GB)" row with a Download button
    appears under the dropdown. Click Download and confirm live progress
    (percent or status text) shows, the row disappears once it finishes,
    and the dropdown's "⬇" flips to "✓" without needing a manual refresh.
12. With a mode's LLM refinement set to anything other than "No LLM
    refinement", confirm a small textarea for extra instructions appears
    (placeholder shows an example instruction + before/after text) and
    disappears when refinement is set back to "off". Type something like
    "always sign off with my name" into CLI or Messaging's box, dictate
    from one of its assigned apps, and confirm Settings → Logs / the
    actual pasted output reflects that extra instruction being followed
    by the LLM (not just the mode's normal refinement behavior).

## Pass criteria

The real Orca migration looks correct (step 2), modes are fully
add/rename/delete/reassign-able and everything persists (steps 3-6),
per-mode LLM model selection actually changes which model gets called
(step 7), the widget flyout reflects the live mode list without breaking
its layout (step 8), History reprocessing is unaffected (step 9), the
LLM dropdown's install-status indicators/info popover/inline download
flow all work end to end (steps 10-11), and a mode's custom LLM
instructions actually change the model's output (step 12).
