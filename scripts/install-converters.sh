#!/usr/bin/env bash
# Fetch the small downmark binary into third-party/downmark/ (~8MB).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/third-party/downmark"
VER="${DOWNMARK_VERSION:-0.10.0}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$ARCH" in
  arm64|aarch64) ARCH=arm64 ;;
  x86_64|amd64) ARCH=amd64 ;;
  *) echo "unsupported arch: $ARCH" >&2; exit 1 ;;
esac
# Git Bash / MSYS on Windows
case "$OS" in
  darwin|linux) EXT=tar.gz; ARCHIVE_OS="$OS" ;;
  mingw*|msys*|cygwin*|windows*) EXT=zip; ARCHIVE_OS=windows; ARCH=amd64 ;;
  *) echo "unsupported os: $OS" >&2; exit 1 ;;
esac

URL="https://github.com/giraffesyo/downmark/releases/download/v${VER}/downmark_${VER}_${ARCHIVE_OS}_${ARCH}.${EXT}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $URL"
curl -fsSL "$URL" -o "$TMP/dm.${EXT}"
mkdir -p "$DEST"
if [[ "$EXT" == "zip" ]]; then
  unzip -qo "$TMP/dm.${EXT}" -d "$TMP"
  BIN="$(find "$TMP" -type f \( -name downmark.exe -o -name downmark \) | head -1)"
else
  tar -xzf "$TMP/dm.${EXT}" -C "$TMP"
  BIN="$(find "$TMP" -type f -name downmark | head -1)"
fi
[[ -n "$BIN" ]] || { echo "downmark binary missing in archive" >&2; exit 1; }

# Bundle name is always `downmark` (Windows: keep .exe beside a no-ext copy for Tauri resource key).
cp "$BIN" "$DEST/downmark"
if [[ "$BIN" == *.exe ]]; then
  cp "$BIN" "$DEST/downmark.exe"
fi
chmod +x "$DEST/downmark" "$DEST/run" 2>/dev/null || true
if [[ -x "$DEST/downmark" ]] || [[ -f "$DEST/downmark.exe" ]]; then
  ("$DEST/downmark" -version || "$DEST/downmark.exe" -version) 2>/dev/null || true
fi
echo "OK → $DEST/downmark ($(du -h "$DEST/downmark" | awk '{print $1}'))"
