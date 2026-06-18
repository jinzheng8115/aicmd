#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_SH="$ROOT_DIR/contrib/aicmd/install.sh"
CARGO_BIN="${CARGO:-}"
if [[ -z "$CARGO_BIN" ]]; then
  if command -v cargo >/dev/null 2>&1; then
    CARGO_BIN="$(command -v cargo)"
  elif [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    CARGO_BIN="$HOME/.cargo/bin/cargo"
  else
    echo "cargo not found" >&2
    exit 1
  fi
fi

echo "[verify] install.sh syntax"
bash -n "$INSTALL_SH"

echo "[verify] install.sh fallback does not expand DEFAULT_VERSION incorrectly"
tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT
mkdir -p "$tmp/bin" "$tmp/install-bin"
cat > "$tmp/bin/curl" <<'FAKE_CURL'
#!/usr/bin/env bash
set -euo pipefail
for arg in "$@"; do
  if [[ "$arg" == *"api.github.com"* ]]; then
    exit 22
  fi
done
out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [[ -n "$out" ]]; then
  printf 'not-a-real-archive' > "$out"
fi
exit 0
FAKE_CURL
chmod +x "$tmp/bin/curl"
set +e
PATH="$tmp/bin:$PATH" AICMD_INSTALL_BIN_DIR="$tmp/install-bin" AICMD_DEFAULT_VERSION="v9.9.9" \
  bash "$INSTALL_SH" > "$tmp/stdout" 2> "$tmp/stderr"
status=$?
set -e
if [[ "$status" -eq 0 ]]; then
  echo "expected installer to fail after fake archive download" >&2
  exit 1
fi
grep -F "falling back to v9.9.9" "$tmp/stderr" >/dev/null
grep -F "回退到 v9.9.9" "$tmp/stderr" >/dev/null
if grep -F "unbound variable" "$tmp/stderr" >/dev/null; then
  cat "$tmp/stderr" >&2
  exit 1
fi

echo "[verify] aicmd update dry-run uses versioned installer URL"
version="$($CARGO_BIN metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
dry_run="$($CARGO_BIN run --quiet -- update --dry-run)"
printf '%s\n' "$dry_run"
grep -F "/v${version}/contrib/aicmd/install.sh" <<< "$dry_run" >/dev/null
if grep -F "/main/contrib/aicmd/install.sh" <<< "$dry_run" >/dev/null; then
  echo "update dry-run should not use main installer URL" >&2
  exit 1
fi

echo "[verify] ok"
