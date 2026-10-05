# Monolith Precision UI + Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship precision-instrument visual tokens, denser toolbars, asset-tree name search (fulltext stub), MCP `asset_search`, and shortcut bindings + settings cheat sheet per `docs/superpowers/specs/2026-10-05-monolith-precision-ui-design.md`.

**Architecture:** CSS design tokens drive chrome; AssetBrowser filters the loaded tree client-side; Domain `search_assets_by_name` backs MCP `asset_search`; App-level keydown map + Settings「快捷键」tab for discovery.

**Tech Stack:** Vue 3 + Vite, Tauri 2, Rust (`assets.rs` / `mcp_server.rs`), Monaco, TipTap.

## Global Constraints

- Accent = ice blue; no purple glow / glass / big gradients.
- Fulltext = stub only (UI + MCP); never silently fall back to name search.
- ⌘U underline = unsupported in cheat sheet; do not add underline mark.
- ⌘K focuses asset tree search; ⌘E toggles `edit` ↔ `preview`.
- MCP tools call Domain only (no ad-hoc SQL in `mcp_server.rs`).
- macOS primary; accelerators use `CmdOrCtrl`.

## File map

| File | Responsibility |
|------|----------------|
| `src/styles.css` | Design tokens + base surfaces |
| `src/App.vue` | Toolbars, settings shortcuts tab, global shortcuts, Monaco/preview bg |
| `src/components/AssetBrowser.vue` | Search box, mode, filter, focus API |
| `src/utils/assetSearch.ts` | Shared name-match helper (TS) |
| `src/i18n/messages.ts` | zh/en copy |
| `src-tauri/src/assets.rs` | `search_assets_by_name` Domain |
| `src-tauri/src/mcp_server.rs` | `asset_search` tool |
| `skills/monolith-assets/SKILL.md` | Agent docs for search |

---

### Task 1: Design tokens + chrome recolor (P0)

**Files:**
- Modify: `src/styles.css`
- Modify: `src/App.vue` (replace hardcoded `#12151a` / `#171b22` / `#323846` / `#4a8fd4` in chrome CSS with `var(--*)` where practical)

**Interfaces:**
- Produces: CSS vars `--bg-0`, `--bg-1`, `--bg-2`, `--hairline`, `--text-1`, `--text-2`, `--text-3`, `--accent`, `--accent-muted`, `--danger`, `--ok`, `--radius`

- [ ] **Step 1:** Add token block to `:root` in `styles.css` (ice blue accent ~`#5eb0ff`, deep bg `#0e1116` / `#151a22` / `#1c2330`).
- [ ] **Step 2:** Point `html, body, #app` and shared modal helpers at tokens.
- [ ] **Step 3:** In `App.vue` scoped toolbar/settings/modal CSS, swap the highest-traffic hex colors to tokens (toolbar, settings-nav active, register-primary).
- [ ] **Step 4:** Align Monaco / preview pane default backgrounds to `--bg-0`/`--bg-1` (initial `editBg`/`previewBg` defaults or CSS on panes).
- [ ] **Step 5:** Visual check via `tauri dev`; commit.

```bash
git add src/styles.css src/App.vue
git commit -m "style: add precision-instrument design tokens"
```

---

### Task 2: Toolbar density (P0)

**Files:**
- Modify: `src/App.vue` (toolbar-file / toolbar-format markup + CSS)
- Modify: `src/i18n/messages.ts` (more-format labels if needed)

**Interfaces:**
- Consumes: tokens from Task 1

- [ ] **Step 1:** File toolbar: enforce single row groups File | Assets | View | Settings; move color pickers into settings「目录」or a small View overflow (prefer settings About/dirs secondary).
- [ ] **Step 2:** Format toolbar: default compact row (heading, size, B, I, link, code); wrap strike/table/task/etc. behind `formatMore` toggle.
- [ ] **Step 3:** Unified toolbar height ~36px; hairline separators; icon title strings include ⌘ hints for keys that already exist.
- [ ] **Step 4:** Smoke in narrow width; commit.

```bash
git add src/App.vue src/i18n/messages.ts
git commit -m "ui: densify file/format toolbars"
```

---

### Task 3: Asset tree name search + fulltext stub (P1)

**Files:**
- Create: `src/utils/assetSearch.ts`
- Modify: `src/components/AssetBrowser.vue`
- Modify: `src/i18n/messages.ts`
- Modify: `src/App.vue` (⌘K → `assetBrowserRef.focusSearch()`)

