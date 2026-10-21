# Transcribe while you talk

Branch: `worktree-agent-a004497dc03fe9cc1`
Files changed: `src-tauri/src/live.rs` (new), `recording.rs`, `audio.rs`, `stt.rs`, `config.rs`, `WidgetView.tsx`, `settings/sections/voice.tsx`

## What to verify

Push-to-talk now transcribes finished phrases in the background and skips long silences, so stopping only transcribes the last few seconds. Needs a human because it depends on the real mic, VAD and timing.

## Steps

1. Run the app, open Settings > Voice and confirm "Transcribe while you talk" is checked.
2. Set the widget to Detailed. Record for about 60 s: say two sentences, stay silent for 30 s, say two more, pause 2 s, say a final sentence. Greyed live text should appear in the widget after each pause.
3. Stop. Transcribing should finish in about a second and the pasted text should contain all five sentences in order, with no repeats or clipped words.
4. Check Settings > Advanced > Logs (or app.log) for `live: chunks=N ... skipped_silence=~30s ... stop_to_text_ms=...`.
5. Record a 2 s dictation: the log should say `live: transcribing the whole recording (recording under 3s)`.
6. Enable Isolated Voice and repeat step 2; text should still be correct.
7. Pin a mode to a different Whisper model, dictate in that app with a pause; confirm it works.
8. Turn the toggle off and dictate: old behavior, no `live:` lines, no live text.

## Pass criteria

Text matches what was said in every case, stop-to-text is a few hundred ms for the long-pause recording, and the log shows the expected `live:` lines.
