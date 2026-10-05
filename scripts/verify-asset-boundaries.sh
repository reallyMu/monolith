#!/usr/bin/env bash
# Feature-boundary verification for local document assets (design 2026-10-04).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "== vue-tsc =="
cd "$ROOT"
npx vue-tsc --noEmit

echo "== cargo test (asset + convert boundaries) =="
cd "$ROOT/src-tauri"
cargo test --lib -- --nocapture

echo "== design DoD checklist (automated coverage) =="
cat <<'EOF'
[auto] L2 admission: file_type_admits_monaco_ext_and_special_name
[auto] L4 path unique + L7 delete keeps file: create_rejects_duplicate_path_and_keeps_file_on_delete
[auto] L6 version naming + collision: new_version_path_uses_timestamp_and_collision_suffix
[auto] L8 no cycle on term move: term_move_rejects_into_descendant
[auto] depth cap: term_depth_caps_at_max
[auto] source_path null: source_path_null_when_absent
[auto] set current version: set_current_switches_and_lists_flags
[auto] L9 missing file + source stale flags: list_marks_missing_file_and_source_flags
[auto] L9 relocate rename/move: relocate_accepts_rename_and_move
[auto] L9 relocate UNIQUE + unsupported type: relocate_rejects_path_owned_by_other_asset_and_unsupported_type
[auto] L9 relocate appends conversion_log: relocate_appends_conversion_log_without_rewriting_history
[auto] C4 all convertible → downmark; images/unknown rejected
[auto] C3 latest success ignores failed/cancelled; append history
[auto] C3 relocate append-only + chain resolve: relocate_appends_log_and_resolves_via_chain
[auto] stale enrich: enrich_marks_stale_when_size_or_mtime_changes
[auto] default output same-dir stem.md
[manual] UI: convert modal block + cancel
[manual] UI: broken-index dialog (relocate / delete / rebuild)
[manual] UI: open registered path after external rename → dialog (not status-bar only)
[manual] UI: source stale ask on open
[manual] UI: register prefills source from conversion_log
[manual] UI: context menu reveal source (Finder / browser for URL)
[manual] UI: 资产登记 remark + source; Ctrl+S no new version row
EOF

echo "OK: automated boundary tests passed"
