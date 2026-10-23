#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?uso: make-appimage.sh <versão>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPDIR="$ROOT/build/AppDir"
OUT="$ROOT/dist/Simion-${VERSION}-x86_64.AppImage"
TOOL="${APPIMAGETOOL:-appimagetool}"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/512x512/apps" \
         "$ROOT/dist"

install -m 0755 "$ROOT/target/release/simion" "$APPDIR/usr/bin/simion"
install -m 0644 "$ROOT/packaging/simion.desktop" "$APPDIR/simion.desktop"
install -m 0644 "$ROOT/packaging/simion.desktop" "$APPDIR/usr/share/applications/simion.desktop"
install -m 0644 "$ROOT/assets/branding/simion-512.png" "$APPDIR/simion.png"
install -m 0644 "$ROOT/assets/branding/simion-512.png" "$APPDIR/usr/share/icons/hicolor/512x512/apps/simion.png"

cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/simion" "$@"
APPRUN
chmod 0755 "$APPDIR/AppRun"

ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" "$APPDIR" "$OUT"
echo "AppImage: $OUT"
