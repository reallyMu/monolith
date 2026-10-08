# Convert Default Output Directory Implementation Plan

> **For agentic workers:** Execute task-by-task. Spec: `docs/superpowers/specs/2026-10-06-convert-default-output-dir-design.md`.

**Goal:** Add `defaultConvertOutputDir` setting; convert save dialog prefills `{dir}/{stem}.md` when set, else source-adjacent `.md`.

**Architecture:** Settings JSON field + Rust path resolver (settings-aware) + settings UI row. Dialog still shown (option A).

**Tech Stack:** Tauri 2, Vue 3, Rust settings/convert modules.

## File map

| File | Change |
|------|--------|
| `src-tauri/src/settings.rs` | Field + trim/validate |
| `src-tauri/src/convert.rs` | Settings-aware default path + tests |
| `src/utils/settingsApi.ts` | TS type |
| `src/i18n/messages.ts` | zh/en labels |
| `src/App.vue` | Draft default + settings row |

---

### Task 1: Rust settings + convert path

- [x] Add `default_convert_output_dir` with `#[serde(default)]`
- [x] Trim/validate in `settings_save`
- [x] Pure `resolve_default_output(input, convert_output_dir)` + wire `convert_default_output`
- [x] Unit tests; `cargo test` in `src-tauri`

### Task 2: Frontend settings UI

- [x] Type + i18n + settings row + draft init
- [ ] Smoke: rebuild / install app for manual check

### Task 3: Spec status

- [x] Mark design spec approved / implemented
