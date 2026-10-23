#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?uso: make-dmg.sh <versão>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${SIMION_BIN:-$ROOT/build/Simion}"
APP="$ROOT/build/Simion.app"
ICONSET="$ROOT/build/Simion.iconset"
DMG_STAGE="$ROOT/build/dmg"
DMG="$ROOT/dist/Simion-${VERSION}-universal.dmg"
ZIP="$ROOT/dist/Simion-${VERSION}-macos-universal.zip"

[ -x "$BIN" ] || { echo "binário não encontrado: $BIN" >&2; exit 1; }

rm -rf "$APP" "$ICONSET" "$DMG_STAGE" "$DMG" "$ZIP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ICONSET" "$DMG_STAGE" "$ROOT/dist"

cp "$BIN" "$APP/Contents/MacOS/Simion"
chmod 0755 "$APP/Contents/MacOS/Simion"
sed "s|\${VERSION}|$VERSION|g" "$ROOT/packaging/macos/Info.plist" > "$APP/Contents/Info.plist"
cp "$ROOT/LICENSE" "$APP/Contents/Resources/LICENSE"

icon() {
  size="$1"; name="$2"
  sips -z "$size" "$size" "$ROOT/assets/branding/simion-1024.png" --out "$ICONSET/$name" >/dev/null
}
icon 16 icon_16x16.png
icon 32 icon_16x16@2x.png
icon 32 icon_32x32.png
icon 64 icon_32x32@2x.png
icon 128 icon_128x128.png
icon 256 icon_128x128@2x.png
icon 256 icon_256x256.png
icon 512 icon_256x256@2x.png
icon 512 icon_512x512.png
icon 1024 icon_512x512@2x.png
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/simion.icns"

# Assinatura ad-hoc: não é notarização, mas evita o aviso de binário "danificado"
# em Macs Apple Silicon. O Gatekeeper ainda pode pedir para abrir com botão direito.
codesign --force --deep --sign - "$APP"

ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"

cp -R "$APP" "$DMG_STAGE/Simion.app"
ln -s /Applications "$DMG_STAGE/Applications"
hdiutil create -volname "Simion" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG"

echo "DMG: $DMG"
echo "ZIP: $ZIP"
