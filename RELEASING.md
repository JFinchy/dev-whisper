# Cutting a release

One-time setup, then a 3-step checklist per release.

## One-time setup (already done on this machine, not yet on GitHub)

A minisign keypair for signing update packages lives at
`~/.tauri/dev-whisper.key` (private, password in the sibling
`~/.tauri/dev-whisper.key.password` file) and
`~/.tauri/dev-whisper.key.pub` (public — already embedded in
`tauri.conf.json`'s `plugins.updater.pubkey`). **Never commit the private
key or password file** — they're outside this repo on purpose.

The GitHub Actions release workflow (`.github/workflows/release.yml`)
needs that private key and password as repo secrets:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/dev-whisper.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < ~/.tauri/dev-whisper.key.password
```

If this machine's private key is ever lost, generate a new keypair
(`bunx tauri signer generate -w ~/.tauri/dev-whisper.key`), update
`tauri.conf.json`'s `pubkey` with the new public key, and re-set both
secrets above — but note every already-installed copy of the app will
reject updates signed by the new key until it's manually reinstalled
once, since that's the whole point of the signature check.

## Per-release checklist

1. Bump the version in all three places (there's no tooling to do this
   automatically, so it's a manual step — a stale one silently breaks
   the update check):
   - `package.json`
   - `src-tauri/Cargo.toml` (`version = "..."`)
   - `src-tauri/tauri.conf.json` (`"version": "..."`)
2. Commit that, then tag and push:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. Watch the `Release` workflow run in the Actions tab. When it's green,
   go to the repo's Releases page and **publish the draft it created** —
   it's left as a draft on purpose (see the workflow's comment) so a bad
   build never silently becomes the "latest" version every installed
   copy auto-updates to. Publishing it is what makes the update actually
   visible to the updater endpoint.

Once published, any running copy of the app will see it on its next
launch (or immediately, if someone clicks "Check for updates" in
Settings → About).
