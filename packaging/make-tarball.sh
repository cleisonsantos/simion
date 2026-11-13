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
install -m 0644 "$ROOT/INSTALL.md" "$STAGE/INSTALL.md"
install -m 0755 "$ROOT/packaging/install-tarball.sh" "$STAGE/install.sh"
for size in 128 64; do
  if [[ -f "$ROOT/assets/branding/simion-${size}.png" ]]; then
    install -m 0644 "$ROOT/assets/branding/simion-${size}.png" "$STAGE/simion-${size}.png"
  fi
done

tar -cJf "$OUT" -C "$STAGE" .

# Confere o tarball recém-criado: um install.sh vazio ou sem +x não pode sair (#27).
CHECK="$(mktemp -d)"
trap 'rm -rf "$CHECK"' EXIT
tar -xJf "$OUT" -C "$CHECK"
if [[ ! -s "$CHECK/install.sh" ]] || (( $(wc -c < "$CHECK/install.sh") <= 100 )); then
  echo "✗ $OUT: install.sh ausente ou vazio" >&2
  exit 1
fi
if [[ ! -x "$CHECK/install.sh" ]]; then
  echo "✗ $OUT: install.sh sem permissão de execução" >&2
  exit 1
fi
if ! sh -n "$CHECK/install.sh"; then
  echo "✗ $OUT: install.sh com erro de sintaxe" >&2
  exit 1
fi
echo "Tarball: $OUT"
