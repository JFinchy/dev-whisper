# Dev Whisper

See `PRODUCT_SPEC.md` for the product vision, `FEATURES.md` for what's
built, `ROADMAP.md` for what's left, and `BACKLOG.md` for full detail
behind each backlog item (known issues, research findings, effort
estimates).

## Manual testing inbox

Work happens across several git worktrees at once (see `git worktree
list`), each on its own branch. `manual-testing-inbox/` is the
coordination point for merging them into `main` without losing track of
what still needs a human to click through it — full convention is
documented in `manual-testing-inbox/README.md`, summarized here:

- A branch that's implementation-complete (compiles, unit tests pass) but
  not yet manually exercised adds `manual-testing-inbox/<feature-slug>.md`
  as part of its normal diff, so the file merges into `main` along with
  the code it's testing. The file names its originating branch and gives
  concrete, copy-pasteable steps to verify the feature.
- Once several branches have merged, everything queued for a manual pass
  sits here together in `main` — work through each file against the
  merged build, not against the individual branch.
- **Pass** → delete the file. These are explicitly to-be-deleted; the
  permanent record of a shipped feature is `FEATURES.md`, not this
  folder.
- **Fail** → don't delete it. Note what broke, fix it on the originating
  worktree, re-merge, re-test. The file stays until it passes.
- **Inbox empty** = everything merged so far has been manually verified
  against current `main`.
