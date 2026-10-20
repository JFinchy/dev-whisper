# Mic visibility, quick switching, and "no sound" warnings

Branch: `main` (2026-10-15)
Files changed: `src-tauri/src/audio.rs`, `src-tauri/src/recording.rs`,
`src/WidgetView.tsx`, `src/App.css`

## What to verify

The widget shows which mic it's using and lets you switch from the hover
menu. If the mic delivers no sound for ~2s while recording (muted,
unplugged, or a virtual device like "Microsoft Teams Audio" outside a
call), it warns you instead of silently recording nothing. Unit tests
cover the thresholds; only real devices show whether they're right.

## Steps

1. Hover the widget (Compact). A **Microphone** section lists System
   default plus each mic (on this Mac: iPhone Microphone, MacBook Pro
   Microphone, Microsoft Teams Audio). The one actually recording is
   highlighted. With AIRHUG 28 picked but off, it shows as a dimmed
   "AIRHUG 28 (not connected)" row.
2. Hover the record button: the tooltip ends with `Mic: <name>`.
3. Click **MacBook Pro Microphone** in the menu, then dictate. It records
   from that mic (`app.log`: `audio: device status -> Preferred ...`
   right after the click, not only after the next recording).
4. Click **Microsoft Teams Audio** (silent outside a call) and start
   recording while talking. Within ~2s the meter is replaced by amber "No
   sound from mic"; hover it for the full message. Stop: the widget says
   "Microsoft Teams Audio sent no audio…", nothing is pasted, and
   transcription is skipped.
5. Switch back to MacBook Pro Microphone, record in a quiet room without
   talking for 5s: no warning should appear (room noise isn't silence).
6. Mid-recording, unplug/turn off a USB or Bluetooth mic: the warning
   appears; plug it back in and talk: it clears.

## Pass criteria

Steps 1–6 as described; in particular step 5 must not false-alarm. If it
does, note your mic and room, since `DEAD_LEVEL` in `audio.rs` would need
lowering.
