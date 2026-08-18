# Isolated Voice mode (energy-gate fallback + speaker-embedding enrollment)

Branch: `JFinchy/ropefish`. Phase 1 (the energy-gate fallback) merged into
`main` as `d7b448c` before the manual-testing-inbox convention existed in
this worktree, so its verification is included here retroactively,
alongside phase 2 (voice enrollment + speaker-embedding masking), which is
new.
Files changed: `src-tauri/src/isolate.rs` (new), `src-tauri/src/voice_isolation.rs`
(new), `src-tauri/src/stt.rs`, `src-tauri/src/recording.rs`,
`src-tauri/src/config.rs`, `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json`, `src/SettingsView.tsx`

## What to verify

A Settings toggle that filters a recording down to the primary user's
voice before Whisper transcribes it, so background noise or a second
speaker doesn't end up in the transcript. Two paths, auto-selected: an
energy/VAD heuristic (phase 1) that only suppresses quiet background
noise, and a speaker-embedding cosine-similarity match (phase 2) that can
actually reject a second real voice once you've enrolled yours. Both are
covered by 86 automated unit/integration tests, but neither has been
exercised with a live mic yet.

## Steps

1. `bun run tauri dev`. Settings → Voice Isolation section should show
   the toggle (off by default) and "Not enrolled".
2. **Baseline (toggle off)**: dictate a normal sentence with the toggle
   off. Confirm it transcribes exactly as it would today — no regression.
3. **Phase 1, quiet room (toggle on, not enrolled)**: turn Isolated Voice
   on. Dictate the same sentence in a normal/quiet setting. Confirm no
   meaningful change vs. step 2 — the energy gate shouldn't eat real
   speech.
4. **Phase 1, background noise**: with the toggle still on and
   unenrolled, dictate while quiet background noise is present (fan,
   typing, distant TV/music). Confirm the noise doesn't bleed into the
   transcript the way it would with the toggle off.
5. **Phase 2, enroll**: Settings → Voice Isolation → "Enroll voice".
   Speak naturally for a few seconds, then "Stop". Confirm status flips
   to "Voice enrolled" with today's date.
6. **Phase 2, solo dictation regression**: with a voice enrolled and the
   toggle on, dictate a normal sentence solo. Confirm it matches what
   you'd get with the toggle off — nothing gets wrongly masked out of
   your own voice.
7. **Phase 2, second-voice rejection**: with a voice enrolled and toggle
   on, dictate while a second real (or played-back) voice talks at
   similar volume. Confirm the second voice does not appear in the
   transcript. Toggle off and repeat — confirm the second voice *does*
   bleed through now, proving the toggle (not luck) did the rejection.
8. **Fully-masked buffer**: with a voice enrolled and toggle on, hold the
   push-to-talk key while only the *other* voice speaks (you stay
   silent). Confirm this degrades to the existing "no speech detected"
   behavior rather than crashing or pasting garbage.
9. **Enrollment hotkey guard**: start voice enrollment (recording
   indicator active), then press the push-to-talk hotkey mid-enrollment.
   Confirm nothing happens — dictation should not fire on the enrollment
   clip. Then press "Stop" to finish enrollment normally.
10. **Not-enough-speech error**: start enrollment and immediately press
    "Stop" (under ~1s of talking). Confirm a friendly error appears
    (not a crash) asking you to speak longer.

## Pass criteria

Steps 2-4 show no regression and real noise suppression with the toggle
on but unenrolled. Steps 5-10 show enrollment works, solo dictation is
unaffected, a second voice is reliably rejected only when enrolled *and*
enabled, and the enrollment/hotkey guard holds. If similarity-threshold
tuning is needed (rejects your own voice, or fails to reject a clearly
different second voice), note the observed behavior rather than just
failing the pass — `SIMILARITY_THRESHOLD` in `isolate.rs` (currently
`0.5`) is the knob to adjust.
