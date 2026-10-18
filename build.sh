#!/usr/bin/env bash
# build.sh — compila o silcut em modo release e empacota.
#
#   ./build.sh              → binários em target/release/ + pacote em dist/
#   ./build.sh --fast       → pula LTO/otimizações extras (compila mais rápido)
#
# O pacote preferido é AppImage (se appimagetool estiver disponível);
# caso contrário gera um tar.gz portátil com o mesmo conteúdo.

set -euo pipefail
cd "$(dirname "$0")"

FAST=0
[[ "${1:-}" == "--fast" ]] && FAST=1

VERSION=$(cargo metadata --no-deps --format-version 1 2>/dev/null | \
    python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
APP=dist/silcut-${VERSION}-x86_64-linux
APPDIR=${APP}.AppDir

echo "== cargo build (release) =="
if [[ $FAST -eq 1 ]]; then
    cargo build --release --fast
else
    cargo build --release
fi

rm -rf dist
mkdir -p "${APPDIR}/usr/bin" "${APPDIR}/usr/share/applications" "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

cp target/release/silcut      "${APPDIR}/usr/bin/"
cp target/release/silcut-gui  "${APPDIR}/usr/bin/"
cp 70-silhouette.rules        "${APPDIR}/usr/bin/" 2>/dev/null || true
cp README.md                  "${APPDIR}/usr/bin/" 2>/dev/null || true

# Ícone: usa o logo se existir, senão um quadrado azul gerado.
ICON="${APPDIR}/usr/share/icons/hicolor/256x256/apps/silcut.png"
if [[ -f icon.png ]]; then
    cp icon.png "$ICON"
else
    python3 - "$ICON" <<'EOF'
import struct, zlib, sys
w = h = 256
rows = b''
for y in range(h):
    rows += b'\x00'
    for x in range(w):
        # quadrado com canto "cortado" (tesoura estilizada)
        inside = 40 < x < 216 and 40 < y < 216 and not (x > 180 and y < 76)
        rows += (b'\x50\xa0\xff' if inside else b'\x18\x1a\x1f')
def chunk(t, d):
    c = t + d
    return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c))
png = b'\x89PNG\r\n\x1a\n'
png += chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
png += chunk(b'IDAT', zlib.compress(rows))
png += chunk(b'IEND', b'')
open(sys.argv[1], 'wb').write(png)
EOF
fi

# CORREÇÃO 1: Copiar o ícone gerado para a raiz do AppDir (Exigência do appimagetool)
cp "$ICON" "${APPDIR}/silcut.png"

# CORREÇÃO 2: Criar o link simbólico AppRun na raiz apontando para o executável GUI
ln -s usr/bin/silcut-gui "${APPDIR}/AppRun"

cat > "${APPDIR}/usr/share/applications/silcut.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=silcut
GenericName=Corte Silhouette/Graphtec
Comment=Corte vetorial nativo para Silhouette no Linux (GPGL, USB direto)
Exec=silcut-gui
Icon=silcut
Categories=Graphics;Engineering;
EOF

cat > "${APPDIR}/silcut.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=silcut
Exec=silcut-gui
Icon=silcut
Categories=Graphics;Engineering;
EOF

# AppRun já é symlink para o binário (CORREÇÃO 2 acima) — não sobrescrever!
ln -sf usr/share/icons/hicolor/256x256/apps/silcut.png "${APPDIR}/.DirIcon" 2>/dev/null || true

echo "== empacotamento =="
if command -v linuxdeploy >/dev/null 2>&1; then
    # linuxdeploy EMBUTE as bibliotecas (GL, xcb, xkbcommon…) no AppImage,
    # deixando-o autocontido — sem precisar instalar nada no sistema alvo.
    echo "linuxdeploy encontrado: embutindo bibliotecas do sistema"
    ARCH=x86_64 linuxdeploy \
        --appdir "${APPDIR}" \
        --executable "${APPDIR}/usr/bin/silcut-gui" \
        --desktop-file "${APPDIR}/usr/share/applications/silcut.desktop" \
        --icon-file "${ICON}" \
        --output appimage
    # o linuxdeploy gera o AppImage no diretório atual
    shopt -s nullglob
    for f in silcut*.AppImage; do
        mv "$f" "${APP}.AppImage"
    done
    echo "AppImage autocontido: ${APP}.AppImage"
elif command -v appimagetool >/dev/null 2>&1; then
    ARCH=x86_64 appimagetool "${APPDIR}" "${APP}.AppImage"
    echo "AppImage: ${APP}.AppImage"
    echo "  aviso: sem linuxdeploy as bibliotecas NÃO foram embutidas;"
    echo "  no sistema alvo podem faltar libgl1/libxkbcommon/xcb (instale o linuxdeploy"
    echo "  para um AppImage 100 % autocontido)."
else
    tar -czf "${APP}.tar.gz" -C "$(dirname "$APPDIR")" "$(basename "$APPDIR")"
    echo "linuxdeploy/appimagetool não encontrados — gerado tar.gz: ${APP}.tar.gz"
    echo "  para AppImage autocontido: baixe linuxdeploy (github.com/linuxdeploy) e rode ./build.sh de novo"
    echo "  (udev: sudo cp 70-silhouette.rules /etc/udev/rules.d/)"
fi
echo "Pronto. Binários soltos em target/release/."

