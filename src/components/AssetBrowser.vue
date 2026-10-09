<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "../i18n";
import ToolbarIcon from "./ToolbarIcon.vue";
import { openDialogFilters } from "../utils/language";
import {
  assetListTree,
  assetListVersions,
  assetMove,
  assetUnmount,
  assetRelocate,
  assetRename,
  assetSetCurrentVersion,
  fileNameFromPath,
  indexInvalid,
  parentDirOfPath,
  revealInOs,
  termCreate,
  termDelete,
  termMove,
  termRename,
  type AssetDto,
  type AssetTreeDto,
  type AssetVersionDto,
  type BrowseTermDto,
} from "../utils/assetsApi";
import { matchAssetName, matchFolderName } from "../utils/assetSearch";

const props = defineProps<{
  selectedAssetId: number | null;
}>();

const emit = defineEmits<{
  openAsset: [asset: AssetDto];
  openPath: [path: string];
  selectAsset: [assetId: number | null];
  error: [message: string];
  status: [message: string];
  refreshed: [tree: AssetTreeDto];
  collapse: [];
  requestSaveNewVersion: [asset: AssetDto];
}>();

const { t } = useI18n();
const tree = ref<AssetTreeDto | null>(null);
const dropKey = ref<string | null>(null);
/** `false` = expanded; missing/`true` = collapsed (folders start collapsed). */
const collapsed = ref<Record<number, boolean>>({});
const searchQuery = ref("");
const searchMode = ref<"name" | "fulltext">("name");
const searchInputRef = ref<HTMLInputElement | null>(null);
const searchDebounced = ref("");
let searchTimer: ReturnType<typeof setTimeout> | null = null;

watch(searchQuery, (q) => {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    searchDebounced.value = q;
  }, 150);
});

function focusSearch() {
  searchInputRef.value?.focus();
  searchInputRef.value?.select();
}

function onSearchKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    searchQuery.value = "";
    searchDebounced.value = "";
  }
}

const searchStubActive = computed(
  () => searchMode.value === "fulltext" && searchDebounced.value.trim().length > 0,
);

/** Tauri WKWebView has no usable window.prompt — inline edit instead. */
type Draft =
  | { mode: "create"; parentId: number; name: string }
  | { mode: "rename"; termId: number; name: string }
  | { mode: "rename-asset"; assetId: number; name: string };

const draft = ref<Draft | null>(null);
const draftInputRef = ref<HTMLInputElement | null>(null);
const draftBusy = ref(false);

/**
 * Pointer drag for reparenting (not HTML5 DnD).
 * Tauri native Finder drop needs dragDropEnabled; HTML5 DnD conflicts with it.
 */
type PtrDrag =
  | {
      kind: "asset";
      id: number;
      asset: AssetDto;
      label: string;
      startX: number;
      startY: number;
      active: boolean;
    }
  | {
      kind: "term";
      id: number;
      label: string;
      startX: number;
      startY: number;
      active: boolean;
    };

const ptrDrag = ref<PtrDrag | null>(null);
const ghostPos = ref({ x: 0, y: 0 });
const DRAG_THRESHOLD_PX = 5;
const GHOST_OFFSET = 12;

type Row =
  | { kind: "term"; term: BrowseTermDto; depth: number; key: string }
  | { kind: "asset"; asset: AssetDto; depth: number; key: string }
  | { kind: "draft"; parentId: number; depth: number; key: string };

const rows = computed<Row[]>(() => {
  if (!tree.value) return [];
  const rootId = tree.value.rootTermId;
  const byParent = new Map<number, BrowseTermDto[]>();
  const termById = new Map<number, BrowseTermDto>();
  for (const term of tree.value.terms) {
    termById.set(term.id, term);
    if (term.id === rootId) continue;
    const pid = term.parentId ?? rootId;
    if (!byParent.has(pid)) byParent.set(pid, []);
    byParent.get(pid)!.push(term);
  }
  for (const list of byParent.values()) {
    list.sort((a, b) => a.sortOrder - b.sortOrder || a.id - b.id);
  }

  const q = searchDebounced.value.trim();
  const filtering = searchMode.value === "name" && q.length > 0;
  // Fulltext stub: do not apply name filter; template shows empty hint.
  if (searchMode.value === "fulltext" && q.length > 0) {
    return [];
  }

  let keepAssetIds: Set<number> | null = null;
  let keepTermIds: Set<number> | null = null;
  if (filtering) {
    keepAssetIds = new Set();
    keepTermIds = new Set([rootId]);
    const subtreeKeep = new Set<number>();
    const addAncestors = (start: number | null) => {
      let tid: number | null = start;
      while (tid != null && tid !== rootId) {
        keepTermIds!.add(tid);
        const term = termById.get(tid);
        tid = term?.parentId ?? null;
      }
    };
    const addDescendants = (id: number) => {
      for (const child of byParent.get(id) ?? []) {
        keepTermIds!.add(child.id);
        subtreeKeep.add(child.id);
        addDescendants(child.id);
      }
    };
    for (const term of tree.value.terms) {
      if (term.id === rootId) continue;
      if (!matchFolderName(q, term.displayName)) continue;
      keepTermIds.add(term.id);
      subtreeKeep.add(term.id);
      addAncestors(term.parentId ?? rootId);
      addDescendants(term.id);
    }
    for (const a of tree.value.assets) {
      const inHitFolder = subtreeKeep.has(a.browseTermId);
      if (!inHitFolder && !matchAssetName(q, a.displayName, a.absolutePath)) continue;
      keepAssetIds.add(a.id);
      addAncestors(a.browseTermId);
    }
  }

  const assetsByTerm = new Map<number, AssetDto[]>();
  for (const a of tree.value.assets) {
    if (keepAssetIds && !keepAssetIds.has(a.id)) continue;
    if (!assetsByTerm.has(a.browseTermId)) assetsByTerm.set(a.browseTermId, []);
    assetsByTerm.get(a.browseTermId)!.push(a);
  }
  for (const list of assetsByTerm.values()) {
    list.sort((a, b) => a.displayName.localeCompare(b.displayName));
  }

  const out: Row[] = [];
  function pushDraft(parentId: number, depth: number) {
    if (filtering) return;
    if (draft.value?.mode === "create" && draft.value.parentId === parentId) {
      out.push({ kind: "draft", parentId, depth, key: `draft:${parentId}` });
    }
  }
  function pushAssets(termId: number, depth: number) {
    for (const asset of assetsByTerm.get(termId) ?? []) {
      out.push({ kind: "asset", asset, depth, key: `asset:${asset.id}` });
    }
  }
  function walk(parentId: number, depth: number) {
    for (const term of byParent.get(parentId) ?? []) {
      if (keepTermIds && !keepTermIds.has(term.id)) continue;
      out.push({ kind: "term", term, depth, key: `term:${term.id}` });
      // Default collapsed; only `false` means expanded. Filtering force-shows hits.
      if (!filtering && collapsed.value[term.id] !== false) continue;
      pushDraft(term.id, depth + 1);
      pushAssets(term.id, depth + 1);
      walk(term.id, depth + 1);
    }
  }
  pushDraft(rootId, 0);
  walk(rootId, 0);
  pushAssets(rootId, 0);
  return out;
});

