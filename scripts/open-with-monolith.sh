#!/bin/bash
# Finder「打开方式」冷启动未公证 App 会被 Gatekeeper 拦（弹窗常写文档名）。
# 本脚本先清隔离属性，再用 open -a 打开（终端路径通常不再弹窗）。
set -euo pipefail
APP="/Applications/Monolith.app"
if [[ ! -d "$APP" ]]; then
  echo "Monolith not installed at $APP" >&2
  exit 1
fi
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
for f in "$@"; do
  [[ -e "$f" ]] || continue
  xattr -d com.apple.quarantine "$f" 2>/dev/null || true
done
if [[ $# -eq 0 ]]; then
  open -a "$APP"
else
  open -a "$APP" "$@"
fi
