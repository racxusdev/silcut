#!/usr/bin/env bash
# install.sh — instala o silcut no sistema para o usuário atual (SEM AppImage).
#
#   ./install.sh              → instala em ~/.local/bin + menu de aplicativos
#   ./install.sh --uninstall  → remove tudo
#
# A regra udev (permissão USB) é copiada se você rodar com sudo;
# sem sudo o script informa o comando para rodar depois.

set -euo pipefail
cd "$(dirname "$0")"

BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/256x256/apps"

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "$BIN_DIR/silcut" "$BIN_DIR/silcut-gui" \
          "$DESKTOP_DIR/silcut.desktop" \
          "$ICON_DIR/silcut.png" \
          "$HOME/.config/silcut/70-silhouette.rules"
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    echo "silcut removido (config do usuário em ~/.config/silcut foi mantida)."
    exit 0
fi

# 1) Compilar se os binários não existem
if [[ ! -x target/release/silcut-gui ]]; then
    echo "== compilando (release) =="
    cargo build --release
fi

# 2) Copiar binários
mkdir -p "$BIN_DIR"
install -m 755 target/release/silcut      "$BIN_DIR/silcut"
install -m 755 target/release/silcut-gui  "$BIN_DIR/silcut-gui"

# 3) Ícone
mkdir -p "$ICON_DIR"
if [[ -f icon.png ]]; then
    cp icon.png "$ICON_DIR/silcut.png"
else
    python3 - "$ICON_DIR/silcut.png" <<'EOF'
import struct, zlib, sys
w = h = 256
rows = b''
for y in range(h):
    rows += b'\x00'
    for x in range(w):
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

# 4) Entrada no menu de aplicativos
mkdir -p "$DESKTOP_DIR"
cat > "$DESKTOP_DIR/silcut.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=silcut
GenericName=Corte Silhouette/Graphtec
Comment=Corte vetorial nativo para Silhouette no Linux (GPGL, USB direto)
Exec=$BIN_DIR/silcut-gui
TryExec=$BIN_DIR/silcut-gui
Icon=silcut
Categories=Graphics;Engineering;
StartupWMClass=silcut
EOF
update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

# 5) Regra udev — precisa de root
if [[ "$(id -u)" -eq 0 ]]; then
    install -m 644 70-silhouette.rules /etc/udev/rules.d/
    udevadm control --reload && udevadm trigger || true
    echo "Regra udev instalada em /etc/udev/rules.d/."
else
    install -m 644 70-silhouette.rules "$HOME/.config/silcut/70-silhouette.rules" 2>/dev/null || {
        mkdir -p "$HOME/.config/silcut"
        install -m 644 70-silhouette.rules "$HOME/.config/silcut/70-silhouette.rules"
    }
    echo "⚠ Permissão USB: rode UMA vez (vai pedir senha):"
    echo "    sudo cp ~/.config/silcut/70-silhouette.rules /etc/udev/rules.d/ && sudo udevadm control --reload && sudo udevadm trigger"
fi

echo
echo "Instalado!"
echo "  Terminal : silcut-gui        (GUI — reabra o terminal antes, ou 'source ~/.profile')"
echo "             silcut --help     (CLI)"
echo "  Menu     : procure 'silcut' no menu de aplicativos"