async function refresh() {
  if (draft.value || ptrDrag.value?.active) return;
  try {
    tree.value = await assetListTree();
    emit("refreshed", tree.value);
  } catch (e) {
    emit("error", String(e));
  }
}

defineExpose({ refresh, focusSearch });

let unlistenFocus: (() => void) | null = null;

function onWindowVisible() {
  void refresh();
}

function onDocVisible() {
  if (document.visibilityState === "visible") onWindowVisible();
}

watch(
  () => props.selectedAssetId,
  () => {
    /* selection highlight via class */
  },
);

async function focusDraft() {
  await nextTick();
  const el = draftInputRef.value;
  if (!el) return;
  el.focus();
  el.select();
}

function beginCreate(parentId: number) {
  if (tree.value && parentId !== tree.value.rootTermId) {
    collapsed.value = { ...collapsed.value, [parentId]: false };
  }
  draft.value = { mode: "create", parentId, name: "" };
  void focusDraft();
}

function beginCreateTopLevel() {
  if (!tree.value) return;
  beginCreate(tree.value.rootTermId);
}

function beginRename(term: BrowseTermDto) {
  if (term.code === "root") return;
  draft.value = { mode: "rename", termId: term.id, name: term.displayName };
  void focusDraft();
}

function beginRenameAsset(asset: AssetDto) {
  draft.value = {
    mode: "rename-asset",
    assetId: asset.id,
    name: asset.displayName,
  };
  void focusDraft();
}

function cancelDraft() {
  if (draftBusy.value) return;
  draft.value = null;
}

function isAssetRemarkDraft() {
  return draft.value?.mode === "rename-asset";
}

/** Escape cancels remark edit even when focus is outside the input (e.g. Monaco). */
function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== "Escape" && e.code !== "Escape") return;
  if (!isAssetRemarkDraft() || draftBusy.value) return;
  e.preventDefault();
  e.stopPropagation();
  cancelDraft();
}

/** Click/press outside the remark input abandons edit (does not rely on input focus/blur). */
function onGlobalOutsideRemark(e: Event) {
  if (!isAssetRemarkDraft() || draftBusy.value) return;
  const target = e.target;
  if (!(target instanceof Element)) return;
  // Stay in edit only when interacting with the remark field itself.
  if (target.closest(".ab-input--remark")) return;
  cancelDraft();
}

async function commitDraft() {
  const d = draft.value;
  if (!d || draftBusy.value) return;
  const name = d.name.trim();
  if (!name) {
    draft.value = null;
    return;
  }
  if (d.mode === "rename") {
    const term = tree.value?.terms.find((x) => x.id === d.termId);
    if (term && name === term.displayName) {
      draft.value = null;
      return;
    }
  }
  if (d.mode === "rename-asset") {
    const asset = tree.value?.assets.find((x) => x.id === d.assetId);
    if (asset && name === asset.displayName) {
      draft.value = null;
      return;
    }
  }
  draftBusy.value = true;
  try {
    if (d.mode === "create") await termCreate(d.parentId, name);
    else if (d.mode === "rename") await termRename(d.termId, name);
    else await assetRename(d.assetId, name);
    draft.value = null;
    await refresh();
  } catch (e) {
    emit("error", String(e));
  } finally {
    draftBusy.value = false;
  }
}

function isRenamingAsset(assetId: number) {
  return draft.value?.mode === "rename-asset" && draft.value.assetId === assetId;
}

type CtxMenu = { x: number; y: number; asset: AssetDto };
const ctxMenu = ref<CtxMenu | null>(null);
const historyOpen = ref(false);
const historyAsset = ref<AssetDto | null>(null);
const historyVersions = ref<AssetVersionDto[]>([]);
const historyBusy = ref(false);
const detailsOpen = ref(false);
const detailsAsset = ref<AssetDto | null>(null);

