# History keeps the original + learning from corrections

Branch: `worktree-agent-a38db6afd181296fb`
Files changed: `src-tauri/src/corrections.rs` (new), `history.rs`,
`pipeline.rs`, `recording.rs`, `config.rs`, `src/settings/sections/history.tsx`,
`src/settings/sections/vocabulary.tsx`

## What to verify

Refined dictations now keep the pre-LLM text in History ("Refined" marker,
Copy/Use original), and edits to History entries are diffed word by word so
a fix made twice is offered as a vocabulary term plus a replacement rule.
Automated tests cover the diff, thresholds, store, replacement and UI with
a mock backend; only a human can confirm the real LLM path, the real
history file and that the replacement fires on live speech.

## Steps

1. `bun run tauri dev`, with Ollama running and an LLM-refining mode active
   (e.g. Messaging in Slack, or set Default to refine). Settings > LLM:
   turn off "only when needed" so every dictation is refined.
2. Dictate "um let's meet at three" into any text field. Open Settings >
   History: the entry shows a small "Refined" tag. Click it: the original
   ("um let's meet at three") appears with "Copy original" and "Use
   original".
3. Click "Copy original" and paste somewhere: it is the original text.
4. Click "Use original": the entry text becomes the original and the tag
   disappears.
5. Dictate something the LLM leaves unchanged (e.g. turn "only when needed"
   back on and say "ship it"): no tag on that entry.
6. Learning: hover an entry, click the pencil, change one word (e.g.
   "Postgres" typed as "post gress" -> "Postgres"), Save. Do the same fix
   on a second entry. Settings > Vocabulary now shows "You've corrected
   'post gress' to 'Postgres' 2 times. Add it?".
7. Click Add. "Postgres" appears in the vocabulary and a "post gress ->
   Postgres" chip appears under it. Dictate "use post gress here": the
   pasted text says "use Postgres here".
8. Remove the chip with its x; dictate the same phrase again: no rewrite.
9. Edit an entry twice with only a casing or punctuation change: no
   suggestion. Rewrite a whole entry: no suggestion.
10. Uncheck "Learn from my corrections", repeat step 6 twice with a new
    word: no suggestion appears. Re-check it afterward.
11. Quit and relaunch; open History. Old entries (from before this build)
    still load.

## Pass criteria

- Refined entries show the tag and the original; unrefined and old entries
  show neither.
- Use original restores the text and drops the tag.
- A substitution edited twice produces exactly one suggestion card; Add
  adds the vocabulary term and a working replacement; Dismiss removes the
  card and it does not come back.
- Casing/punctuation-only edits and rewrites never suggest anything, and
  nothing is learned while the toggle is off.
- Not built: reading the focused field after a paste to learn corrections
  made outside the app (see the branch report).
