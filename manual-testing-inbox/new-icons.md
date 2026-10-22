# New app icon and menu bar icon

Branch: `main` (committed directly, 2026-10-22)
Files changed: `src-tauri/icons/*`, `src-tauri/icons/source/tray-icon.py`,
`src-tauri/src/lib.rs` (tray dot placement and grey glyph), `site/icon.png`

## What to verify

Icons only look right (or wrong) on a real Dock and menu bar; the tests
only check that the tray image decodes and the dot lands in the right
place.

## Steps

1. Install 0.3.3. The Dock, Finder and Cmd-Tab show the new prompt/wave/
   cursor icon (you may need `killall Dock` if macOS caches the old one).
2. Menu bar: the `>~|` glyph, same height as the icons beside it, black on
   a light menu bar and white on a dark one (switch System Settings →
   Appearance to check both).
3. Hold the hotkey: the glyph turns grey with a red dot top-right, visible
   on both light and dark menu bars. Release: back to normal.
4. With always-on listening on, the green dot shows while it's awake.

## Pass criteria

All three states are legible in both appearances, and nothing is clipped
or blurry at normal and Retina scale.
