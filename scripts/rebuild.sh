#!/usr/bin/env bash
# Rebuild Tauri bundles (deb + AppImage) and copy them into ./bundle/.
#
# Why fakeroot?
#   Tauri's deb writer produces a corrupt ar header when invoked under a
#   domain user with a 6+ digit uid/gid (observed: bad archive header magic).
#   Running under fakeroot makes uid/gid appear as 0:0 inside the bundler,
#   which sidesteps the bug and yields a valid .deb.

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

BUILD_TARGETS="${BUILD_TARGETS:-deb,appimage}"
DEST="$ROOT/bundle"

echo ">>> Rebuilding Tauri app (targets: $BUILD_TARGETS)"

RUN_PREFIX=""
if command -v fakeroot >/dev/null 2>&1; then
    RUN_PREFIX="fakeroot"
else
    echo "!! fakeroot not found - the .deb may be corrupt if your uid > 999999."
    echo "   Install with: sudo apt-get install fakeroot"
fi

$RUN_PREFIX npx tauri build --bundles "$BUILD_TARGETS"

echo ""
echo ">>> Locating built bundles"

CANDIDATE_DIRS=(
    "$ROOT/src-tauri/target/release/bundle"
    "/tmp/cursor-sandbox-cache"
)

SRC_BUNDLE=""
NEWEST_TS=0
for base in "${CANDIDATE_DIRS[@]}"; do
    [ -d "$base" ] || continue
    while IFS= read -r dir; do
        [ -d "$dir/deb" ] || [ -d "$dir/appimage" ] || continue
        ts=$(stat -c %Y "$dir" 2>/dev/null || echo 0)
        if [ "$ts" -gt "$NEWEST_TS" ]; then
            NEWEST_TS=$ts
            SRC_BUNDLE="$dir"
        fi
    done < <(find "$base" -type d -name bundle 2>/dev/null)
done

if [ -z "$SRC_BUNDLE" ]; then
    echo "!! Could not find a freshly built bundle directory."
    exit 1
fi

echo "    Source: $SRC_BUNDLE"
echo "    Dest:   $DEST"

mkdir -p "$DEST/deb" "$DEST/appimage"

if [ -d "$SRC_BUNDLE/deb" ]; then
    find "$SRC_BUNDLE/deb" -maxdepth 1 -name '*.deb' -exec cp -v {} "$DEST/deb/" \;
fi
if [ -d "$SRC_BUNDLE/appimage" ]; then
    find "$SRC_BUNDLE/appimage" -maxdepth 1 -name '*.AppImage' -exec cp -v {} "$DEST/appimage/" \;
fi

echo ""
echo ">>> Verifying .deb integrity"
shopt -s nullglob
deb_ok=1
for deb in "$DEST/deb/"*.deb; do
    if ar t "$deb" >/dev/null 2>&1; then
        echo "    OK   $deb"
    else
        echo "    FAIL $deb (corrupt ar header)"
        deb_ok=0
    fi
done

echo ""
if [ "$deb_ok" -eq 1 ]; then
    echo ">>> Done. Install with:"
    echo "    sudo dpkg -i \"$DEST/deb/\"*.deb"
else
    echo ">>> Done with warnings - at least one .deb is corrupt."
    exit 2
fi
