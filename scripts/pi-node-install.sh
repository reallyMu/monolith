#!/usr/bin/env bash
# Fetch official Node ≥22.19 and keep only the runtime binary for Monolith bundling.
# Output: third-party/node/bin/node[.exe] + LICENSE
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/third-party/node"
NODE_VER="${MONOLITH_NODE_VERSION:-22.19.0}"
BASE="https://nodejs.org/dist/v${NODE_VER}"

arch_triple() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os" in
    darwin)
      case "$arch" in
        arm64|aarch64) echo "darwin-arm64" ;;
        x86_64) echo "darwin-x64" ;;
        *) echo "error: unsupported mac arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    linux)
      case "$arch" in
        x86_64) echo "linux-x64" ;;
        aarch64|arm64) echo "linux-arm64" ;;
        *) echo "error: unsupported linux arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    mingw*|msys*|cygwin*)
      echo "win-x64"
      ;;
    *)
      # Windows Git Bash / CI often reports MINGW64_NT-...
      if [[ "${OS:-}" == "Windows_NT" ]] || [[ -n "${WINDIR:-}" ]]; then
        echo "win-x64"
      else
        echo "error: unsupported OS: $os" >&2
        exit 1
      fi
      ;;
  esac
}

# Allow CI override: MONOLITH_NODE_TRIPLE=win-x64|darwin-arm64|...
TRIPLE="${MONOLITH_NODE_TRIPLE:-$(arch_triple)}"
CACHE="${TMPDIR:-/tmp}/monolith-node-v${NODE_VER}-${TRIPLE}"
mkdir -p "$OUT/bin" "$CACHE"

BIN_NAME="node"
EXT="tar.gz"
if [[ "$TRIPLE" == win-* ]]; then
  BIN_NAME="node.exe"
  EXT="zip"
fi

STAMP="$OUT/bin/.node-v${NODE_VER}-${TRIPLE}"
if [[ -f "$OUT/bin/$BIN_NAME" && -f "$STAMP" ]]; then
  echo "ok: bundled node already present ($("$OUT/bin/$BIN_NAME" -v 2>/dev/null || echo unknown))"
  exit 0
fi

ARCHIVE="node-v${NODE_VER}-${TRIPLE}.${EXT}"
URL="${BASE}/${ARCHIVE}"
echo "fetch: $URL"
curl -fsSL -o "$CACHE/$ARCHIVE" "$URL"

rm -rf "$CACHE/extract"
mkdir -p "$CACHE/extract"
if [[ "$EXT" == "zip" ]]; then
  unzip -q "$CACHE/$ARCHIVE" -d "$CACHE/extract"
else
  tar -xzf "$CACHE/$ARCHIVE" -C "$CACHE/extract"
fi

SRC_DIR="$CACHE/extract/node-v${NODE_VER}-${TRIPLE}"
if [[ ! -d "$SRC_DIR" ]]; then
  echo "error: expected extract dir missing: $SRC_DIR" >&2
  exit 1
fi

rm -f "$OUT/bin/node" "$OUT/bin/node.exe" "$OUT/bin"/'.node-v'*
cp "$SRC_DIR/bin/$BIN_NAME" "$OUT/bin/$BIN_NAME" 2>/dev/null \
  || cp "$SRC_DIR/$BIN_NAME" "$OUT/bin/$BIN_NAME"
chmod +x "$OUT/bin/$BIN_NAME"
if [[ -f "$SRC_DIR/LICENSE" ]]; then
  cp "$SRC_DIR/LICENSE" "$OUT/LICENSE"
fi
printf '%s\n' "$NODE_VER" "$TRIPLE" >"$STAMP"

VER="$("$OUT/bin/$BIN_NAME" -v)"
echo "ok: installed $VER → $OUT/bin/$BIN_NAME"
