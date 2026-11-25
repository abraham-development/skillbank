#!/usr/bin/env bash
# Compila e instala el CLI `skillbank` en ~/.local/bin (que ya está en el PATH de Ubuntu)
# y actualiza la skill guía en los agentes instalados.
#
# Uso:  scripts/install-cli.sh            # instala y actualiza la guía
#       scripts/install-cli.sh --no-guide # solo instala el binario
set -euo pipefail

cd "$(dirname "$0")/.."
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

echo "→ Compilando e instalando skillbank en ~/.local/bin …"
cargo install --path crates/skillbank-cli --locked --root "$HOME/.local" --force --quiet

BIN="$HOME/.local/bin/skillbank"
"$BIN" --version

if [ "${1:-}" != "--no-guide" ]; then
  echo "→ Instalando/actualizando la skill guía en los agentes detectados …"
  "$BIN" agents install-guide
fi

case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) echo "⚠ ~/.local/bin no está en tu PATH. Añade a ~/.bashrc:  export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac
echo "✓ Listo. Prueba:  skillbank --help"
