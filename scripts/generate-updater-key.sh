#!/usr/bin/env bash
# Generate the ed25519 keypair used to sign update bundles.
#
# Run this ONCE per project. Keep the private key secret -- if it leaks,
# anyone can publish a "valid" update for your app.
#
# Outputs:
#   ~/.tauri/lambda-finder.key       <-- PRIVATE key (DO NOT COMMIT)
#   ~/.tauri/lambda-finder.key.pub   <-- PUBLIC key (paste into tauri.conf.json)
#
# After running this script:
#   1. Copy the contents of ~/.tauri/lambda-finder.key.pub
#   2. Replace `REPLACE_WITH_BASE64_PUBLIC_KEY` in src-tauri/tauri.conf.json
#   3. Export the private key whenever you build a release:
#         export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/lambda-finder.key)"
#         export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # or your chosen password

set -euo pipefail

cd "$(dirname "$0")/.."

KEY_DIR="${HOME}/.tauri"
KEY_PATH="${KEY_DIR}/lambda-finder.key"

mkdir -p "$KEY_DIR"
chmod 700 "$KEY_DIR"

if [ -f "$KEY_PATH" ]; then
    echo "!! Key already exists at $KEY_PATH"
    echo "   Aborting so we don't overwrite it. Delete it manually if you really want to regenerate."
    exit 1
fi

echo ">>> Generating updater keypair at $KEY_PATH"
echo "    (You'll be prompted for an optional password.)"
npx tauri signer generate -w "$KEY_PATH"

PUB_PATH="${KEY_PATH}.pub"
chmod 600 "$KEY_PATH"
chmod 644 "$PUB_PATH" 2>/dev/null || true

echo ""
echo ">>> Done. Public key:"
echo ""
cat "$PUB_PATH"
echo ""
echo ">>> Next steps:"
echo "    1. Copy the line above and paste it into src-tauri/tauri.conf.json"
echo "       under  plugins.updater.pubkey"
echo "    2. Before each release build, export:"
echo "         export TAURI_SIGNING_PRIVATE_KEY=\"\$(cat $KEY_PATH)\""
echo "         export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=\"<your password or empty>\""
echo ""
echo "    Keep $KEY_PATH OUT of git. It is your release identity."
