# Manual test inbox

Things that need a human at a live mic to verify — not checkable by
`cargo test`/`tsc`, so they don't block a commit but do block calling a
feature done. Check items off as they're verified; move a fully-checked
entry's date to "done" in `BACKLOG.md`/`ROADMAP.md` and delete it from
here.

## Isolated Voice mode, phase 2 (2026-08-17)

Speaker-embedding enrollment + cosine-similarity masking — see
`BACKLOG.md` for the full design.

- [ ] Enroll for real (Settings → Voice Isolation → Enroll voice, speak
      for a few seconds, Stop) → status flips to "Voice enrolled".
- [ ] Dictate solo with Isolated Voice on → no transcript regression vs.
      it off (nothing gets wrongly masked out of your own speech).
- [ ] Dictate with a second real/played voice at similar volume, toggle
      Isolated Voice on/off → second voice drops out of the transcript
      only when on and enrolled.
- [ ] Repeat the second-voice test with no enrollment → confirm the
      energy-gate fallback helps with quiet background noise but is
      knowingly weaker against a second real voice at similar volume
      (expected, not a bug).
- [ ] A fully-masked buffer (only the other voice present, enrolled)
      degrades into the existing "no speech detected" path rather than
      crashing.
- [ ] A hotkey press during an in-progress enrollment is inert (the
      `RecordingPurpose` guard in `recording.rs`) — doesn't misfire
      `transcribe_and_paste` on the enrollment clip.
- [ ] Tune `SIMILARITY_THRESHOLD` (`isolate.rs`, currently 0.5) against
      real voices if same-speaker clips read too low or different-speaker
      clips read too high.
