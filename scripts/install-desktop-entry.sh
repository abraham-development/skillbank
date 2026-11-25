#!/usr/bin/env bash
# Instala la app de escritorio para tu usuario (sin sudo) y la hace aparecer en
# "Mostrar aplicaciones" de Ubuntu con su ícono. Útil antes de tener el paquete .deb.
#
# Uso:  scripts/install-desktop-entry.sh               # compila (release) e instala
#       scripts/install-desktop-entry.sh --uninstall   # quita la app y su entrada
#
# Siempre compila con `pnpm tauri build`: un `cargo build --release` a secas genera un binario
# de desarrollo que busca la interfaz en http://localhost:1420 y no funciona por sí solo.
set -euo pipefail

cd "$(dirname "$0")/.."
APP_ID="banco-de-skills"                      # = nombre del binario = WM_CLASS / app_id en Wayland
BIN_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
ICONS_DIR="$HOME/.local/share/icons/hicolor"
DESKTOP="$APPS_DIR/$APP_ID.desktop"

refresh() {
  command -v update-desktop-database >/dev/null && update-desktop-database -q "$APPS_DIR" || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t -f "$ICONS_DIR" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$BIN_DIR/$APP_ID" "$DESKTOP"
  for s in 32x32 64x64 128x128 256x256 512x512; do rm -f "$ICONS_DIR/$s/apps/$APP_ID.png"; done
  refresh
  echo "✓ Banco de Skills (app de escritorio) desinstalada. El banco y el CLI no se tocan."
  exit 0
fi

[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
echo "→ Compilando la app (release, con la interfaz incluida)…"
pnpm tauri build --no-bundle

SRC_BIN="target/release/$APP_ID"
[ -x "$SRC_BIN" ] || { echo "✗ No existe $SRC_BIN; compila primero."; exit 1; }
# El binario debe llevar la interfaz dentro (si no, buscaría un servidor en localhost:1420).
JS_ASSET=$(ls dist/assets/*.js 2>/dev/null | head -1 | xargs -r basename)
if [ -z "$JS_ASSET" ] || ! grep -aq "$JS_ASSET" "$SRC_BIN"; then
  echo "✗ $SRC_BIN no incluye la interfaz (¿se compiló con cargo en vez de pnpm tauri build?)."
  exit 1
fi

echo "→ Instalando en ~/.local …"
install -Dm755 "$SRC_BIN" "$BIN_DIR/$APP_ID"
install -Dm644 src-tauri/icons/32x32.png      "$ICONS_DIR/32x32/apps/$APP_ID.png"
install -Dm644 src-tauri/icons/64x64.png      "$ICONS_DIR/64x64/apps/$APP_ID.png"
install -Dm644 src-tauri/icons/128x128.png    "$ICONS_DIR/128x128/apps/$APP_ID.png"
install -Dm644 src-tauri/icons/128x128@2x.png "$ICONS_DIR/256x256/apps/$APP_ID.png"
install -Dm644 src-tauri/icons/icon.png       "$ICONS_DIR/512x512/apps/$APP_ID.png"

mkdir -p "$APPS_DIR"
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Banco de Skills
GenericName=Banco de Agent Skills
Comment=Guarda y reutiliza tus Agent Skills entre proyectos y agentes
Exec=$BIN_DIR/$APP_ID
Icon=$APP_ID
Terminal=false
Categories=Development;
Keywords=skills;agent;claude;codex;cursor;antigravity;agentes;ia;
StartupWMClass=$APP_ID
StartupNotify=true
EOF
chmod 644 "$DESKTOP"
command -v desktop-file-validate >/dev/null && desktop-file-validate "$DESKTOP" || true
refresh
echo "✓ Listo: busca \"Banco de Skills\" en Mostrar aplicaciones (puedes anclarla al dock)."
