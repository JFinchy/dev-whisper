# Settings layout toggle, About page & vocabulary books

Branch: `JFinchy/greenling`
Files changed: `src-tauri/src/theme.rs`, `src-tauri/src/config.rs`,
`src-tauri/src/lib.rs`, `src/App.css`, `src/SettingsView.tsx`,
`src/vocabularyBooks.ts` (new)

## What to verify

Three related additions, all UI-facing:

1. **Layout toggle.** Appearance now has a "Layout" picker below the theme
   picker, with two options: **Signal** (the left sidebar, current
   default) and **Signal Chain** (a horizontal pipeline of nodes —
   Input/Recognition/Mode/Refinement/Output/App — where clicking a node
   expands a drawer beneath it; this is the earlier design checkpoint,
   now wired up as a real selectable layout instead of just a git
   reference). Every section that works in Signal also works in Signal
   Chain — nothing is Signal-only.
2. **About page.** A new footer nav item (next to Appearance/Advanced in
   Signal; folded into the "App" node's drawer in Signal Chain) with
   written explanations of how dictation, Voice Isolation, Vocabulary,
   Snippets, Modes, LLM refinement, History, and Integrations each work.
3. **Vocabulary books.** The Vocabulary page now leads with four
   adoptable term bundles — Frontend, Backend, Full-Stack, Product — each
   120-180 curated words. Clicking one merges its words into your
   existing vocabulary list (skipping duplicates); a fully-adopted book
   shows as disabled/checked.

Automated coverage: `cargo test --lib` (138 tests, including the new
`theme::tests` layout cases) and `bun run build` (tsc + vite) both pass,
but nothing automated drives the actual layout switch, the chain drawer
navigation, or clicking a vocabulary book — that's what this pass is for.

## Steps

### Layout toggle
1. `bun run tauri dev`, tray icon → Open Settings…
2. Go to **Appearance**. Confirm a "Layout" section appears below the
   theme picker, with **Signal** selected by default.
3. Click **Signal Chain**. Confirm the whole window re-renders as a
   horizontal pipeline (Input → Recognition → Mode → Refinement → Output,
   plus a detached "App" node) instead of the sidebar.
4. Click through every node — Input, Recognition, Mode, Refinement,
   Output, App — and confirm each expands a drawer with the same content
   the sidebar version had (Recognition should include Whisper model,
   Voice Isolation, Vocabulary *and* Snippets; App should include
   General, Appearance incl. the Layout picker, About, and Advanced/logs).
5. From inside the Signal Chain's App node, switch Layout back to
   **Signal**. Confirm it returns to the sidebar.
6. Quit and relaunch (or just reopen Settings) — confirm the last-picked
   layout persisted (not reset to Signal).

### About page
7. In Signal layout, click **About** in the sidebar footer. Confirm it
   shows written explanations for dictation, Voice Isolation, Vocabulary,
   Snippets, Modes, LLM refinement, History, and Integrations — not
   placeholder text.
8. Switch to Signal Chain, open the **App** node, and confirm the same
   About content appears there too, between Appearance and Advanced/logs.

### Vocabulary books
9. Go to **Vocabulary**. Confirm four book cards appear above the
   existing term-chip list: Frontend, Backend, Full-Stack, Product, each
   showing a word count and short description.
10. Click **Frontend**. Confirm its words appear in the term list below
    (as normal removable chips), and the Frontend card now shows an
    "Adopted" state and is no longer clickable.
11. Remove one of the just-adopted words from the chip list, then look at
    the Frontend card again — confirm it goes back to being clickable
    (since it's no longer fully adopted).
12. Manually add a term that also happens to be in the Backend book, then
    click **Backend** — confirm no duplicate chip is created for that term.

## Pass criteria

All of steps 3-6, 7-8, and 9-12 behave as described. Nothing that worked
before this change (any existing sidebar section, theme picking, the
widget hover flyout) should be missing or broken in either layout.
