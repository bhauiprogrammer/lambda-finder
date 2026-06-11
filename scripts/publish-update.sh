#!/usr/bin/env bash
# Build a signed release and produce the `latest.json` manifest that the
# in-app updater fetches from your GitHub release.
#
# Prerequisites (run scripts/generate-updater-key.sh once first):
#   export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/lambda-finder.key)"
#   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<password or empty>"
#
# Usage:
#   bash scripts/publish-update.sh "Notes for this release"
#
# Output (under bundle/release/):
#   Lambda Finder_<ver>_amd64.AppImage
#   Lambda Finder_<ver>_amd64.AppImage.sig
#   latest.json
#
# Then, manually:
#   1. Tag the commit and push:    git tag v<ver> && git push --tags
#   2. Create a GitHub release for that tag.
#   3. Upload all THREE files in bundle/release/ as release assets.
#   4. Mark the release as "latest".
# The in-app updater will pick it up on next launch.

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

NOTES="${1:-New version}"

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
    echo "!! TAURI_SIGNING_PRIVATE_KEY is not set."
    echo "   Run: export TAURI_SIGNING_PRIVATE_KEY=\"\$(cat ~/.tauri/lambda-finder.key)\""
    exit 1
fi

VERSION="$(node -p "require('./package.json').version")"
echo ">>> Building & signing release v${VERSION}"

bash "$ROOT/scripts/rebuild.sh"

SRC_APPIMAGE="$(ls -1t "$ROOT/bundle/appimage/"*.AppImage 2>/dev/null | head -n1 || true)"
if [ -z "$SRC_APPIMAGE" ]; then
    echo "!! No AppImage found in bundle/appimage/"
    exit 1
fi

# Tauri writes the .sig next to the bundle in src-tauri/target/release/bundle/.
SIG_SRC="$(find "$ROOT/src-tauri/target/release/bundle/appimage" -maxdepth 1 -name '*.AppImage.sig' 2>/dev/null | head -n1 || true)"
if [ -z "$SIG_SRC" ]; then
    echo "!! No .sig file produced. Did tauri build skip signing?"
    echo "   Make sure TAURI_SIGNING_PRIVATE_KEY is exported before the build."
    exit 1
fi

DEST="$ROOT/bundle/release"
mkdir -p "$DEST"
cp -v "$SRC_APPIMAGE" "$DEST/"
cp -v "$SIG_SRC"      "$DEST/"

APPIMAGE_NAME="$(basename "$SRC_APPIMAGE")"
SIG_NAME="$(basename "$SIG_SRC")"
SIG_CONTENT="$(cat "$DEST/$SIG_NAME")"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

# Read the configured GitHub Releases endpoint to derive the asset URL.
ENDPOINT="$(node -e "
  const fs = require('fs');
  const c = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json','utf8'));
  process.stdout.write(c.plugins.updater.endpoints[0]);
")"

# https://github.com/<owner>/<repo>/releases/latest/download/latest.json
#   ->  https://github.com/<owner>/<repo>/releases/latest/download/<APPIMAGE_NAME>
ASSET_URL="${ENDPOINT%/latest.json}/$APPIMAGE_NAME"

cat > "$DEST/latest.json" <<EOF
{
  "version": "$VERSION",
  "notes": $(node -e "process.stdout.write(JSON.stringify(process.argv[1]))" "$NOTES"),
  "pub_date": "$PUB_DATE",
  "platforms": {
    "linux-x86_64": {
      "signature": "$SIG_CONTENT",
      "url": "$ASSET_URL"
    }
  }
}
EOF

echo ""
echo ">>> Release artifacts ready in $DEST"
ls -la "$DEST"
echo ""
echo ">>> Next:"
echo "    git tag v$VERSION && git push --tags"
echo "    gh release create v$VERSION \\"
echo "        \"$DEST/$APPIMAGE_NAME\" \\"
echo "        \"$DEST/$SIG_NAME\" \\"
echo "        \"$DEST/latest.json\" \\"
echo "        --title \"v$VERSION\" --notes \"$NOTES\" --latest"
