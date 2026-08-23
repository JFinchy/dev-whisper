# Settings visual cleanup (Dictation page + shell layout)

Branch: `main`
Files changed: `src/App.css`, `src/SettingsView.tsx`

## What to verify

Three complaints about the Settings window drove this: (1) the Dictation
page felt cramped/cluttered with too little white space and too much
mismatched color, (2) scrolling far enough down made the black background
disappear, (3) a sticky/scrolling top bar visually clashed with content
under it. Fixes:

- The four Dictation-page sections (General, Voice Commands, Microphone,
  Push-to-talk key) are now bordered cards with real padding instead of
  bare text separated by thin dividers, and daisyui's default accent
  color (checkboxes, buttons, selects) now routes through the app's own
  theme accent instead of clashing with it.
- The settings shell is now a fixed-height layout where only the content
  pane scrolls internally (sidebar and background always fill the
  window), instead of letting the whole page grow taller than the
  viewport with nothing behind the overflow.
- The page title is now pinned (`position: sticky`) with a fully solid
  background, so it can't visually blend with content scrolling under it.

This is layout/CSS — automated tests don't cover it, so it needs eyes on
the actual window.

## Steps

1. From repo root: `bun run tauri dev`. Click the menu bar mic icon →
   Settings (or however you normally open it).
2. Land on the **Dictation** page (default). Confirm General, Voice
   Commands, Microphone, and Push-to-talk key each render as a distinct
   bordered card with visible padding — not raw text stacked with thin
   divider lines.
3. Confirm checkboxes, the push-to-talk button (click it to enter
   "listening" state), and selects use the app's warm accent color
   (matches the sidebar's active nav item), not an unrelated blue/violet.
4. Shrink the Settings window (drag the bottom edge) until the Dictation
   page's content is taller than the window. Scroll all the way to the
   bottom. Confirm the dark background fills the entire window at every
   scroll position — no patch of "missing" background/wrong color past a
   certain point.
5. While scrolled down, confirm the "Dictation" page title stays pinned
   at the top and has a fully solid background — no scrolled-past content
   showing/bleeding through it.
6. Repeat steps 4–5 on at least one other page (e.g. Insights or History)
   to confirm the scroll/sticky-title fix isn't Dictation-specific.
7. Switch to the **Signal Chain** layout (Appearance page → Layout) and
   confirm it's unaffected — this fix intentionally only touches the
   sidebar layout's shell.

## Pass criteria

Dictation page reads as clean, spaced-out cards in one consistent accent
color; the background never disappears while scrolling at any window
size; the page title stays solid and legible while scrolling; the
alternate Signal Chain layout still looks/scrolls the way it did before.
