#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?uso: make-tarball.sh <versão>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGE="$ROOT/build/simion-linux-${VERSION}"
OUT="$ROOT/dist/simion_${VERSION}_linux_x86_64.tar.xz"

rm -rf "$STAGE"
mkdir -p "$STAGE" "$ROOT/dist"

install -m 0755 "$ROOT/target/release/simion" "$STAGE/simion"
install -m 0644 "$ROOT/packaging/simion.desktop" "$STAGE/simion.desktop"
install -m 0644 "$ROOT/assets/branding/simion-512.png" "$STAGE/simion.png"
install -m 0644 "$ROOT/README.md" "$STAGE/README.md"
install -m 0644 "$ROOT/LICENSE" "$STAGE/LICENSE"
install -m 0755 "$ROOT/packaging/install-tarball.sh" "$STAGE/install.sh"

tar -cJf "$OUT" -C "$STAGE" .
echo "Tarball: $OUT"
