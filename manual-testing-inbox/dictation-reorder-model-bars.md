# Dictation page reorder + Whisper model speed/accuracy bars

Branch: `main`
Files changed: `src/SettingsView.tsx`

## What to verify

Two small, independent UI changes:

1. On the Dictation page, Microphone and Push-to-talk key now come before
   Voice Commands (was: General, Voice Commands, Microphone,
   Push-to-talk — now: General, Microphone, Push-to-talk key, Voice
   Commands).
2. The Whisper model picker (Settings → Voice) now shows a Speed and
   Accuracy bar under each model, alongside the existing size/download
   controls — same tradeoff the label text already states in words
   ("fastest, least accurate" etc.), just also visual.

Pure layout/JSX — not covered by automated tests.

## Steps

1. `bun run tauri dev`, open Settings.
2. Land on **Dictation** (default page). Confirm card order top-to-bottom:
   General, Microphone, Push-to-talk key, Voice Commands.
3. Go to **Voice**. Confirm each model row (Tiny/Base/Small) shows a
   "Speed" bar and an "Accuracy" bar under the size, with Tiny having the
   longest Speed bar / shortest Accuracy bar and Small the reverse.
4. Download/activate a model as usual — confirm the bars don't interfere
   with the existing Download/Use button or the in-progress `%` state.

## Pass criteria

Dictation section order matches step 2; Voice model bars render
correctly at every model download/active state and don't break the
existing controls.
