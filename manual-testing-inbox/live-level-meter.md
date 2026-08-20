# Live input level meter

Branch: `main`
Files changed: `src-tauri/src/audio.rs`, `src-tauri/src/recording.rs`,
`src/WidgetView.tsx`

## What to verify

While recording, the widget now shows a live animation driven by the
mic's actual input level — a small bar visualizer in Compact/Detailed
mode, a pulsing record dot in Minimal mode — instead of a static red dot,
so it's obvious audio is actually being picked up. Automated coverage
(`cargo test --lib audio::`) checks the RMS-to-0.0-1.0 scaling function
against synthetic tones, but the 8x gain tuning and the actual on-screen
animation can only be judged against a real mic and real speech.

## Steps

1. From repo root: `bun run tauri dev`. Look for the mic icon in the menu
   bar (not a window).
2. Open Settings → General and set the widget to **Compact** (default).
   Start a recording and talk normally. Confirm the status text is
   replaced by a small bar meter that visibly reacts as you speak —
   louder words should produce taller bars, silence should settle the
   bars low (not fully flat/invisible — there's a 15% minimum bar height
   so it doesn't look broken/frozen).
3. Try clearly quiet vs. loud speech. Confirm the meter's dynamic range
   feels reasonable — quiet speech shouldn't stay pinned at the minimum
   the whole time, and normal speaking volume shouldn't immediately clip
   every bar to 100%. If it does, the 8x gain in `audio::rms_level` needs
   retuning (see the BACKLOG.md entry for where).
4. Switch the widget to **Detailed** mode. Repeat step 2 — confirm the
   meter replaces the status label at the top while recording, and the
   persistent message area below is unaffected.
5. Switch to **Minimal** mode. Start recording and talk. Confirm the
   record dot pulses/grows visibly with your voice rather than staying a
   fixed size.
6. Stop recording (in any mode). Confirm the meter/dot immediately
   settles back to resting state — no stale "still showing loud" reading
   left over from the last thing you said.
7. Start a recording, then quit and relaunch the app mid-recording (or
   otherwise force the widget to lose the WebSocket-ish event stream
   momentarily) — mostly a sanity check that nothing crashes or leaves a
   dangling polling thread; check Settings → Logs for anything alarming
   after stopping a recording.

## Pass criteria

The meter visibly and smoothly reacts to real speech in all three widget
modes (steps 2, 4, 5), the dynamic range feels usable rather than
pinned-high or invisible (step 3), and it resets cleanly on stop (step 6)
with nothing broken by starting/stopping repeatedly (step 7).
