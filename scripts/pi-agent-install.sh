#!/usr/bin/env bash
# Install Pi Bridge deps for Monolith.
# Default: vendor latest earendil-works/pi @ main (npm registry may lag).
# Fallback: PI_FROM_NPM=1 uses package.json registry version only.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRIDGE="$ROOT/third-party/pi-agent/bridge"
SRC="$ROOT/third-party/pi-agent/source/pi"
PI_REPO="${PI_REPO:-https://github.com/earendil-works/pi.git}"
PI_REF="${PI_REF:-main}"

# Prefer bundled Node when present
if [[ -x "$ROOT/third-party/node/bin/node" ]]; then
  export PATH="$ROOT/third-party/node/bin:$PATH"
fi

need_node() {
  if ! command -v node >/dev/null 2>&1; then
    echo "error: Node.js not found. Run: npm run pi-node:install" >&2
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

# Offline / repeat builds: keep existing bridge if already wired (set PI_FORCE=1 to re-vendor).
if [[ "${PI_FORCE:-}" != "1" ]] && [[ -f "$BRIDGE/node_modules/@earendil-works/pi-coding-agent/dist/index.js" ]]; then
  echo "ok: pi-agent already present in $BRIDGE/node_modules (PI_FORCE=1 to re-vendor)"
  exit 0
fi

if [[ "${PI_FROM_NPM:-}" == "1" ]]; then
  cd "$BRIDGE"
  if [[ -f package-lock.json ]]; then npm ci; else npm install; fi
  echo "ok: pi-agent from npm in $BRIDGE/node_modules"
  exit 0
fi

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "error: missing $1" >&2; exit 1; }
}
need_cmd git
need_cmd npm

echo "=== vendor pi @ ${PI_REF} ==="
if [[ -d "$SRC/.git" ]]; then
  cd "$SRC"
  git fetch --depth 1 origin "$PI_REF"
  git checkout -f FETCH_HEAD
else
  rm -rf "$SRC"
  mkdir -p "$(dirname "$SRC")"
  git clone --depth 1 --branch "$PI_REF" "$PI_REPO" "$SRC"
  cd "$SRC"
fi
SHA="$(git rev-parse --short HEAD)"
echo "HEAD=$SHA $(git log -1 --format=%s)"

echo "=== npm install (pi monorepo) ==="
npm install --ignore-scripts

echo "=== npm run build ==="
npm run build
test -f "$SRC/packages/coding-agent/dist/index.js"

echo "=== pack workspace packages into bridge/ ==="
rm -f "$BRIDGE"/earendil-works-*.tgz
# Package set drifts on pi main — skip dirs that disappeared (e.g. protocol).
DIRS=(chord ai agent mcp tui codemode coding-agent durable env protocol telemetry)
for d in "${DIRS[@]}"; do
  if [[ ! -d "$SRC/packages/$d" ]]; then
    echo "skip missing package dir: packages/$d"
    continue
  fi
  (cd "$SRC/packages/$d" && npm pack --pack-destination "$BRIDGE" >/dev/null)
done
test -n "$(ls -A "$BRIDGE"/earendil-works-pi-coding-agent-*.tgz 2>/dev/null)" \
  || { echo "error: pi-coding-agent tarball missing after pack" >&2; exit 1; }

python3 - <<PY
import json, re
from pathlib import Path
bridge = Path("$BRIDGE")
overrides = {}
for t in sorted(bridge.glob("earendil-works-*.tgz")):
    m = re.match(r"(earendil-works-.+)-(\d+\.\d+\.\d+(?:-[^.]*)?)\.tgz\$", t.name)
    if not m:
        raise SystemExit(f"bad tarball name {t.name}")
    scoped = "@earendil-works/" + m.group(1).removeprefix("earendil-works-")
    overrides[scoped] = f"file:{t.name}"
pkg = {
    "name": "monolith-pi-bridge",
    "private": True,
    "version": "0.1.0",
    "type": "module",
    "engines": {"node": ">=22.19.0"},
    "dependencies": {
        "@earendil-works/pi-coding-agent": overrides["@earendil-works/pi-coding-agent"],
    },
    "overrides": overrides,
    "piSource": {
        "repo": "earendil-works/pi",
        "ref": "$PI_REF",
        "sha": "$SHA",
    },
}
(bridge / "package.json").write_text(json.dumps(pkg, indent=2) + "\n")
print("wrote package.json overrides for", len(overrides), "packages sha=$SHA")
PY

cd "$BRIDGE"
rm -rf node_modules package-lock.json
npm install
test -f node_modules/@earendil-works/pi-coding-agent/dist/index.js
echo "ok: pi-agent vendored from ${PI_REF} ($SHA) → $BRIDGE/node_modules"