function closeCtxMenu() {
  ctxMenu.value = null;
}

function onAssetContextMenu(e: MouseEvent, asset: AssetDto) {
  e.preventDefault();
  e.stopPropagation();
  emit("selectAsset", asset.id);
  const pad = 8;
  const menuW = 200;
  const menuH = 320;
  let x = e.clientX;
  let y = e.clientY;
  if (x + menuW > window.innerWidth - pad) x = window.innerWidth - menuW - pad;
  if (y + menuH > window.innerHeight - pad) y = window.innerHeight - menuH - pad;
  ctxMenu.value = { x, y, asset };
}

async function ctxOpen() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (a) onAssetClick(a);
}

function ctxRename() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (a) beginRenameAsset(a);
}

async function ctxHistory() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (!a) return;
  historyAsset.value = a;
  historyOpen.value = true;
  historyBusy.value = true;
  try {
    historyVersions.value = await assetListVersions(a.id);
  } catch (e) {
    emit("error", String(e));
    historyOpen.value = false;
  } finally {
    historyBusy.value = false;
  }
}

function ctxDetails() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (!a) return;
  detailsAsset.value = a;
  detailsOpen.value = true;
}

async function ctxReveal() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (!a?.absolutePath) return;
  try {
    await revealInOs(a.absolutePath);
  } catch (e) {
    emit("error", String(e));
  }
}

function isHttpSource(path: string | null | undefined): boolean {
  const p = (path || "").trim();
  return p.startsWith("http://") || p.startsWith("https://");
}

async function ctxRevealSource() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (!a?.sourcePath) {
    emit("error", t("ctxRevealSourceMissing"));
    return;
  }
  try {
    // URL → default browser; local path → Finder (reveal_in_os).
    await revealInOs(a.sourcePath);
  } catch (e) {
    emit("error", String(e));
  }
}

async function ctxCopyPath() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (!a?.absolutePath) return;
  try {
    await navigator.clipboard.writeText(a.absolutePath);
    emit("status", t("pathCopied"));
  } catch (e) {
    emit("error", String(e));
  }
}

async function ctxRelocate() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (a) await relocateAsset(a);
}

function ctxSaveNewVersion() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (a) emit("requestSaveNewVersion", a);
}

async function ctxUnregister() {
  const a = ctxMenu.value?.asset;
  closeCtxMenu();
  if (a) await removeAsset(a);
}

function closeHistory() {
  historyOpen.value = false;
  historyAsset.value = null;
  historyVersions.value = [];
}

function closeDetails() {
  detailsOpen.value = false;
  detailsAsset.value = null;
}

async function openHistoryVersion(ver: AssetVersionDto) {
  emit("openPath", ver.absolutePath);
}

async function setHistoryCurrent(ver: AssetVersionDto) {
  if (!historyAsset.value) return;
  historyBusy.value = true;
  try {
    const updated = await assetSetCurrentVersion(historyAsset.value.id, ver.id);
    historyVersions.value = await assetListVersions(updated.id);
    historyAsset.value = updated;
    await refresh();
    emit("openAsset", updated);
  } catch (e) {
    emit("error", String(e));
  } finally {
    historyBusy.value = false;
  }
}

function teardownPtrListeners() {
  window.removeEventListener("pointermove", onPtrMove);
  window.removeEventListener("pointerup", onPtrUp);
  window.removeEventListener("pointercancel", onPtrUp);
}

onMounted(() => {
  void refresh();
  window.addEventListener("click", closeCtxMenu);
  window.addEventListener("blur", closeCtxMenu);
  window.addEventListener("keydown", onGlobalKeydown, true);
  document.addEventListener("pointerdown", onGlobalOutsideRemark, true);
  document.addEventListener("mousedown", onGlobalOutsideRemark, true);
  document.addEventListener("visibilitychange", onDocVisible);
  void (async () => {
    try {
      unlistenFocus = await getCurrentWindow().onFocusChanged((e) => {
        if (e.payload) onWindowVisible();
      });
    } catch {
      /* not Tauri */
    }
  })();
});

onUnmounted(() => {
  teardownPtrListeners();
  document.documentElement.classList.remove("monolith-asset-dragging");
  window.removeEventListener("click", closeCtxMenu);
  window.removeEventListener("blur", closeCtxMenu);
  window.removeEventListener("keydown", onGlobalKeydown, true);
  document.removeEventListener("pointerdown", onGlobalOutsideRemark, true);
  document.removeEventListener("mousedown", onGlobalOutsideRemark, true);
  document.removeEventListener("visibilitychange", onDocVisible);
  unlistenFocus?.();
  unlistenFocus = null;
});

function hitDropTermId(clientX: number, clientY: number): number | null {
  const el = document.elementFromPoint(clientX, clientY);
  if (!el) return null;
  const termRow = el.closest<HTMLElement>("[data-drop-term-id]");
  if (termRow?.dataset.dropTermId) {
    const id = Number(termRow.dataset.dropTermId);
    return Number.isFinite(id) ? id : null;
  }
  if (el.closest(".ab-tree") && tree.value) {
    return tree.value.rootTermId;
  }
  return null;
}

