#!/usr/bin/env bash
# Smoke do instalador do tarball (#27): extrai, instala num prefixo temporário,
# confere, reinstala, desinstala e testa os erros de origem e de flag.
set -euo pipefail

TARBALL="$(cd "$(dirname "${1:?uso: ci-install-smoke.sh <tarball>}")" && pwd)/$(basename "$1")"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT

fail() {
  echo "✗ smoke: $*" >&2
  exit 1
}

mkdir "$T/pkg"
tar -xJf "$TARBALL" -C "$T/pkg"
cd "$T/pkg"

sh -n install.sh || fail "install.sh com erro de sintaxe"
[[ -x install.sh ]] || fail "install.sh sem +x"

PREFIX="$T/prefix" ./install.sh --dry-run
[[ ! -e "$T/prefix" ]] || fail "--dry-run alterou o prefixo"

PREFIX="$T/prefix" ./install.sh
"$T/prefix/bin/simion" --version
[[ -f "$T/prefix/share/applications/simion.desktop" ]] || fail "simion.desktop não instalado"
[[ -f "$T/prefix/share/icons/hicolor/512x512/apps/simion.png" ]] || fail "ícone 512 não instalado"

PREFIX="$T/prefix" ./install.sh || fail "segunda instalação falhou"

PREFIX="$T/prefix" ./install.sh --uninstall
[[ ! -e "$T/prefix/bin/simion" ]] || fail "--uninstall deixou o binário"
[[ ! -e "$T/prefix/share/applications/simion.desktop" ]] || fail "--uninstall deixou o .desktop"

mkdir "$T/broken"
cp install.sh "$T/broken/"
rc=0
"$T/broken/install.sh" || rc=$?
[[ $rc -eq 1 ]] || fail "bundle incompleto saiu com $rc, esperado 1"

rc=0
./install.sh --bogus || rc=$?
[[ $rc -eq 1 ]] || fail "flag desconhecida saiu com $rc, esperado 1"

echo "smoke ok"
