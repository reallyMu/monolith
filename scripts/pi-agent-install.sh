#!/usr/bin/env bash
# Install npm deps for Monolith's bundled Pi Bridge.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRIDGE="$ROOT/third-party/pi-agent/bridge"

need_node() {
  if ! command -v node >/dev/null 2>&1; then
    echo "error: Node.js not found. Install Node ≥ 22.19 (https://nodejs.org/)." >&2
    exit 1
  fi
  local major minor
  major="$(node -p "process.versions.node.split('.')[0]")"
  minor="$(node -p "process.versions.node.split('.')[1]")"
  if [[ "$major" -lt 22 ]] || { [[ "$major" -eq 22 ]] && [[ "$minor" -lt 19 ]]; }; then
    echo "error: Node $(node -v) is too old; need ≥ 22.19.0" >&2
    exit 1
  fi
}

need_node
cd "$BRIDGE"
if [[ -f package-lock.json ]]; then
  npm ci
else
  npm install
fi
echo "ok: pi-agent bridge deps in $BRIDGE/node_modules"
