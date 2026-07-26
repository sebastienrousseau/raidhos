#!/usr/bin/env bash
# Build a portable AppImage of the RaidhOS CLI + helper.
# Requires `appimagetool` from https://github.com/AppImage/AppImageKit.
#
# Run from the repo root:
#   packaging/appimage/build-appimage.sh
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/../.." && pwd)
APPDIR="$ROOT_DIR/build/RaidhOS.AppDir"
TARGET_DIR="$ROOT_DIR/target/release"

if [[ ! -x "$TARGET_DIR/raidhos-cli" ]]; then
    echo "build the release binaries first:" >&2
    echo "  cargo build --release --workspace --exclude raidhos-ui" >&2
    exit 1
fi
if ! command -v appimagetool >/dev/null 2>&1; then
    echo "appimagetool not found. Download from:" >&2
    echo "  https://github.com/AppImage/AppImageKit/releases" >&2
    exit 1
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$TARGET_DIR/raidhos-cli" "$APPDIR/usr/bin/"
cp "$TARGET_DIR/raidhos-priv-helper" "$APPDIR/usr/bin/"

cat > "$APPDIR/raidhos.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=RaidhOS
Comment=Memory-safe multi-ISO USB imager
Exec=raidhos-cli
Icon=raidhos
Terminal=true
Categories=System;Utility;
EOF

# Use the same placeholder icon as the Tauri bundle.
if [[ -s "$ROOT_DIR/crates/ui-tauri/src-tauri/icons/icon.png" ]]; then
    cp "$ROOT_DIR/crates/ui-tauri/src-tauri/icons/icon.png" "$APPDIR/raidhos.png"
    cp "$APPDIR/raidhos.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/raidhos.png"
fi

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE=$(dirname "$(readlink -f "$0")")
exec "$HERE/usr/bin/raidhos-cli" "$@"
EOF
chmod +x "$APPDIR/AppRun"

appimagetool "$APPDIR" "$ROOT_DIR/build/RaidhOS-x86_64.AppImage"
echo "wrote $ROOT_DIR/build/RaidhOS-x86_64.AppImage"
