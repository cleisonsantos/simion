# Instalar o Simion (tarball Linux)

## 1. Extrair

O tarball não tem pasta raiz: extraia dentro de uma pasta nova.

```sh
mkdir simion && cd simion
tar -xJf ../simion_<versão>_linux_x86_64.tar.xz
```

## 2. Instalar

Rode o instalador **de dentro da pasta extraída**:

```sh
./install.sh              # instala em ~/.local, sem root
./install.sh --help       # todas as opções
./install.sh --dry-run    # mostra o que faria, sem alterar nada
./install.sh --uninstall  # remove o Simion
sudo ./install.sh --system  # instala em /usr/local
```

O `install.sh` vive dentro do tarball; não existe `install.sh` solto na release.

## 3. Qual arquivo usar

| Distro | Arquivo |
| --- | --- |
| Arch / CachyOS / Manjaro | `.tar.xz` (este) ou `.AppImage` — **não** use `.deb`/`.rpm` |
| Debian / Ubuntu | `.deb` |
| Fedora / RHEL / openSUSE | `.rpm` |
| Qualquer distro | `.AppImage` ou `.tar.xz` |
