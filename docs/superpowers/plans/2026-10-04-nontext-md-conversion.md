# Non-text → Markdown Conversion Implementation Plan

> **For agentic workers:** Implement task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bundle downmark (~8MB) conversion with conversion_log, blocking progress UI, asset source prefill, and Finder reveal for source.

**Architecture:** Rust owns spawn + SQLite `conversion_log` in `assets.sqlite`. Vue shows modal lock during convert and prefills `source_path` from latest successful log by output path. Single tool under `third-party/downmark/`.

**Tech Stack:** Tauri 2, rusqlite, Vue 3, bundled [downmark](https://github.com/giraffesyo/downmark) CLI.

## Global Constraints

- Asset admission unchanged (`file-types.json` only).
- Provenance only on register (`source_path`); log is separate.
- Single tool: all convertible extensions → downmark (no OCR/images).
- Modal block + cancel; no parallel convert jobs.
- No silent fallback to the other tool or treating binary as text.

## File map

| File | Role |
|------|------|
| `src-tauri/src/convert.rs` | Routing, spawn, cancel, log CRUD, Tauri commands |
| `src-tauri/src/assets.rs` | Schema migrate `conversion_log`; register helper lookup; reveal already exists |
| `src-tauri/src/lib.rs` | Register convert commands + menu item |
| `third-party/mineru/`, `third-party/markitdown/` | Bundled tool trees + README |
| `src/utils/convertApi.ts` | Frontend invoke wrappers |
| `src/components/ConvertModal.vue` | Blocking progress UI |
| `src/App.vue` | Intercept unsupported opens/drops; menu; register prefill |
| `src/components/AssetBrowser.vue` | Context menu reveal source |
| `src/i18n/messages.ts` | Copy |
| `src/convert-types.json` | Convertible extensions → tool (SSOT with Rust include_str) |

---

## Task 1: convert-types SSOT + conversion_log schema + unit tests

- [ ] Add `src/convert-types.json` (pdf/images → mineru; office/html/… → markitdown)
- [ ] Migrate `conversion_log` in `assets.rs` init
- [ ] Add `convert.rs`: resolve tool, insert log, latest_success_for_output
- [ ] Cargo tests: routing, latest success, cancelled excluded
- [ ] Wire module in `lib.rs`

## Task 2: spawn + cancel commands

- [ ] Resolve bundled tool path from resource/`third-party`
- [ ] `convert_start` / `convert_cancel` (or single command with cancel token via Mutex)
- [ ] Emit progress events; kill on cancel; write terminal log row
- [ ] Stub/wrapper scripts in `third-party/*/bin` so CI can dry-run without full ML stack when `MONOLITH_CONVERT_STUB=1`

## Task 3: ConvertModal + App entry points

- [ ] `ConvertModal.vue` full-screen modal, progress, cancel
- [ ] App: unsupported open/drop → convert flow (pick output, default same dir)
- [ ] Menu「转换为 Markdown…」
- [ ] On success open MD tab

## Task 4: Register prefill + Finder source

- [ ] Register dialog: `conversion_latest_source(output_path)` prefill
- [ ] AssetBrowser: context menu reveal `source_path`
- [ ] i18n strings

## Task 5: Verify + install

- [ ] `cargo test`, `vue-tsc`, extend `verify-asset-boundaries.sh` or add convert checks
- [ ] Release rebuild/install when tools stubs work end-to-end