function startPtrDrag(
  e: PointerEvent,
  payload:
    | { kind: "term"; id: number; label: string }
    | { kind: "asset"; id: number; asset: AssetDto; label: string },
) {
  if (e.button !== 0 || draft.value) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest("button, input, a")) return;
  ptrDrag.value = {
    ...payload,
    startX: e.clientX,
    startY: e.clientY,
    active: false,
  };
  ghostPos.value = { x: e.clientX, y: e.clientY };
  window.addEventListener("pointermove", onPtrMove);
  window.addEventListener("pointerup", onPtrUp);
  window.addEventListener("pointercancel", onPtrUp);
}

function onTermPtrDown(e: PointerEvent, term: BrowseTermDto) {
  if (term.code === "root") return;
  startPtrDrag(e, { kind: "term", id: term.id, label: term.displayName });
}

function onAssetPtrDown(e: PointerEvent, asset: AssetDto) {
  startPtrDrag(e, {
    kind: "asset",
    id: asset.id,
    asset,
    label: asset.displayName,
  });
}

function onPtrMove(e: PointerEvent) {
  const d = ptrDrag.value;
  if (!d) return;
  ghostPos.value = { x: e.clientX, y: e.clientY };
  if (!d.active) {
    const dx = e.clientX - d.startX;
    const dy = e.clientY - d.startY;
    if (dx * dx + dy * dy < DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) return;
    d.active = true;
    ptrDrag.value = { ...d };
    document.documentElement.classList.add("monolith-asset-dragging");
  }
  // Don't treat dropping onto self as a target
  const termId = hitDropTermId(e.clientX, e.clientY);
  if (termId == null || (d.kind === "term" && termId === d.id)) {
    dropKey.value = null;
    return;
  }
  dropKey.value =
    tree.value && termId === tree.value.rootTermId ? "tree:root" : `term:${termId}`;
}

async function onPtrUp(e: PointerEvent) {
  const d = ptrDrag.value;
  teardownPtrListeners();
  ptrDrag.value = null;
  const key = dropKey.value;
  dropKey.value = null;
  document.documentElement.classList.remove("monolith-asset-dragging");
  if (!d) return;

  if (!d.active) {
    if (d.kind === "asset") onAssetClick(d.asset);
    return;
  }

  const termId = hitDropTermId(e.clientX, e.clientY);
  if (termId == null && !key) return;
  const targetId =
    termId ??
    (key === "tree:root" && tree.value
      ? tree.value.rootTermId
      : key?.startsWith("term:")
        ? Number(key.slice(5))
        : null);
  if (targetId == null || !Number.isFinite(targetId)) return;

  try {
    if (d.kind === "term") {
      if (d.id === targetId) return;
      await termMove(d.id, targetId);
    } else {
      // Change folder (not multi-mount mirror): drop from the row's source folder.
      await assetMove(d.id, targetId, d.asset.browseTermId);
    }
    await refresh();
  } catch (err) {
    emit("error", String(err));
  }
}

async function removeFolder(term: BrowseTermDto) {
  if (term.code === "root") return;
  const ok = await ask(t("assetDeleteFolderConfirm", { name: term.displayName }), {
    title: t("assetDeleteFolderTitle"),
    kind: "warning",
  });
  if (!ok) return;
  try {
    await termDelete(term.id);
    await refresh();
  } catch (e) {
    emit("error", String(e));
  }
}

async function removeAsset(asset: AssetDto) {
  const ok = await ask(t("assetUnmountConfirm", { name: asset.displayName }), {
    title: t("assetUnmountTitle"),
    kind: "warning",
  });
  if (!ok) return;
  try {
    const r = await assetUnmount(asset.id, asset.browseTermId);
    if (r.unregistered && props.selectedAssetId === asset.id) emit("selectAsset", null);
    await refresh();
  } catch (e) {
    emit("error", String(e));
  }
}

async function relocateAsset(asset: AssetDto) {
  const selected = await open({
    title: t("assetRelocateTitle"),
    defaultPath: parentDirOfPath(asset.absolutePath),
    multiple: false,
    directory: false,
    filters: openDialogFilters(),
  });
  if (typeof selected !== "string") return;
  try {
    const updated = await assetRelocate(asset.id, selected);
    await refresh();
    emit("openAsset", updated);
  } catch (e) {
    emit("error", String(e));
  }
}

function onAssetClick(asset: AssetDto) {
  emit("selectAsset", asset.id);
  emit("openAsset", asset);
}

function toggle(termId: number) {
  // undefined/true → expand (false); false → collapse (true)
  collapsed.value = {
    ...collapsed.value,
    [termId]: collapsed.value[termId] === false,
  };
}

function isRenaming(termId: number) {
  return draft.value?.mode === "rename" && draft.value.termId === termId;
}
</script>

