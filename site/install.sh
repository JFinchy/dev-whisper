#!/bin/sh
# Installs the latest Dev Whisper into /Applications.
#
#   curl -fsSL https://jfinchy.github.io/dev-whisper/install.sh | sh
#
# It downloads the same Dev-Whisper.dmg as the website's download button,
# straight from GitHub Releases, checks the app's signature, and copies it
# into Applications. Because curl (unlike a browser) doesn't mark the file
# as downloaded, macOS doesn't show its "can't verify" prompt on first
# launch. Nothing else is changed on your Mac.
#
# Options (environment variables):
#   DW_APP_DIR=~/Applications   install somewhere other than /Applications
#   DW_NO_OPEN=1                don't open the app afterwards

set -eu

URL="https://github.com/JFinchy/dev-whisper/releases/latest/download/Dev-Whisper.dmg"
APP_NAME="Dev Whisper.app"
APP_DIR="${DW_APP_DIR:-/Applications}"

fail() {
  printf 'Dev Whisper: %s\n' "$1" >&2
  exit 1
}

[ "$(uname -s)" = "Darwin" ] || fail "this installer is for macOS."
[ "$(uname -m)" = "arm64" ] || fail "Dev Whisper needs a Mac with Apple Silicon (M1 or later)."

tmp=$(mktemp -d)
mounted=""
cleanup() {
  [ -n "$mounted" ] && hdiutil detach "$tmp/mnt" -quiet >/dev/null 2>&1 || true
  rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

echo "Downloading Dev Whisper…"
curl -fL --progress-bar -o "$tmp/Dev-Whisper.dmg" "$URL" || fail "the download failed."

hdiutil attach -nobrowse -readonly -quiet -mountpoint "$tmp/mnt" "$tmp/Dev-Whisper.dmg" \
  || fail "couldn't open the downloaded disk image."
mounted=1

app="$tmp/mnt/$APP_NAME"
[ -d "$app" ] || fail "the disk image doesn't contain $APP_NAME."
codesign --verify --deep --strict "$app" 2>/dev/null || fail "the app's signature check failed, so it wasn't installed."

mkdir -p "$APP_DIR" 2>/dev/null || true
dest="$APP_DIR/$APP_NAME"
if [ ! -w "$APP_DIR" ] || { [ -e "$dest" ] && [ ! -w "$dest" ]; }; then
  fail "can't write to $APP_DIR. Run it with DW_APP_DIR=~/Applications, or ask an admin."
fi

if pgrep -f "$dest/Contents/MacOS/" >/dev/null 2>&1; then
  fail "Dev Whisper is running. Quit it from the menu bar, then run this again."
fi

rm -rf "$dest"
ditto "$app" "$dest" || fail "copying into $APP_DIR failed."
echo "Installed $dest"

if [ -z "${DW_NO_OPEN:-}" ]; then
  open "$dest"
  echo "Opening Dev Whisper. The setup checklist in Settings walks you through the rest."
fi
