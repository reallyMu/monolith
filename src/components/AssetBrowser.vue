<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "../i18n";
import {
  assetDelete,
  assetListTree,
  assetMove,
  assetRelocate,
  termCreate,
  termDelete,
  termMove,
  termRename,
  type AssetDto,
  type AssetTreeDto,
  type BrowseTermDto,
} from "../utils/assetsApi";

const props = defineProps<{
  selectedAssetId: number | null;
}>();

const emit = defineEmits<{
  openAsset: [asset: AssetDto];
  selectAsset: [assetId: number | null];
  error: [message: string];
  refreshed: [tree: AssetTreeDto];
  collapse: [];
}>();

const { t } = useI18n();
const tree = ref<AssetTreeDto | null>(null);
const dropKey = ref<string | null>(null);
const collapsed = ref<Record<number, boolean>>({});

type Row =
  | { kind: "term"; term: BrowseTermDto; depth: number; key: string }
  | { kind: "asset"; asset: AssetDto; depth: number; key: string };

const rows = computed<Row[]>(() => {
  if (!tree.value) return [];
  const byParent = new Map<number | null, BrowseTermDto[]>();
  for (const term of tree.value.terms) {
    const pid = term.parentId;
    if (!byParent.has(pid)) byParent.set(pid, []);
    byParent.get(pid)!.push(term);
  }
  for (const list of byParent.values()) {
    list.sort((a, b) => a.sortOrder - b.sortOrder || a.id - b.id);
  }
  const assetsByTerm = new Map<number, AssetDto[]>();
  for (const a of tree.value.assets) {
    if (!assetsByTerm.has(a.browseTermId)) assetsByTerm.set(a.browseTermId, []);
    assetsByTerm.get(a.browseTermId)!.push(a);
  }
  for (const list of assetsByTerm.values()) {
    list.sort((a, b) => a.displayName.localeCompare(b.displayName));
  }

  const out: Row[] = [];
  function walk(parentId: number | null, depth: number) {
    for (const term of byParent.get(parentId) ?? []) {
      out.push({ kind: "term", term, depth, key: `term:${term.id}` });
      if (collapsed.value[term.id]) continue;
      for (const asset of assetsByTerm.get(term.id) ?? []) {
        out.push({ kind: "asset", asset, depth: depth + 1, key: `asset:${asset.id}` });
      }
      walk(term.id, depth + 1);
    }
  }
  walk(null, 0);
  return out;
});

async function refresh() {
  try {
    tree.value = await assetListTree();
    emit("refreshed", tree.value);
  } catch (e) {
    emit("error", String(e));
  }
}

defineExpose({ refresh });

onMounted(() => {
  void refresh();
});

watch(
  () => props.selectedAssetId,
  () => {
    /* selection highlight via class */
  },
);

function onTermDragStart(e: DragEvent, termId: number, code: string) {
  if (code === "root") {
    e.preventDefault();
    return;
  }
  e.dataTransfer?.setData("text/browse-term-id", String(termId));
  e.dataTransfer?.setData("text/plain", `browse-term:${termId}`);
  e.dataTransfer!.effectAllowed = "move";
}

function onAssetDragStart(e: DragEvent, assetId: number) {
  e.dataTransfer?.setData("text/asset-id", String(assetId));
  e.dataTransfer?.setData("text/plain", `asset:${assetId}`);
  e.dataTransfer!.effectAllowed = "move";
}

function parseTermId(dt: DataTransfer | null): number | null {
  if (!dt) return null;
  const raw = dt.getData("text/browse-term-id")?.trim();
  if (raw && Number.isFinite(Number(raw))) return Number(raw);
  const m = /^browse-term:(\d+)$/.exec(dt.getData("text/plain")?.trim() ?? "");
  return m ? Number(m[1]) : null;
}

function parseAssetId(dt: DataTransfer | null): number | null {
  if (!dt) return null;
  const raw = dt.getData("text/asset-id")?.trim();
  if (raw && Number.isFinite(Number(raw))) return Number(raw);
  const m = /^asset:(\d+)$/.exec(dt.getData("text/plain")?.trim() ?? "");
  return m ? Number(m[1]) : null;
}

function onTermDragOver(e: DragEvent, key: string) {
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  dropKey.value = key;
}

function onDragLeave(key: string) {
  if (dropKey.value === key) dropKey.value = null;
}

async function onTermDrop(e: DragEvent, termId: number) {
  e.preventDefault();
  dropKey.value = null;
  const movingTerm = parseTermId(e.dataTransfer);
  if (movingTerm != null) {
    if (movingTerm === termId) return;
    try {
      await termMove(movingTerm, termId);
      await refresh();
    } catch (err) {
      emit("error", String(err));
    }
    return;
  }
  const movingAsset = parseAssetId(e.dataTransfer);
  if (movingAsset != null) {
    try {
      await assetMove(movingAsset, termId);
      await refresh();
    } catch (err) {
      emit("error", String(err));
    }
  }
}

async function addFolder(parentId: number) {
  const name = window.prompt(t("assetNewFolderPrompt"));
  if (!name?.trim()) return;
  try {
    await termCreate(parentId, name.trim());
    await refresh();
  } catch (e) {
    emit("error", String(e));
  }
}