<template>
  <aside class="asset-browser" :class="{ 'asset-browser--dragging': ptrDrag?.active }">
    <div class="ab-head">
      <button
        type="button"
        class="ab-icon ab-collapse"
        :title="t('collapseAssets')"
        :aria-label="t('collapseAssets')"
        @click="emit('collapse')"
      >
        <ToolbarIcon name="collapseLeft" :size="14" />
      </button>
      <span class="ab-title">{{ t("assets") }}</span>
      <button
        type="button"
        class="ab-icon"
        :title="t('assetRefresh')"
        :aria-label="t('assetRefresh')"
        @click="refresh"
      >
        <ToolbarIcon name="refresh" :size="14" />
      </button>
      <button
        type="button"
        class="ab-icon"
        :title="t('assetNewFolderTop')"
        :aria-label="t('assetNewFolderTop')"
        :disabled="!tree"
        @click="beginCreateTopLevel"
      >
        <ToolbarIcon name="plus" :size="14" />
      </button>
    </div>
    <div class="ab-search">
      <input
        ref="searchInputRef"
        v-model="searchQuery"
        type="search"
        class="ab-search-input"
        :placeholder="t('assetSearchPlaceholder')"
        :aria-label="t('assetSearchPlaceholder')"
        @keydown="onSearchKeydown"
      />
      <select v-model="searchMode" class="ab-search-mode" :aria-label="t('assetSearchMode')">
        <option value="name">{{ t("assetSearchModeName") }}</option>
        <option value="fulltext">{{ t("assetSearchModeFulltext") }}</option>
      </select>
    </div>
    <div v-if="searchStubActive" class="ab-search-stub">{{ t("assetSearchFulltextStub") }}</div>
    <div
      v-else-if="searchMode === 'name' && searchDebounced.trim() && !rows.length"
      class="ab-search-stub"
    >
      {{ t("assetSearchEmpty") }}
    </div>
    <div class="ab-tree" :class="{ 'ab-tree--drop': dropKey === 'tree:root' }">
      <div
        v-for="row in rows"
        :key="row.key"
        class="ab-row"
        :class="{
          'ab-row--term': row.kind === 'term',
          'ab-row--asset': row.kind === 'asset',
          'ab-row--draft': row.kind === 'draft',
          'ab-row--drop': dropKey === row.key,
          'ab-row--selected':
            row.kind === 'asset' && selectedAssetId === row.asset.id,
          'ab-row--missing': row.kind === 'asset' && indexInvalid(row.asset),
          'ab-row--dragging':
            ptrDrag?.active &&
            ((row.kind === 'asset' && ptrDrag.kind === 'asset' && ptrDrag.id === row.asset.id) ||
              (row.kind === 'term' && ptrDrag.kind === 'term' && ptrDrag.id === row.term.id)),
        }"
        :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
        :data-drop-term-id="row.kind === 'term' ? String(row.term.id) : undefined"
      >
        <template v-if="row.kind === 'term'">
          <button type="button" class="ab-twist" @click="toggle(row.term.id)">
            {{ collapsed[row.term.id] !== false ? "▸" : "▾" }}
          </button>
          <div class="ab-main" @pointerdown="onTermPtrDown($event, row.term)">
            <input
              v-if="isRenaming(row.term.id) && draft"
              ref="draftInputRef"
              v-model="draft.name"
              class="ab-input"
              :placeholder="t('assetRenameFolderPrompt')"
              :disabled="draftBusy"
              @keydown.enter.prevent="commitDraft"
              @keydown.escape.prevent="cancelDraft"
              @blur="commitDraft"
            />
            <span v-else class="ab-label">{{ row.term.displayName }}</span>
          </div>
          <button
            type="button"
            class="ab-mini"
            :title="t('assetNewFolder')"
            :aria-label="t('assetNewFolder')"
            @click.stop="beginCreate(row.term.id)"
          >
            <ToolbarIcon name="plus" :size="12" />
          </button>
          <button
            v-if="row.term.code !== 'root'"
            type="button"
            class="ab-mini"
            :title="t('assetRename')"
            :aria-label="t('assetRename')"
            @click.stop="beginRename(row.term)"
          >
            <ToolbarIcon name="pencil" :size="12" />
          </button>
          <button
            v-if="row.term.code !== 'root'"
            type="button"
            class="ab-mini"
            :title="t('assetDeleteFolderTitle')"
            :aria-label="t('assetDeleteFolderTitle')"
            @click.stop="removeFolder(row.term)"
          >
            <ToolbarIcon name="x" :size="12" />
          </button>
        </template>
        <template v-else-if="row.kind === 'draft' && draft?.mode === 'create'">
          <span class="ab-twist-spacer" />
          <div class="ab-main">
            <input
              ref="draftInputRef"
              v-model="draft.name"
              class="ab-input"
              :placeholder="t('assetNewFolderPrompt')"
              :disabled="draftBusy"
              @keydown.enter.prevent="commitDraft"
              @keydown.escape.prevent="cancelDraft"
              @blur="commitDraft"
            />
          </div>
          <button
            type="button"
            class="ab-mini"
            :title="t('confirm')"
            :aria-label="t('confirm')"
            @mousedown.prevent
            @click="commitDraft"
          >
            <ToolbarIcon name="checkSquare" :size="12" />
          </button>
          <button
            type="button"
            class="ab-mini"
            :title="t('cancel')"
            :aria-label="t('cancel')"
            @mousedown.prevent
            @click="cancelDraft"
          >
            <ToolbarIcon name="x" :size="12" />
          </button>
        </template>
        <template v-else-if="row.kind === 'asset'">
          <span class="ab-twist-spacer" />
          <div
            class="ab-main"
            :title="
              indexInvalid(row.asset)
                ? t('assetIndexBrokenTitle')
                : fileNameFromPath(row.asset.absolutePath) || undefined
            "
            @pointerdown="!isRenamingAsset(row.asset.id) && onAssetPtrDown($event, row.asset)"
            @contextmenu="onAssetContextMenu($event, row.asset)"
          >
            <input
              v-if="isRenamingAsset(row.asset.id) && draft"
              ref="draftInputRef"
              v-model="draft.name"
              class="ab-input ab-input--remark"
              data-remark-edit="1"
              :placeholder="t('assetRemarkPrompt')"
              :disabled="draftBusy"
              @keydown.enter.prevent="commitDraft"
              @keydown.escape.prevent="cancelDraft"
              @mousedown.stop
              @pointerdown.stop
            />
            <span v-else class="ab-label">
              {{ row.asset.displayName }}
              <span class="ab-type">.{{ row.asset.fileType }}</span>
              <span v-if="indexInvalid(row.asset)" class="ab-miss">{{ t("assetIndexInvalid") }}</span>
            </span>
          </div>
          <button
            type="button"
            class="ab-mini"
            :title="t('assetRenameRemark')"
            :aria-label="t('assetRenameRemark')"
            @pointerdown.stop
            @mousedown.stop
            @click.stop="beginRenameAsset(row.asset)"
          >
            <ToolbarIcon name="pencil" :size="12" />
          </button>
          <button
            v-if="indexInvalid(row.asset)"
            type="button"
            class="ab-mini"
            :title="t('assetRelocate')"
            :aria-label="t('assetRelocate')"
            @click.stop="relocateAsset(row.asset)"
          >
            <ToolbarIcon name="relocate" :size="12" />
          </button>
        </template>
      </div>
      <div v-if="tree && !tree.assets.length && !draft" class="ab-empty">{{ t("assetEmpty") }}</div>
    </div>

    <Teleport to="body">
      <div
        v-if="ptrDrag?.active"
        class="ab-ghost"
        :class="ptrDrag.kind === 'term' ? 'ab-ghost--term' : 'ab-ghost--asset'"
        :style="{
          left: `${ghostPos.x + GHOST_OFFSET}px`,
          top: `${ghostPos.y + GHOST_OFFSET}px`,
        }"
      >
        <span class="ab-ghost-icon">{{ ptrDrag.kind === "term" ? "▸" : "≡" }}</span>
        <span class="ab-ghost-label">{{ ptrDrag.label }}</span>
      </div>

      <div
        v-if="ctxMenu"
        class="ab-ctx"
        :style="{ left: `${ctxMenu.x}px`, top: `${ctxMenu.y}px` }"
        @click.stop
        @contextmenu.prevent
      >
        <button type="button" @click="ctxOpen">{{ t("ctxOpen") }}</button>
        <button type="button" @click="ctxRename">{{ t("assetRenameRemark") }}</button>
        <button type="button" @click="ctxHistory">{{ t("ctxHistory") }}</button>
        <button type="button" @click="ctxDetails">{{ t("ctxDetails") }}</button>
        <div class="ab-ctx-sep" />
        <button type="button" :disabled="!ctxMenu.asset.absolutePath" @click="ctxReveal">
          {{ t("ctxReveal") }}
        </button>
        <button
          type="button"
          :disabled="!ctxMenu.asset.sourcePath"
          :title="ctxMenu.asset.sourcePath || t('ctxRevealSourceMissing')"
          @click="ctxRevealSource"
        >
          {{
            isHttpSource(ctxMenu.asset.sourcePath)
              ? t("ctxRevealSourceUrl")
              : t("ctxRevealSource")
          }}
        </button>
        <button type="button" :disabled="!ctxMenu.asset.absolutePath" @click="ctxCopyPath">
          {{ t("ctxCopyPath") }}
        </button>
        <button type="button" @click="ctxRelocate">{{ t("assetRelocate") }}</button>
        <div class="ab-ctx-sep" />
        <button type="button" @click="ctxSaveNewVersion">{{ t("saveNewVersion") }}</button>
        <div class="ab-ctx-sep" />
        <button type="button" class="ab-ctx-danger" @click="ctxUnregister">
          {{ t("ctxUnmount") }}
        </button>
      </div>

      <div
        v-if="historyOpen"
        class="ab-modal-backdrop"
        @click.self="closeHistory"
        @keydown.escape.prevent="closeHistory"
      >
        <div class="ab-modal" role="dialog">
          <header class="ab-modal-head">
            <h2>{{ t("historyTitle") }}</h2>
            <button type="button" class="ab-modal-close" @click="closeHistory">×</button>
          </header>
          <p v-if="historyAsset" class="ab-modal-sub">{{ historyAsset.displayName }}</p>
          <div v-if="historyBusy" class="ab-modal-empty">…</div>
          <div v-else-if="!historyVersions.length" class="ab-modal-empty">{{ t("historyEmpty") }}</div>
          <ul v-else class="ab-ver-list">
            <li v-for="ver in historyVersions" :key="ver.id" class="ab-ver-item">
              <div class="ab-ver-meta">
                <span class="ab-ver-name" :title="ver.absolutePath">
                  {{ fileNameFromPath(ver.absolutePath) }}
                </span>
                <span class="ab-ver-time">{{ ver.createdAt }}</span>
                <span v-if="ver.isCurrent" class="ab-ver-badge">{{ t("historyCurrent") }}</span>
                <span v-if="!ver.fileExists" class="ab-ver-miss">{{ t("historyMissing") }}</span>
              </div>
              <div class="ab-ver-actions">
                <button type="button" :disabled="!ver.fileExists" @click="openHistoryVersion(ver)">
                  {{ t("historyOpen") }}
                </button>
                <button
                  type="button"
                  :disabled="ver.isCurrent || !ver.fileExists || historyBusy"
                  @click="setHistoryCurrent(ver)"
                >
                  {{ t("historySetCurrent") }}
                </button>
              </div>
            </li>
          </ul>
        </div>
      </div>

      <div
        v-if="detailsOpen && detailsAsset"
        class="ab-modal-backdrop"
        @click.self="closeDetails"
        @keydown.escape.prevent="closeDetails"
      >
        <div class="ab-modal" role="dialog">
          <header class="ab-modal-head">
            <h2>{{ t("detailsTitle") }}</h2>
            <button type="button" class="ab-modal-close" @click="closeDetails">×</button>
          </header>
          <dl class="ab-details">
            <div>
              <dt>{{ t("assetRemark") }}</dt>
              <dd>{{ detailsAsset.displayName }}</dd>
            </div>
            <div>
              <dt>{{ t("assetFileName") }}</dt>
              <dd>{{ fileNameFromPath(detailsAsset.absolutePath) || "—" }}</dd>
            </div>
            <div>
              <dt>{{ t("detailsPath") }}</dt>
              <dd class="ab-details-path">{{ detailsAsset.absolutePath || "—" }}</dd>
            </div>
            <div>
              <dt>{{ t("detailsType") }}</dt>
              <dd>.{{ detailsAsset.fileType }}</dd>
            </div>
            <div>
              <dt>{{ t("detailsCreated") }}</dt>
              <dd>{{ detailsAsset.createdAt }}</dd>
            </div>
            <div>
              <dt>{{ t("detailsSource") }}</dt>
              <dd class="ab-details-path">
                {{ detailsAsset.sourcePath?.trim() ? detailsAsset.sourcePath : "null" }}
              </dd>
            </div>
            <div v-if="detailsAsset.sourcePath">
              <dt>{{ t("detailsSourceMtime") }}</dt>
              <dd>
                {{
                  detailsAsset.sourceMtime != null
                    ? new Date(detailsAsset.sourceMtime * 1000).toLocaleString()
                    : "—"
                }}
              </dd>
            </div>
            <div v-if="detailsAsset.sourcePath">
              <dt>{{ t("detailsSourceSize") }}</dt>
              <dd>
                {{
                  detailsAsset.sourceSize != null
                    ? `${detailsAsset.sourceSize} B`
                    : "—"
                }}
              </dd>
            </div>
            <div v-if="detailsAsset.sourceStale" class="ab-details-stale">
              <dt>{{ t("detailsSourceStale") }}</dt>
              <dd>{{ t("sourceReconvert") }}</dd>
            </div>
          </dl>
          <div class="ab-modal-foot">
            <button type="button" @click="closeDetails">{{ t("detailsClose") }}</button>
          </div>
        </div>
      </div>
    </Teleport>
  </aside>
