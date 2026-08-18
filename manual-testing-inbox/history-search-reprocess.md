# History reprocessing + full-text search

Branch: `JFinchy/hippocamp`
Files changed: `src-tauri/src/history.rs`, `src-tauri/src/lib.rs`,
`src/SettingsView.tsx`

## What to verify

Two additions to the History settings section:

1. **Full-text search** — a search box filters the entire retained
   history (not just the 200 most recent entries the list view normally
   shows), matching against transcript text, journal summary, or app name.
2. **Reprocessing** — a "↻" button per entry lets you re-run its saved
   text through a different Mode (and optionally the LLM refinement pass),
   preview the result, then copy it or replace the stored entry with it.
   This reformats the *saved text*, not a re-transcription from audio —
   there's no original recording kept around to re-run through Whisper.

Automated coverage (`cargo test --lib history::`) checks the search
matching predicate in isolation, but nothing automated exercises the real
Tauri command wiring, the search box, or the reprocess UI end to end —
that's what this pass is for.

## Steps

1. From repo root: `bun run tauri dev`. Look for the mic icon in the menu
   bar (not a window).
2. Dictate a handful of distinct test transcripts (5-10) across a couple
   of different apps, so History has enough variety to search and
   reprocess against.
3. Open Settings → **History**. Confirm all recent entries show as usual.
4. Type a word you know is in one of your test transcripts into the new
   search box. Confirm the list narrows to just matching entries, and that
   it's actually reaching into the full history, not just what happened to
   already be visible.
5. Type something that matches nothing. Confirm you get "No transcripts
   match "...""  rather than an empty list with no explanation.
6. Clear the search box. Confirm the list reverts to the normal
   recent-first view.
7. Trigger a *new* dictation while a search query is still active in the
   box. Confirm the list re-filters against your active search rather than
   silently reverting to the unfiltered view (this exercises the
   re-subscribed `history-updated` listener, not just the initial fetch).
8. Pick an entry, click **↻**. Change the mode dropdown, leave "Refine
   with LLM" unchecked, click **Run**. Confirm a formatted preview appears
   below (e.g. switching to a mode that changes casing/structure should
   visibly change the text).
9. Check **Refine with LLM** and **Run** again (needs Ollama running).
   Confirm the preview updates to the LLM-refined version, and that it
   takes a bit longer (the "Run" button should show a spinner while
   waiting).
10. Click **Copy result** and paste elsewhere to confirm it copied the
    *reprocessed* text, not the original.
11. Click **Replace** on a different reprocess run. Confirm the entry's
    displayed text updates in place to the reprocessed version, and that
    it survives a Settings window close/reopen (i.e. it's actually
    persisted, not just a local UI change).
12. Click **↻** again on an entry with no result yet, then **Discard** —
    confirm the panel closes without changing anything.

## Pass criteria

Search (steps 4-7) reaches the full history and stays in sync with new
entries; reprocessing (steps 8-12) produces correct output for both the
plain-formatting and LLM-refined paths, Copy and Replace both do what they
say, and Replace persists across a Settings window reopen.
