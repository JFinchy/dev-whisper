#!/usr/bin/env bash
# One-time: creates the self-signed "Dev Whisper Local Signing" code-signing
# identity that scripts/dev-sign-runner.sh signs dev builds with, so macOS
# keeps Accessibility/Input Monitoring grants across rebuilds.
#
# Run it yourself in Terminal: the trust step asks for your Mac password.
set -euo pipefail

IDENTITY="Dev Whisper Local Signing"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -q "\"$IDENTITY\""; then
  echo "\"$IDENTITY\" already exists and is trusted. Nothing to do."
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"

cat > cert.cnf <<CNF
[req]
distinguished_name = dn
prompt = no
x509_extensions = ext
[dn]
CN = $IDENTITY
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF

openssl req -x509 -newkey rsa:2048 -nodes -keyout key.pem -out cert.pem -days 3650 -config cert.cnf 2>/dev/null
pw=$(openssl rand -hex 16)
# Legacy PBE/MAC: macOS `security import` can't read OpenSSL 3's defaults.
openssl pkcs12 -export -inkey key.pem -in cert.pem -out id.p12 -passout "pass:$pw" \
  -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1 2>/dev/null
security import id.p12 -k "$KEYCHAIN" -P "$pw" -T /usr/bin/codesign
echo "Trusting the certificate for code signing (macOS will ask for your password)…"
security add-trusted-cert -p codeSign -k "$KEYCHAIN" cert.pem

security find-identity -v -p codesigning | grep "$IDENTITY"
echo "Done. Next 'bun run tauri dev': click Always Allow if codesign asks to use the key,"
echo "then grant Accessibility to the dev app one last time."