</template>

<style scoped>
.asset-browser {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  background: var(--bg-0);
  color: var(--text-2);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  line-height: var(--lh-body);
  -webkit-app-region: no-drag;
  user-select: none;
}
.asset-browser--dragging {
  cursor: grabbing;
}
.ab-collapse {
  font-size: var(--text-icon);
  font-weight: 600;
  margin-right: 2px;
}
.ab-head {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: var(--toolbar-h);
  padding: 0 8px;
  box-sizing: border-box;
  border-bottom: 1px solid var(--hairline);
}
.ab-title {
  flex: 1;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-1);
}
.ab-search {
  display: flex;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--hairline);
  background: var(--bg-1);
}
.ab-search-input {
  flex: 1;
  min-width: 0;
  height: 26px;
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-0);
  color: var(--text-1);
  padding: 0 8px;
  font: inherit;
}
.ab-search-input:focus {
  outline: none;
  border-color: var(--accent);
}
.ab-search-mode {
  flex: 0 0 auto;
  height: 26px;
  max-width: 5.5rem;
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  background: var(--bg-2);
  color: var(--text-2);
  font: inherit;
  font-size: var(--text-xs);
  padding: 0 4px;
}
.ab-search-stub {
  padding: 12px 10px;
  color: var(--text-3);
  font-size: var(--text-xs);
  line-height: var(--lh-body);
  border-bottom: 1px solid var(--hairline);
}
.ab-icon,
.ab-mini,
.ab-twist {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1.4;
  min-width: 22px;
  min-height: 22px;
}
.ab-icon:hover,
.ab-mini:hover,
.ab-twist:hover {
  color: #fff;
}
.ab-icon:disabled,
.ab-mini:disabled {
  opacity: 0.35;
  cursor: default;
}
.ab-tree {
  flex: 1;
  overflow: auto;
  padding: 4px 0 12px;
  min-height: 80px;
}
.ab-tree--drop {
  outline: 1px dashed #4a8fd4;
  outline-offset: -4px;
  background: rgba(36, 48, 68, 0.35);
  box-shadow: inset 0 0 0 1px rgba(74, 143, 212, 0.25);
}
.ab-row {
  display: flex;
  align-items: center;
  gap: 2px;
  min-height: 26px;
  padding-right: 4px;
  transition: background 0.08s ease, box-shadow 0.08s ease;
}
.ab-row--drop {
  background: var(--accent-muted);
  box-shadow: inset 3px 0 0 var(--accent);
}
.ab-row--selected {
  background: var(--accent-muted);
  box-shadow: inset 3px 0 0 var(--accent);
  color: var(--text-1);
}
.ab-row--missing {
  background: #2a2218;
}
.ab-row--missing .ab-label {
  color: #e0a060;
}
.ab-row--missing.ab-row--selected {
  background: #3a2e18;
}
.ab-row--dragging {
  opacity: 0.35;
  outline: 1px dashed #3a4252;
  outline-offset: -1px;
  background: rgba(30, 36, 48, 0.55);
}
.ab-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  cursor: grab;
  padding: 2px 4px;
  border-radius: 3px;
  touch-action: none;
}
.asset-browser--dragging .ab-main {
  cursor: grabbing;
}
.ab-row--draft .ab-main {
  cursor: text;
}
.ab-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ab-input {
  width: 100%;
  min-width: 0;
  height: 22px;
  margin: 0;
  padding: 0 6px;
  border: 1px solid #4a8fd4;
  border-radius: 3px;
  background: #1a2230;
  color: #e8eef8;
  font: inherit;
  outline: none;
  user-select: text;
}
.ab-type {
  margin-left: 4px;
  opacity: 0.45;
  font-size: var(--text-xs);
}
.ab-miss {
  margin-left: 6px;
  padding: 0 5px;
  border: 1px solid #a07830;
  border-radius: 3px;
  color: #f0d090;
  font-size: var(--text-xs);
  font-weight: 650;
}
.ab-twist {
  width: 16px;
  flex: 0 0 16px;
}
.ab-twist-spacer {
  width: 16px;
  flex: 0 0 16px;
}
.ab-empty {
  padding: 12px;
  opacity: 0.5;
}
</style>

