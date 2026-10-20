#!/usr/bin/env bash
# Dev/test shortcut: fetches the default Whisper model into src-tauri/models
# (the app's dev fallback location, see models.rs). Pinned and verified the
# same way the in-app downloader is.
set -euo pipefail

MODEL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/src-tauri/models"
MODEL_FILE="$MODEL_DIR/ggml-base.en-q5_1.bin"
URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en-q5_1.bin"
SHA256="4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f"

mkdir -p "$MODEL_DIR"

if [ -f "$MODEL_FILE" ]; then
  echo "Model already present at $MODEL_FILE"
  exit 0
fi

echo "Downloading Whisper base.en (quantized q5_1, ~59MB) to $MODEL_FILE"
curl -L --fail -o "$MODEL_FILE.tmp" "$URL"
echo "$SHA256  $MODEL_FILE.tmp" | shasum -a 256 -c - >/dev/null || {
  rm -f "$MODEL_FILE.tmp"
  echo "Checksum mismatch, download discarded." >&2
  exit 1
}
mv "$MODEL_FILE.tmp" "$MODEL_FILE"
echo "Done (checksum verified)."
