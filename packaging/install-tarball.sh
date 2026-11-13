#!/bin/sh
# Instalador do tarball portátil do Simion (vira o install.sh do .tar.xz).
# Sem flags: instala em $PREFIX (padrão ~/.local), sem root.
# Saída: 0 ok, 1 origem/flag inválida, 2 permissão, 3 dependência ausente.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
PREFIX="${PREFIX:-$HOME/.local}"
DRY_RUN=0
UNINSTALL=0
SYSTEM=0

usage() {
	cat <<'EOF'
Uso: ./install.sh [opções]

Instala o Simion a partir desta pasta (o tarball extraído).

Opções:
  --system      instala em /usr/local (exige root; não uso sudo por conta própria)
  --dry-run     só mostra o que seria feito, sem alterar nada
  --uninstall   remove o que o instalador colocou no prefixo
  -h, --help    mostra esta ajuda

Sem opções, instala em ~/.local (ou em $PREFIX, se definido), sem root:
  bin/simion
  share/applications/simion.desktop
  share/icons/hicolor/{512x512,128x128,64x64}/apps/simion.png

Exemplos:
  ./install.sh
  PREFIX=/opt/simion ./install.sh
  sudo ./install.sh --system
  ./install.sh --uninstall
EOF
}

die() {
	code=$1
	shift
	printf '✗ %s\n' "$*" >&2
	exit "$code"
}

step() {
	printf '→ %s\n' "$*"
}

# Caches do menu e dos ícones: opcionais e silenciosos.
refresh_caches() {
	if command -v update-desktop-database >/dev/null 2>&1 && [ -d "$PREFIX/share/applications" ]; then
		update-desktop-database -q "$PREFIX/share/applications" >/dev/null 2>&1 || true
	fi
	if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -d "$ICONS" ]; then
		gtk-update-icon-cache -q -t -f "$ICONS" >/dev/null 2>&1 || true
	fi
}

for arg in "$@"; do
	case "$arg" in
		--system) SYSTEM=1 ;;
		--dry-run) DRY_RUN=1 ;;
		--uninstall) UNINSTALL=1 ;;
		-h | --help)
			usage
			exit 0
			;;
		*)
			printf '✗ opção desconhecida: %s\n' "$arg" >&2
			echo 'uso: ./install.sh [--system] [--dry-run] [--uninstall] [--help]' >&2
			exit 1
			;;
	esac
done

if [ "$SYSTEM" -eq 1 ]; then
	PREFIX=/usr/local
	if [ "$DRY_RUN" -eq 0 ] && [ "$(id -u)" -ne 0 ]; then
		die 2 "/usr/local exige root (não uso sudo por conta própria)"
	fi
fi

for cmd in mkdir cp chmod rm; do
	command -v "$cmd" >/dev/null 2>&1 || die 3 "comando ausente: $cmd"
done

BIN="$PREFIX/bin/simion"
DESKTOP="$PREFIX/share/applications/simion.desktop"
ICONS="$PREFIX/share/icons/hicolor"

[ "$DRY_RUN" -eq 1 ] && echo "(dry-run: nada será alterado)"

if [ "$UNINSTALL" -eq 1 ]; then
	removed=0
	for f in "$BIN" "$DESKTOP" \
		"$ICONS/512x512/apps/simion.png" \
		"$ICONS/128x128/apps/simion.png" \
		"$ICONS/64x64/apps/simion.png"; do
		if [ -e "$f" ] || [ -L "$f" ]; then
			step "removendo $f"
			[ "$DRY_RUN" -eq 1 ] || rm -f "$f" || die 2 "não consegui remover $f (permissão?)"
			removed=1
		fi
	done
	if [ "$removed" -eq 0 ]; then
		echo "nada para remover em $PREFIX"
	elif [ "$DRY_RUN" -eq 0 ]; then
		refresh_caches
	fi
	exit 0
fi

# Origem: tudo precisa estar ao lado deste script.
for f in simion simion.desktop simion.png; do
	if [ ! -f "$HERE/$f" ]; then
		printf '✗ instalação incompleta: não encontrei %s em %s\n' "$f" "$HERE" >&2
		echo 'dica: extraia com  tar -xJf simion_<versao>_linux_x86_64.tar.xz  e rode ./install.sh de dentro da pasta' >&2
		exit 1
	fi
done

if command -v install >/dev/null 2>&1; then
	HAVE_INSTALL=1
else
	HAVE_INSTALL=0
fi

# put <modo> <origem> <destino> <descrição>
put() {
	step "$4"
	[ "$DRY_RUN" -eq 1 ] && return 0
	if [ "$HAVE_INSTALL" -eq 1 ]; then
		install -Dm"$1" "$2" "$3" || die 2 "não consegui gravar $3 (permissão?)"
	else
		# BusyBox sem install: rm antes evita "Text file busy" no binário em uso.
		mkdir -p "$(dirname "$3")" &&
			rm -f "$3" &&
			cp "$2" "$3" &&
			chmod "$1" "$3" || die 2 "não consegui gravar $3 (permissão?)"
	fi
}

put 755 "$HERE/simion" "$BIN" "binário em $BIN"
put 644 "$HERE/simion.desktop" "$DESKTOP" "atalho de menu em $DESKTOP"
put 644 "$HERE/simion.png" "$ICONS/512x512/apps/simion.png" "ícone 512x512"
for size in 128 64; do
	if [ -f "$HERE/simion-$size.png" ]; then
		put 644 "$HERE/simion-$size.png" "$ICONS/${size}x$size/apps/simion.png" "ícone ${size}x$size"
	fi
done

[ "$DRY_RUN" -eq 1 ] && exit 0

refresh_caches

version=$("$BIN" --version 2>/dev/null) || version=""
version=${version#Simion }
printf '✓ Simion %s instalado em %s\n' "${version:-(versão desconhecida)}" "$BIN"
printf '  menu: %s\n' "$DESKTOP"
case ":$PATH:" in
	*":$PREFIX/bin:"*) ;;
	*) printf 'aviso: %s/bin não está no PATH; rode:  export PATH="%s/bin:$PATH"\n' "$PREFIX" "$PREFIX" ;;
esac