<style>
/* global: Teleport ghost + drag cursor (not scoped) */
.monolith-asset-dragging,
.monolith-asset-dragging * {
  cursor: grabbing !important;
}
.ab-ghost {
  position: fixed;
  z-index: 10000;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 220px;
  padding: 5px 10px;
  border-radius: 6px;
  border: 1px solid #4a8fd4;
  background: rgba(26, 34, 48, 0.94);
  color: #e8eef8;
  font-size: var(--text-sm);
  font-weight: 600;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  pointer-events: none;
  transform: translate3d(0, 0, 0) rotate(-1.5deg);
  will-change: left, top;
}
.ab-ghost--term {
  border-color: #6aa3d8;
}
.ab-ghost--asset {
  border-color: #4a8fd4;
}
.ab-ghost-icon {
  opacity: 0.7;
  flex: 0 0 auto;
}
.ab-ghost-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ab-ctx {
  position: fixed;
  z-index: 10001;
  min-width: 188px;
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--hairline);
  background: var(--bg-1);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.ab-ctx button {
  border: none;
  background: transparent;
  color: #d6d8de;
  text-align: left;
  padding: 7px 10px;
  border-radius: 5px;
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}
.ab-ctx button:hover:not(:disabled) {
  background: #2a3448;
  color: #fff;
}
.ab-ctx button:disabled {
  opacity: 0.35;
  cursor: default;
}
.ab-ctx-sep {
  height: 1px;
  margin: 3px 6px;
  background: var(--hairline);
}
.ab-ctx-danger {
  color: #e08080 !important;
}

