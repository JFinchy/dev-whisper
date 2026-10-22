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
# Without admin rights /Applications is read-only; use the user's own
# Applications folder instead (unless a folder was asked for).
if [ ! -w "$APP_DIR" ] && [ -z "${DW_APP_DIR:-}" ]; then
  APP_DIR="$HOME/Applications"
  mkdir -p "$APP_DIR"
  echo "No write access to /Applications, so installing into ~/Applications."
fi
[ -w "$APP_DIR" ] || fail "can't write to $APP_DIR."
dest="$APP_DIR/$APP_NAME"

if pgrep -f "$dest/Contents/MacOS/" >/dev/null 2>&1; then
  fail "Dev Whisper is running. Quit it from the menu bar, then run this again."
fi

# Copy the new app in beside the old one, then swap them, so a step macOS
# blocks never leaves a half-deleted app behind.
staged="$APP_DIR/.Dev Whisper.installing.app"
rm -rf "$staged"
ditto "$app" "$staged" || fail "copying into $APP_DIR failed."
if [ -e "$dest" ] && ! mv "$dest" "$tmp/previous.app" 2>/dev/null; then
  rm -rf "$staged"
  fail "macOS didn't let your terminal replace the Dev Whisper already in $APP_DIR.
Its App Management setting protects installed apps. Either drag that copy
of Dev Whisper to the Trash and run this again, or allow your terminal in
System Settings > Privacy & Security > App Management."
fi
if ! mv "$staged" "$dest"; then
  [ -e "$tmp/previous.app" ] && mv "$tmp/previous.app" "$dest"
  rm -rf "$staged"
  fail "couldn't move the new app into $APP_DIR; the old one is unchanged."
fi
echo "Installed $dest"

if [ -z "${DW_NO_OPEN:-}" ]; then
  open "$dest"
  echo "Opening Dev Whisper. The setup checklist in Settings walks you through the rest."
fi