async function renameFolder(term: BrowseTermDto) {
  if (term.code === "root") return;
  const name = window.prompt(t("assetRenameFolderPrompt"), term.displayName);
  if (!name?.trim() || name.trim() === term.displayName) return;
  try {
    await termRename(term.id, name.trim());
    await refresh();
  } catch (e) {
    emit("error", String(e));
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
  const ok = await ask(t("assetDeleteConfirm", { name: asset.displayName }), {
    title: t("assetDeleteTitle"),
    kind: "warning",
  });
  if (!ok) return;
  try {
    await assetDelete(asset.id);
    if (props.selectedAssetId === asset.id) emit("selectAsset", null);
    await refresh();
  } catch (e) {
    emit("error", String(e));
  }
}

async function relocateAsset(asset: AssetDto) {
  const selected = await open({
    multiple: false,
    directory: false,
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
  if (!asset.fileExists) {
    emit("error", t("assetMissing", { path: asset.absolutePath ?? "" }));
    return;
  }
  emit("openAsset", asset);
}

function toggle(termId: number) {
  collapsed.value = { ...collapsed.value, [termId]: !collapsed.value[termId] };
}
</script>

<template>
  <aside class="asset-browser">
    <div class="ab-head">
      <button
        type="button"
        class="ab-icon ab-collapse"
        :title="t('collapseAssets')"
        @click="emit('collapse')"
      >
        ‹
      </button>
      <span class="ab-title">{{ t("assets") }}</span>
      <button type="button" class="ab-icon" :title="t('assetRefresh')" @click="refresh">↻</button>
      <button
        type="button"
        class="ab-icon"
        :title="t('assetNewFolder')"
        @click="tree && addFolder(tree.rootTermId)"
      >
        +
      </button>
    </div>
    <div class="ab-tree">
      <div
        v-for="row in rows"
        :key="row.key"
        class="ab-row"
        :class="{
          'ab-row--term': row.kind === 'term',
          'ab-row--asset': row.kind === 'asset',
          'ab-row--drop': dropKey === row.key,
          'ab-row--selected':
            row.kind === 'asset' && selectedAssetId === row.asset.id,
          'ab-row--missing': row.kind === 'asset' && !row.asset.fileExists,
        }"
        :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
      >
        <template v-if="row.kind === 'term'">
          <button type="button" class="ab-twist" @click="toggle(row.term.id)">
            {{ collapsed[row.term.id] ? "▸" : "▾" }}
          </button>
          <div
            class="ab-main"
            draggable="true"
            @dragstart="onTermDragStart($event, row.term.id, row.term.code)"
            @dragover="onTermDragOver($event, row.key)"
            @dragleave="onDragLeave(row.key)"
            @drop="onTermDrop($event, row.term.id)"
          >
            <span class="ab-label">{{ row.term.displayName }}</span>
          </div>
          <button
            type="button"
            class="ab-mini"
            :title="t('assetNewFolder')"
            @click="addFolder(row.term.id)"
          >
            +
          </button>
          <button
            v-if="row.term.code !== 'root'"
            type="button"
            class="ab-mini"
            :title="t('assetRename')"
            @click="renameFolder(row.term)"
          >
            ✎
          </button>
          <button
            v-if="row.term.code !== 'root'"
            type="button"
            class="ab-mini"
            :title="t('assetDeleteFolderTitle')"
            @click="removeFolder(row.term)"
          >
            ×
          </button>
        </template>
        <template v-else>
          <span class="ab-twist-spacer" />
          <div
            class="ab-main"
            draggable="true"
            @dragstart="onAssetDragStart($event, row.asset.id)"
            @click="onAssetClick(row.asset)"
          >
            <span class="ab-label">
              {{ row.asset.displayName }}
              <span class="ab-type">.{{ row.asset.fileType }}</span>
              <span v-if="!row.asset.fileExists" class="ab-miss">!</span>
            </span>
          </div>
          <button
            v-if="!row.asset.fileExists"
            type="button"
            class="ab-mini"
            :title="t('assetRelocate')"
            @click="relocateAsset(row.asset)"
          >
            ↗
          </button>
          <button
            type="button"
            class="ab-mini"
            :title="t('assetDeleteTitle')"
            @click="removeAsset(row.asset)"
          >
            ×
          </button>
        </template>
      </div>
      <div v-if="tree && !tree.assets.length" class="ab-empty">{{ t("assetEmpty") }}</div>
    </div>
  </aside>
</template>

<style scoped>
.asset-browser {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  background: #12151a;
  color: #c8ced8;
  font-size: 12px;
}
.ab-collapse {
  font-size: 16px;
  font-weight: 600;
  margin-right: 2px;
}
.ab-head {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid #2a2f38;
}
.ab-title {
  flex: 1;
  font-weight: 600;
  letter-spacing: 0.02em;
}
.ab-icon,
.ab-mini,
.ab-twist {
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1.4;
}
.ab-icon:hover,
.ab-mini:hover,
.ab-twist:hover {
  color: #fff;
}
.ab-tree {
  flex: 1;
  overflow: auto;
  padding: 4px 0 12px;
}
.ab-row {
  display: flex;
  align-items: center;
  gap: 2px;
  min-height: 26px;
  padding-right: 4px;
}
.ab-row--drop {
  background: #243044;
}
.ab-row--selected {
  background: #1e3a5f;
}
.ab-row--missing .ab-label {
  color: #e0a060;
}
.ab-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  cursor: grab;
  padding: 2px 4px;
  border-radius: 3px;
}
.ab-row--asset .ab-main {
  cursor: pointer;
}
.ab-row--asset .ab-main[draggable="true"] {
  cursor: grab;
}
.ab-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ab-type {
  margin-left: 4px;
  opacity: 0.45;
  font-size: 10px;
}
.ab-miss {
  margin-left: 4px;
  color: #f0a060;
  font-weight: 700;
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
