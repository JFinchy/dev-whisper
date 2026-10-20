#!/usr/bin/env bash
# Sets the app version everywhere it lives, then prints the release steps.
# Usage: ./scripts/bump-version.sh 0.2.0
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

new="${1:-}"
if ! [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 X.Y.Z" >&2
  exit 1
fi

sed -i '' "s/^  \"version\": \".*\",$/  \"version\": \"$new\",/" package.json src-tauri/tauri.conf.json
# Only the [package] version (the first `version =` line), not dependencies.
sed -i '' "1,/^version = \".*\"$/s/^version = \".*\"$/version = \"$new\"/" src-tauri/Cargo.toml
(cd src-tauri && cargo update -p tauri-app --offline >/dev/null 2>&1 || true)

./scripts/check-versions.sh
cat <<STEPS

Next:
  git commit -am "Bump version to $new"
  git tag v$new && git push origin main v$new
Then publish the draft release once the Release workflow is green.
STEPS
