#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/src-tauri/models"
MODEL_FILE="$MODEL_DIR/ggml-base.en-q5_1.bin"
URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin"

mkdir -p "$MODEL_DIR"

if [ -f "$MODEL_FILE" ]; then
  echo "Model already present at $MODEL_FILE"
  exit 0
fi

echo "Downloading Whisper base.en (quantized q5_1, ~59MB) to $MODEL_FILE"
curl -L --fail -o "$MODEL_FILE.tmp" "$URL"
mv "$MODEL_FILE.tmp" "$MODEL_FILE"
echo "Done."
