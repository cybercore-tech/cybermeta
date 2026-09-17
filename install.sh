#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

# Prefer the cybercore schema cargo target root when unset.
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  SCHEMA_TARGET="$HOME/.cargo-target"
  if [[ -d "$SCHEMA_TARGET" ]]; then
    export CARGO_TARGET_DIR="$SCHEMA_TARGET"
  fi
fi

echo "Building cybermeta (release)..."
cargo build --release

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  BIN_SRC="${CARGO_TARGET_DIR}/release/cybermeta"
elif [[ -f "$HOME/.cargo/config.toml" ]] && grep -q "target-dir" "$HOME/.cargo/config.toml"; then
  TARGET_DIR=$(grep "target-dir" "$HOME/.cargo/config.toml" | sed -E 's/.*=\s*"(.*)"/\1/')
  BIN_SRC="${TARGET_DIR}/release/cybermeta"
else
  BIN_SRC="target/release/cybermeta"
fi

if [[ ! -f "$BIN_SRC" ]]; then
  echo "Could not find built binary at: $BIN_SRC"
  echo "Check your cargo target-dir configuration."
  exit 1
fi

BIN_DST="$HOME/.local/bin/cybermeta"
mkdir -p "$HOME/.local/bin"
cp "$BIN_SRC" "$BIN_DST"
chmod +x "$BIN_DST"

echo "Installed to $BIN_DST"

if command -v cybermeta >/dev/null 2>&1; then
  echo "Done — run 'cybermeta' to start."
else
  echo "Installed, but ~/.local/bin isn't on your \$PATH yet."
  echo "Add this to your shell config, then restart your shell:"
  echo ""
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
