#!/usr/bin/env bash
set -euo pipefail

SCRIPT_SOURCE="${BASH_SOURCE[0]:-}"
if [[ -n "$SCRIPT_SOURCE" ]]; then
  ROOT_DIR="$(cd "$(dirname "$SCRIPT_SOURCE")/../.." && pwd)"
else
  ROOT_DIR="$PWD"
fi
REPO="${AICMD_REPO:-jinzheng8115/aicmd}"
BIN_DIR="${AICMD_INSTALL_BIN_DIR:-$HOME/.local/bin}"
LEGACY_SHARE_DIR="${AICMD_INSTALL_SHARE_DIR:-$HOME/.local/share/aicmd}"
CARGO_BIN="${CARGO:-$HOME/.cargo/bin/cargo}"
FROM_SOURCE=false
NO_SHELL_INTEGRATION=false
VERSION="${AICMD_VERSION:-}"
DEFAULT_VERSION="${AICMD_DEFAULT_VERSION:-v0.30.8}"

usage() {
  cat <<'HELP'
Usage: contrib/aicmd/install.sh [--from-source] [--version vX.Y.Z] [--no-shell-integration]

Default mode downloads the GitHub Release binary and does not require Rust.
Use --from-source for local development builds with cargo.

用法：contrib/aicmd/install.sh [--from-source] [--version vX.Y.Z] [--no-shell-integration]

默认下载 GitHub Release 二进制，不需要 Rust。
本地开发构建请使用 --from-source。
HELP
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --from-source) FROM_SOURCE=true; shift ;;
    --version) VERSION="$2"; shift 2 ;;
    --version=*) VERSION="${1#--version=}"; shift ;;
    --no-shell-integration) NO_SHELL_INTEGRATION=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

install_shell_integration() {
  if [[ "$NO_SHELL_INTEGRATION" == "true" ]]; then
    printf '%s\n' "skipped"
    return
  fi
  local rc_file shell_name marker line
  shell_name="$(basename "${SHELL:-}")"
  case "$shell_name" in
    zsh) rc_file="$HOME/.zshrc" ;;
    bash) rc_file="$HOME/.bashrc" ;;
    *) rc_file="$HOME/.zshrc" ;;
  esac
  marker="# >>> aicmd shell integration >>>"
  line='eval "$(aicmd shell-init)"'
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

default_mcp_config() {
  cat <<'JSON'
{
  "mcp": {
    "servers": {
      "tavily": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "tavily-mcp"],
        "env": {
          "TAVILY_API_KEY": "tvly-xxxx"
        }
      },
      "context7": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "@upstash/context7-mcp"]
      }
    },
    "commands": {
      "search": {
        "description": "Search the web using Tavily",
        "server": "tavily"
      },
      "context7-library": {
        "description": "Resolve a package/library name using Context7",
        "server": "context7"
      }
    }
  }
}
JSON
}

target_triple() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Darwin:arm64|Darwin:aarch64) printf 'aarch64-apple-darwin\n' ;;
    Darwin:x86_64) printf 'x86_64-apple-darwin\n' ;;
    Linux:x86_64) printf 'x86_64-unknown-linux-musl\n' ;;
    Linux:aarch64|Linux:arm64) printf 'aarch64-unknown-linux-musl\n' ;;
    *) echo "Unsupported platform: $os $arch" >&2; exit 2 ;;
  esac
}

latest_version() {
  if [[ -n "$VERSION" ]]; then
    printf '%s\n' "$VERSION"
    return
  fi

  local tag
  tag="$(curl -fsSL -H "User-Agent: aicmd-installer" "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1 || true)"
  if [[ -n "$tag" ]]; then
    printf '%s\n' "$tag"
    return
  fi

  echo "Warning: failed to query latest release from GitHub API; falling back to $DEFAULT_VERSION." >&2
  echo "提示：无法从 GitHub API 获取最新版本，已回退到 ${DEFAULT_VERSION}。" >&2
  printf '%s\n' "$DEFAULT_VERSION"
}

