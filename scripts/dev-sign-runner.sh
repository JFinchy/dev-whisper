#!/bin/sh
# Cargo runner (see .cargo/config.toml): signs the dev app binary with a
# stable local identity before launching it. Ad-hoc signatures change on
# every build, so macOS treated each rebuild as a new app and revoked its
# Accessibility/Input Monitoring grants. A fixed identity + identifier
# keeps those grants across rebuilds. One-time setup is in README.md
# ("Stable dev signing").
IDENTITY="Dev Whisper Local Signing"
bin="$1"

case "$(basename "$bin")" in
  tauri-app)
    if security find-identity -v -p codesigning 2>/dev/null | grep -q "\"$IDENTITY\""; then
      # Signing can stall on a macOS keychain dialog (e.g. the keychain
      # locked overnight). Don't hang the dev launch on it indefinitely.
      codesign --force --sign "$IDENTITY" --identifier com.devwhisper.app.dev "$bin" 2>/dev/null &
      pid=$!
      waited=0
      while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt 20 ]; do
        sleep 1
        waited=$((waited + 1))
      done
      if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null
        echo "dev-sign-runner: codesign is waiting on a keychain dialog; running UNSIGNED this time (paste will need Accessibility re-granted). Approve the dialog next launch." >&2
      elif ! wait "$pid"; then
        echo "dev-sign-runner: codesign failed; running unsigned" >&2
      fi
    else
      echo "dev-sign-runner: '$IDENTITY' not found; running unsigned (Accessibility will reset each rebuild)" >&2
    fi
    ;;
esac

exec "$@"