**Interfaces:**
- Produces: `matchAssetName(query, displayName, absolutePath): boolean`
- Produces: `AssetBrowser` expose `{ refresh, focusSearch }`
- Produces: `searchMode: 'name' | 'fulltext'`, `searchQuery` in AssetBrowser

- [ ] **Step 1:** Add `assetSearch.ts`:

```ts
export function basenameOf(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

export function matchAssetName(query: string, displayName: string, absolutePath: string | null | undefined): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  if (displayName.toLowerCase().includes(q)) return true;
  if (absolutePath && basenameOf(absolutePath).toLowerCase().includes(q)) return true;
  return false;
}
```

- [ ] **Step 2:** AssetBrowser header: input + mode control; debounce 150ms; Esc clears.
- [ ] **Step 3:** When `mode==='name'` and query non-empty, filter flat walk: keep matching assets + ancestor terms; when `mode==='fulltext'` and query non-empty, show stub empty state (do not filter as name).
- [ ] **Step 4:** `defineExpose({ refresh, focusSearch })`; App ⌘K calls `focusSearch`.
- [ ] **Step 5:** Manual test 3 names; commit.

```bash
git add src/utils/assetSearch.ts src/components/AssetBrowser.vue src/i18n/messages.ts src/App.vue
git commit -m "feat: asset tree name search with fulltext stub"
```

---

### Task 4: Domain + MCP `asset_search` (P1)

**Files:**
- Modify: `src-tauri/src/assets.rs`
- Modify: `src-tauri/src/mcp_server.rs`
- Modify: `skills/monolith-assets/SKILL.md`
- Test: unit test in `assets.rs` `#[cfg(test)]`

**Interfaces:**
- Produces: `pub fn search_assets_by_name(conn, query, folder_scope, recursive) -> Result<Vec<AssetDto>, String>`
- Produces: MCP tool `asset_search` args `{ query, mode?, folder?, browse_term_id?, recursive? }`

- [ ] **Step 1:** Write failing Rust test: insert two assets, search substring of display_name → 1 hit; basename hit → 1 hit.
- [ ] **Step 2:** Implement `search_assets_by_name` using existing list helpers + case-insensitive contains on display_name and path basename; honor folder via `resolve_browse_term` + `list_assets_in_folder` / recursive walk already used elsewhere.
- [ ] **Step 3:** Register `asset_search` in `mcp_server.rs`: `mode=fulltext` → JSON `{ "implemented": false, "message": "全文检索尚未实现" }` (or English consistent with other MCP messages); `mode=name` → call Domain.
- [ ] **Step 4:** Update SKILL.md search section.
- [ ] **Step 5:** `cargo test` + stdio tools/list smoke; commit.

```bash
cargo test -p monolith search_assets -- --nocapture
git add src-tauri/src/assets.rs src-tauri/src/mcp_server.rs skills/monolith-assets/SKILL.md
git commit -m "feat(mcp): add asset_search name mode with fulltext stub"
```

---

### Task 5: Shortcuts + cheat sheet (P2)

**Files:**
- Modify: `src/App.vue` (window keydown map, settings tab `shortcuts`)
- Modify: `src/i18n/messages.ts`
- Modify: `src-tauri/src/lib.rs` only if new menu items needed (optional)

**Interfaces:**
- Consumes: format actions already on App (`toggleBold`, etc.), `focusSearch`, `setViewMode`
- Produces: settings tab listing all keys from spec §6.2

- [ ] **Step 1:** Central `onAppKeydown` (capture): map ⌘B/I/D/L, ⇧⌘K, ⇧⌘T, ⌘E, ⌘K; skip when target is input/textarea/contenteditable except when intentional (⌘S already elsewhere).
- [ ] **Step 2:** ⌘E toggles active tab `edit` ↔ `preview`.
- [ ] **Step 3:** Settings nav + pane「快捷键」readonly table; ⌘U row marked unsupported.
- [ ] **Step 4:** Manual shortcut pass on MD file; commit.

```bash
git add src/App.vue src/i18n/messages.ts
git commit -m "feat: markdown shortcuts and settings cheat sheet"
```

---

### Task 6: Release verify

- [ ] `npm run tauri build`
- [ ] Install `/Applications/Monolith.app`
- [ ] Checklist against spec §8
- [ ] Sync Skill to Agents via settings if MCP binary changed

---

## Spec coverage check

| Spec | Task |
|------|------|
| §3 tokens / editor | T1 |
| §4 toolbars | T2 |
| §5 tree search | T3 |
| §5b MCP search | T4 |
| §6 shortcuts | T5 |
| §8 acceptance | T6 |
