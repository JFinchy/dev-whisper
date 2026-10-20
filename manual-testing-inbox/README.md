# Manual testing inbox

Coordination point for merging several worktrees' worth of changes into
`main` without losing track of what still needs a human to click through it.

## The convention

What to test next lives in `PRIORITIES.md`; each feature's steps live in
its own `<feature-slug>.md` file in this folder.

- A branch that's implementation-complete (compiles, unit tests pass) but
  hasn't been manually exercised yet adds one file here:
  `<feature-slug>.md`, named for the feature, not the branch, plus one
  line in `PRIORITIES.md` under the tier matching its impact.
- Both are committed as part of the branch's normal diff, so they merge
  into `main` along with the code being tested.
- The file states, at the top, which branch/worktree it came from — that's
  the only way a tester (or the next agent) knows where to route a fix.
- Work through `PRIORITIES.md` top-down, Critical first, against the
  merged build.
- **Pass** → move its line to **Backlog** in `PRIORITIES.md`, dated. The
  detail file stays here as the record of what was verified and how.
  The record of the feature itself is `FEATURES.md`.
- **Fail** → leave its line in its tier with a one-line note on what
  broke. Fix on the originating worktree, re-merge, re-test.
- **Critical/Important/Minor all empty** = everything merged so far has
  been manually verified against current `main`.

**Tiers:** Critical = blocks moving forward (everyday dictation,
releases). Important = a core feature, nothing blocked on it. Minor =
cosmetic, edge case or polish. Backlog = passed, newest first. Pick by
impact if it were broken, not by recency.

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
