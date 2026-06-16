#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${AICMD_INSTALL_BIN_DIR:-$HOME/.local/bin}"
AICHAT_DIR="${AICHAT_CONFIG_DIR:-$HOME/Library/Application Support/aichat}"
FUNCTIONS_DIR="$AICHAT_DIR/functions"

mkdir -p "$BIN_DIR" "$FUNCTIONS_DIR/tools" "$AICHAT_DIR/roles"
install -m 0755 "$ROOT_DIR/bin/aicmd" "$BIN_DIR/aicmd"
install -m 0755 "$ROOT_DIR/bin/aicmd-mem" "$BIN_DIR/aicmd-mem"
install -m 0755 "$ROOT_DIR/bin/aicmd-mem-search" "$BIN_DIR/aicmd-mem-search"
install -m 0755 "$ROOT_DIR/bin/aicmd-err" "$BIN_DIR/aicmd-err"
install -m 0755 "$ROOT_DIR/bin/aicmd-do" "$BIN_DIR/aicmd-do"
install -m 0644 "$ROOT_DIR/functions/tools/tavily_mcp_search.mjs" "$FUNCTIONS_DIR/tools/tavily_mcp_search.mjs"
install -m 0644 "$ROOT_DIR/roles/auto.md" "$AICHAT_DIR/roles/auto.md"

if command -v npm >/dev/null 2>&1; then
  cp "$ROOT_DIR/functions/tools/package.json" "$FUNCTIONS_DIR/tools/package.json"
  (cd "$FUNCTIONS_DIR/tools" && npm install --omit=dev >/dev/null)
else
  echo "npm not found. Install Node.js/npm, then run: cd '$FUNCTIONS_DIR/tools' && npm install --omit=dev" >&2
fi

cat <<MSG
Installed aicmd helpers to: $BIN_DIR
Installed aichat role/tool files to: $AICHAT_DIR

Make sure $BIN_DIR is in PATH, then run:
  aicmd hello
MSG