download_binary() {
  local version target archive url tmp
  version="$(latest_version)"
  target="$(target_triple)"
  archive="aicmd-$version-$target.tar.gz"
  url="https://github.com/$REPO/releases/download/$version/$archive"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  echo "Downloading $url"
  curl -fsSL "$url" -o "$tmp/$archive"
  tar -xzf "$tmp/$archive" -C "$tmp"
  install -m 0755 "$tmp/aicmd" "$BIN_DIR/aicmd"
}

build_from_source() {
  if [[ ! -x "$CARGO_BIN" ]]; then
    echo "cargo not found at $CARGO_BIN. Install Rust or set CARGO=/path/to/cargo." >&2
    exit 127
  fi
  (cd "$ROOT_DIR" && "$CARGO_BIN" build --release)
  install -m 0755 "$ROOT_DIR/target/release/aicmd" "$BIN_DIR/aicmd"
}

install_wrapper() {
  local name args path
  name="$1"
  args="$2"
  path="$BIN_DIR/$name"
  cat > "$path" <<SH_WRAPPER
#!/usr/bin/env bash
set -euo pipefail
exec aicmd $args "\$@"
SH_WRAPPER
  chmod 0755 "$path"
}

mkdir -p "$BIN_DIR"
if [[ "$FROM_SOURCE" == "true" ]]; then
  build_from_source
else
  download_binary
fi
install_wrapper aicmd-do do
install_wrapper aicmd-err err
install_wrapper aicmd-model model
install_wrapper aicmd-mcp mcp-raw
install_wrapper aicmd-shell-init shell-init
rm -f "$LEGACY_SHARE_DIR/model-config.example.yaml"
rmdir "$LEGACY_SHARE_DIR" 2>/dev/null || true
SHELL_RC_FILE="$(install_shell_integration)"
CONFIG_DIR="$($BIN_DIR/aicmd model dir)"
MCP_CONFIG_PATH="${AICMD_MCP_CONFIG_FILE:-$CONFIG_DIR/mcp.json}"
mkdir -p "$CONFIG_DIR"
if [[ ! -f "$MCP_CONFIG_PATH" ]]; then
  default_mcp_config > "$MCP_CONFIG_PATH"
  chmod 0600 "$MCP_CONFIG_PATH" 2>/dev/null || true
  MCP_CONFIG_STATUS="Installed MCP config: $MCP_CONFIG_PATH"
else
  MCP_CONFIG_STATUS="Existing MCP config kept: $MCP_CONFIG_PATH. To reset, copy mcp.json manually."
fi

CONFIG_PATH="$($BIN_DIR/aicmd model path)"
CONFIG_STATUS="Existing config kept: $CONFIG_PATH"
if [[ -f "$ROOT_DIR/.env" && ! -f "$CONFIG_PATH" ]]; then
  CONFIG_STATUS="Found $ROOT_DIR/.env. Run: aicmd init --from-env"
elif [[ -f "$ROOT_DIR/.env" && -f "$CONFIG_PATH" ]]; then
  CONFIG_STATUS="Existing config kept: $CONFIG_PATH. To overwrite from .env, run: aicmd init --from-env --force"
elif [[ ! -f "$CONFIG_PATH" ]]; then
  CONFIG_STATUS="No .env found. Copy .env.example to .env, fill it, then run: aicmd init --from-env"
fi

if [[ "$SHELL_RC_FILE" == "skipped" ]]; then
  SHELL_STATUS="  Skipped by --no-shell-integration"
else
  SHELL_STATUS="  Installed into: $SHELL_RC_FILE
  New terminals will load it automatically.
  Current terminal: run source "$SHELL_RC_FILE" once."
fi

cat <<MSG
Installed AICmd to: $BIN_DIR/aicmd
Installed compatibility wrappers:
  $BIN_DIR/aicmd-do
  $BIN_DIR/aicmd-err
  $BIN_DIR/aicmd-model
  $BIN_DIR/aicmd-mcp
  $BIN_DIR/aicmd-shell-init
Shell integration for cd commands:
$SHELL_STATUS

Config:
  $CONFIG_STATUS
  $MCP_CONFIG_STATUS

Make sure $BIN_DIR is in PATH, then run:
  aicmd 列出当前目录最大的 10 个文件
MSG
