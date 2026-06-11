#!/usr/bin/env bash
# Rebuild the Tauri bundles (deb + AppImage) and immediately launch the
# freshly built AppImage. Use this when you want to test the packaged
# artifact end-to-end with your latest code changes.
#
# For day-to-day development, prefer `npm run dev` instead -- it is much
# faster (debug build, no bundling) and auto-reloads on changes.

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

echo ">>> Rebuilding bundles via scripts/rebuild.sh"
bash "$ROOT/scripts/rebuild.sh"

APPIMAGE_DIR="$ROOT/bundle/appimage"

shopt -s nullglob
APPIMAGES=("$APPIMAGE_DIR"/*.AppImage)
shopt -u nullglob

if [ "${#APPIMAGES[@]}" -eq 0 ]; then
    echo "!! No AppImage found in $APPIMAGE_DIR"
    exit 1
fi

# Pick the most recently modified AppImage in case multiple versions exist.
LATEST=""
LATEST_TS=0
for img in "${APPIMAGES[@]}"; do
    ts=$(stat -c %Y "$img" 2>/dev/null || echo 0)
    if [ "$ts" -gt "$LATEST_TS" ]; then
        LATEST_TS=$ts
        LATEST="$img"
    fi
done

echo ""
echo ">>> Marking AppImage executable: $LATEST"
chmod +x "$LATEST"

echo ">>> Launching $LATEST"
exec "$LATEST"
