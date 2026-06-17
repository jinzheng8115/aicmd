#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN_DIR="${AICMD_INSTALL_BIN_DIR:-$HOME/.local/bin}"
LEGACY_SHARE_DIR="${AICMD_INSTALL_SHARE_DIR:-$HOME/.local/share/aicmd}"
CARGO_BIN="${CARGO:-$HOME/.cargo/bin/cargo}"

install_shell_integration() {
  local rc_file shell_name marker line
  shell_name="$(basename "${SHELL:-}")"
  case "$shell_name" in
    zsh) rc_file="$HOME/.zshrc" ;;
    bash) rc_file="$HOME/.bashrc" ;;
    *) rc_file="$HOME/.zshrc" ;;
  esac
  marker="# >>> aicmd shell integration >>>"
  line='eval "$(aicmd-shell-init)"'
  mkdir -p "$(dirname "$rc_file")"
  touch "$rc_file"
  if grep -Fq "$marker" "$rc_file" || grep -Fxq "$line" "$rc_file"; then
    printf '%s\n' "$rc_file"
    return
  fi
  {
    printf '\n%s\n' "$marker"
    printf '%s\n' "$line"
    printf '%s\n' "# <<< aicmd shell integration <<<"
  } >> "$rc_file"
  printf '%s\n' "$rc_file"
}

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
install -m 0755 "$ROOT_DIR/contrib/aicmd/bin/aicmd-mcp" "$BIN_DIR/aicmd-mcp"
install -m 0755 "$ROOT_DIR/contrib/aicmd/bin/aicmd-shell-init" "$BIN_DIR/aicmd-shell-init"
rm -f "$LEGACY_SHARE_DIR/model-config.example.yaml"
rmdir "$LEGACY_SHARE_DIR" 2>/dev/null || true
SHELL_RC_FILE="$(install_shell_integration)"

CONFIG_PATH="$("$BIN_DIR/aicmd-model" path)"
CONFIG_STATUS="Existing config kept: $CONFIG_PATH"
if [[ -f "$ROOT_DIR/.env" && ! -f "$CONFIG_PATH" ]]; then
  CONFIG_STATUS="Found $ROOT_DIR/.env. Run: aicmd-model init --from-env"
elif [[ -f "$ROOT_DIR/.env" && -f "$CONFIG_PATH" ]]; then
  CONFIG_STATUS="Existing config kept: $CONFIG_PATH. To overwrite from .env, run: aicmd-model init --from-env --force"
elif [[ ! -f "$CONFIG_PATH" ]]; then
  CONFIG_STATUS="No .env found. Copy .env.example to .env, fill it, then run: aicmd-model init --from-env"
fi

cat <<MSG
Installed AICmd to: $BIN_DIR/aicmd
Installed helpers:
  $BIN_DIR/aicmd-do
  $BIN_DIR/aicmd-err
  $BIN_DIR/aicmd-model
  $BIN_DIR/aicmd-mcp
  $BIN_DIR/aicmd-shell-init
Shell integration for cd commands:
  Installed into: $SHELL_RC_FILE
  New terminals will load it automatically.
  Current terminal: run source "$SHELL_RC_FILE" once.

Config:
  $CONFIG_STATUS

Make sure $BIN_DIR is in PATH, then run:
  aicmd 列出当前目录最大的 10 个文件
MSG
