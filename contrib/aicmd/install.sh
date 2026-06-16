#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN_DIR="${AICMD_INSTALL_BIN_DIR:-$HOME/.local/bin}"
LEGACY_SHARE_DIR="${AICMD_INSTALL_SHARE_DIR:-$HOME/.local/share/aicmd}"
CARGO_BIN="${CARGO:-$HOME/.cargo/bin/cargo}"

mkdir -p "$BIN_DIR"

if [[ ! -x "$CARGO_BIN" ]]; then
  echo "cargo not found at $CARGO_BIN. Install Rust or set CARGO=/path/to/cargo." >&2
  exit 127
fi

(cd "$ROOT_DIR" && "$CARGO_BIN" build --release)
install -m 0755 "$ROOT_DIR/target/release/aicmd" "$BIN_DIR/aicmd"
install -m 0755 "$ROOT_DIR/contrib/aicmd/bin/aicmd-do" "$BIN_DIR/aicmd-do"
install -m 0755 "$ROOT_DIR/contrib/aicmd/bin/aicmd-err" "$BIN_DIR/aicmd-err"
install -m 0755 "$ROOT_DIR/contrib/aicmd/bin/aicmd-model" "$BIN_DIR/aicmd-model"
rm -f "$LEGACY_SHARE_DIR/model-config.example.yaml"
rmdir "$LEGACY_SHARE_DIR" 2>/dev/null || true

cat <<MSG
Installed AICmd to: $BIN_DIR/aicmd
Installed helpers:
  $BIN_DIR/aicmd-do
  $BIN_DIR/aicmd-err
  $BIN_DIR/aicmd-model
Make sure $BIN_DIR is in PATH, then run:
  aicmd 列出当前目录最大的 10 个文件
MSG
