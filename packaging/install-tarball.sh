#!/bin/sh
# Instala o Simion no prefixo do usuário, sem root.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

install -Dm755 "$HERE/simion" "$PREFIX/bin/simion"
install -Dm644 "$HERE/simion.desktop" "$PREFIX/share/applications/simion.desktop"
install -Dm644 "$HERE/simion.png" "$PREFIX/share/icons/hicolor/512x512/apps/simion.png"

echo "Simion instalado em $PREFIX/bin/simion"
case ":${PATH}:" in
  *":$PREFIX/bin:"*) ;;
  *) echo "Adicione $PREFIX/bin ao PATH se necessário." ;;
esac
