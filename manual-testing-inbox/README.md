# Manual testing inbox

Coordination point for merging several worktrees' worth of changes into
`main` without losing track of what still needs a human to click through it.

## The convention

- A branch that's implementation-complete (compiles, unit tests pass) but
  hasn't been manually exercised yet adds one file here:
  `<feature-slug>.md`, named for the feature, not the branch.
- That file is committed as part of the branch's normal diff, so it merges
  into `main` along with the code it's testing.
- The file states, at the top, which branch/worktree it came from — that's
  the only way a tester (or the next agent) knows where to route a fix.
- Once several branches have merged, everything queued for a manual pass
  is sitting here together in `main`. Work through each file against the
  merged build.
- **Pass** → delete the file in the same testing session (that's why these
  are "to-be-deleted": nothing here is meant to be permanent documentation;
  once verified, the real record of the feature is `FEATURES.md`).
- **Fail** → do *not* delete it. Note what broke (edit the file or reply to
  whoever's tracking the session), go fix it on the originating worktree,
  re-merge, and re-test. The file stays until it passes.
- **Inbox empty** = every merged branch has been manually verified against
  the current `main`. Non-empty = something is still pending a human check
  or is a known-broken fix-in-progress.

## File format

```markdown
# <Feature name>

Branch: `<worktree-branch-name>`
Files changed: <short list, or "see `git log` on the branch">

## What to verify

<1-2 sentences on what this feature does and why it needs a human, not
just the automated tests, to check it.>

## Steps

1. ...
2. ...

## Pass criteria

<what "it worked" looks like, concretely>
```

Keep steps concrete and copy-pasteable (exact commands, exact UI labels) —
whoever runs this pass may not be the person who wrote the feature.