.ab-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10002;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(8, 10, 14, 0.55);
}
.ab-modal {
  width: min(480px, calc(100% - 40px));
  max-height: min(70vh, 560px);
  overflow: auto;
  padding: 14px 16px 12px;
  border-radius: 10px;
  border: 1px solid var(--hairline);
  background: var(--bg-1);
  color: var(--text-1);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  line-height: var(--lh-body);
}
.ab-modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.ab-modal-head h2 {
  margin: 0;
  font-size: var(--text-title);
  font-weight: var(--fw-semibold);
  line-height: var(--lh-tight);
  color: #e8eef8;
}
.ab-modal-close {
  border: none;
  background: transparent;
  color: #8b93a7;
  font-size: var(--text-icon);
  cursor: pointer;
  line-height: 1;
}
.ab-modal-sub {
  margin: 6px 0 12px;
  font-size: var(--text-sm);
  color: #8b93a7;
}
.ab-modal-empty {
  padding: 20px 0;
  text-align: center;
  opacity: 0.5;
  font-size: var(--text-sm);
}
.ab-ver-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.ab-ver-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--bg-0);
  border: 1px solid var(--hairline);
}
.ab-ver-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 10px;
  align-items: baseline;
  font-size: var(--text-sm);
}
.ab-ver-name {
  font-weight: var(--fw-semibold);
  color: #e8eef8;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}
.ab-ver-time {
  color: #8b93a7;
  font-size: var(--text-xs);
}
.ab-ver-badge {
  font-size: var(--text-xs);
  padding: 1px 6px;
  border-radius: 999px;
  background: #2a4a72;
  color: #cfe0f8;
}
.ab-ver-miss {
  color: #e0a060;
  font-size: var(--text-xs);
}
.ab-ver-actions {
  display: flex;
  gap: 6px;
}
.ab-ver-actions button,
.ab-modal-foot button {
  height: 26px;
  padding: 0 10px;
  border: 1px solid #3a4252;
  border-radius: 5px;
  background: #222733;
  color: #d6d8de;
  font: inherit;
  font-size: var(--text-sm);
  font-weight: var(--fw-medium);
  cursor: pointer;
}
.ab-ver-actions button:disabled {
  opacity: 0.4;
  cursor: default;
}
.ab-details {
  margin: 12px 0 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-size: var(--text-sm);
}
.ab-details dt {
  color: #8b93a7;
  margin-bottom: 2px;
  font-size: var(--text-sm);
}
.ab-details dd {
  margin: 0;
  color: #e8eef8;
  word-break: break-all;
  font-size: var(--text-sm);
}
.ab-details-path {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: #c8ced8;
}
.ab-modal-foot {
  display: flex;
  justify-content: flex-end;
  margin-top: 14px;
}
</style>
