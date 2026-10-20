#!/usr/bin/env bash
# Fails if package.json, Cargo.toml and tauri.conf.json disagree on the
# version. A stale one silently breaks the update check (see RELEASING.md).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

pkg=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json)
cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' src-tauri/Cargo.toml | head -1)
tauri=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' src-tauri/tauri.conf.json)

if [ "$pkg" = "$cargo" ] && [ "$cargo" = "$tauri" ]; then
  echo "versions agree: $pkg"
else
  echo "version mismatch: package.json=$pkg Cargo.toml=$cargo tauri.conf.json=$tauri" >&2
  exit 1
fi
