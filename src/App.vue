<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import MonacoEditor, { type ScrollInfo } from "./components/MonacoEditor.vue";
import MarkdownWysiwyg from "./components/MarkdownWysiwyg.vue";
import HtmlPreview from "./components/HtmlPreview.vue";
import TreePreview from "./components/TreePreview.vue";
import AssetBrowser from "./components/AssetBrowser.vue";
import ConvertModal from "./components/ConvertModal.vue";
import FormulaEditorModal from "./components/FormulaEditorModal.vue";
import ToolbarIcon from "./components/ToolbarIcon.vue";
import {
  findMathAtOffset,
  findMathByLatex,
  replaceMathRange,
  wrapMath,
  type MathMode,
  type MathRange,
} from "./utils/mdMathRange";
import type { EditorTab, ViewMode } from "./types";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/messages";
import {
  clearRecent,
  listRecent,
  pushRecent,
  readTextFile,
  openInOs,
  takePendingOpens,
  pathIsDir,
  writeTextFile,
} from "./utils/api";
import {
  assetCreateFromPath,
  assetDelete,
  assetFindByPath,
  folderImport,
  assetRelocate,
  assetSaveNewVersion,
  fileNameFromPath,
  indexInvalid,
  parentDirOfPath,
  revealInOs,
  type AssetDto,
} from "./utils/assetsApi";
import {
  clipperListInbox,
  clipperOpenInstall,
  mcpDiscoverAgents,
  mcpInstallForAgents,
  mcpSkillGet,
  mcpSkillReload,
  type DiscoveredAgent,
  type InboxEntry,
  type McpSkillView,
} from "./utils/clipperApi";
import {
  settingsGet,
  settingsSave,
  type AppSettings,
  type SettingsView,
} from "./utils/settingsApi";
import {
  convertCancel,
  convertDefaultOutput,
  convertIsSupported,
  convertRun,
  convertToolForPath,
  conversionLatestSource,
  type ConversionSourceDto,
} from "./utils/convertApi";
import {
  contrastBorder,
  contrastHighlightBorder,
  contrastHighlightFg,
  contrastMuted,
  contrastText,
} from "./utils/contrast";
import {
  isMarkdownPath,
  isSupportedPath,
  languageFromPath,
  openDialogFilters,
  previewKind,
  titleFromPath,
} from "./utils/language";
const { t } = useI18n();

let tabSeq = 1;
let untitledSeq = 1;

function newTab(partial?: Partial<EditorTab>): EditorTab {
  const id = `tab-${tabSeq++}`;
  return {
    id,
    path: null,
    title: titleFromPath(null, untitledSeq++),
    content: "",
    dirty: false,
    language: "markdown",
    viewMode: "split",
    cursorLine: 1,
    cursorCol: 1,
    ...partial,
  };
}

/* Bump keys when default chassis colors change so old localStorage doesn't hide the refresh. */
const BG_EDIT_KEY = "monolith.bg.edit.v2";
const BG_PREVIEW_KEY = "monolith.bg.preview.v2";
const FONT_SIZE_KEY = "monolith.fontSize";

const FONT_SIZES = [11, 12, 13, 14, 15, 16, 18, 20, 22, 24];

/** Common Markdown inserts (label → text). */
const MD_CHAR_DEFS: { label: string; titleKey: MessageKey; text: string }[] = [
  { label: "—", titleKey: "charEmDash", text: "—" },
  { label: "…", titleKey: "charEllipsis", text: "…" },
  { label: "·", titleKey: "charMiddleDot", text: "·" },
  { label: "×", titleKey: "charTimes", text: "×" },
  { label: "✓", titleKey: "charCheck", text: "✓" },
  { label: "→", titleKey: "charRightArrow", text: "→" },
  { label: "←", titleKey: "charLeftArrow", text: "←" },
  { label: "≠", titleKey: "charNe", text: "≠" },
  { label: "≤", titleKey: "charLe", text: "≤" },
  { label: "≥", titleKey: "charGe", text: "≥" },
  { label: "©", titleKey: "charCopy", text: "©" },
  { label: "®", titleKey: "charReg", text: "®" },
  { label: "™", titleKey: "charTm", text: "™" },
  { label: "§", titleKey: "charSection", text: "§" },
  { label: "¶", titleKey: "charPilcrow", text: "¶" },
  { label: "«»", titleKey: "charGuillemets", text: "«»" },
  { label: "“”", titleKey: "charCurlyDq", text: "“”" },
  { label: "‘’", titleKey: "charCurlySq", text: "‘’" },
];
const mdChars = computed(() =>
  MD_CHAR_DEFS.map((c) => ({ label: c.label, title: t(c.titleKey), text: c.text })),
);

/** Empty on launch — Untitled only via New / + (not auto-created). */
const tabs = ref<EditorTab[]>([]);
const activeId = ref<string | null>(null);
const recent = ref<string[]>([]);
const recentOpen = ref(false);
const recentBtnRef = ref<HTMLButtonElement | null>(null);
const recentMenuStyle = ref<Record<string, string>>({});
const charsOpen = ref(false);
const charsBtnRef = ref<HTMLButtonElement | null>(null);
const charsMenuStyle = ref<Record<string, string>>({});
const formulaOpen = ref(false);
const formulaLatex = ref("");
const formulaMode = ref<MathMode>("block");
const formulaEditing = ref(false);
const formulaRange = ref<MathRange | null>(null);
const tableOpen = ref(false);
const tableHover = ref({ rows: 0, cols: 0 });
const tableBtnRef = ref<HTMLButtonElement | null>(null);
const tablePickerStyle = ref<Record<string, string>>({});
const TABLE_PICK_MAX = 8;
const splitRatio = ref(0.5);
const dragging = ref(false);
const statusError = ref("");
const ASSET_WIDTH_KEY = "monolith.assets.width";
const ASSET_COLLAPSED_KEY = "monolith.assets.collapsed";
const ASSET_RAIL_PX = 36;
const ASSET_MIN_PX = 180;
const ASSET_MAX_PX = 480;
const ASSET_DEFAULT_PX = 260;

function readAssetWidth(): number {
  const n = Number(localStorage.getItem(ASSET_WIDTH_KEY));
  if (Number.isFinite(n) && n >= ASSET_MIN_PX && n <= ASSET_MAX_PX) return n;
  return ASSET_DEFAULT_PX;
}

const assetsCollapsed = ref(localStorage.getItem(ASSET_COLLAPSED_KEY) === "1");
const assetWidth = ref(readAssetWidth());
const assetWidthBeforeCollapse = ref<number | null>(null);
const selectedAssetId = ref<number | null>(null);
const assetBrowserRef = ref<InstanceType<typeof AssetBrowser> | null>(null);
const resizingAssets = ref(false);

const assetPaneWidth = computed(() =>
  assetsCollapsed.value ? ASSET_RAIL_PX : assetWidth.value,
);

function persistAssetLayout() {
  localStorage.setItem(ASSET_COLLAPSED_KEY, assetsCollapsed.value ? "1" : "0");
  if (!assetsCollapsed.value) {
    localStorage.setItem(ASSET_WIDTH_KEY, String(assetWidth.value));
  }
}

function toggleAssetsSidebar() {
  if (assetsCollapsed.value) {
    assetsCollapsed.value = false;
    assetWidth.value = Math.max(
      ASSET_MIN_PX,
      assetWidthBeforeCollapse.value ?? readAssetWidth(),
    );
    assetWidthBeforeCollapse.value = null;
  } else {
    assetWidthBeforeCollapse.value = assetWidth.value;
    assetsCollapsed.value = true;
  }
  persistAssetLayout();
}

function onAssetResizeDown(e: MouseEvent) {
  if (assetsCollapsed.value) return;
  e.preventDefault();
  resizingAssets.value = true;
  const startX = e.clientX;
  const startW = assetWidth.value;
  const onMove = (ev: MouseEvent) => {
    const next = Math.min(ASSET_MAX_PX, Math.max(ASSET_MIN_PX, startW + (ev.clientX - startX)));
    assetWidth.value = next;
  };
  const onUp = () => {
    resizingAssets.value = false;
    persistAssetLayout();
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}
const fontSize = ref(Number(localStorage.getItem(FONT_SIZE_KEY)) || 13);
const editorRef = ref<InstanceType<typeof MonacoEditor> | null>(null);
const previewEditorRef = ref<InstanceType<typeof MonacoEditor> | null>(null);
const mdWysiwygRef = ref<InstanceType<typeof MarkdownWysiwyg> | null>(null);
const htmlPreviewRef = ref<InstanceType<typeof HtmlPreview> | null>(null);
const treePreviewRef = ref<InstanceType<typeof TreePreview> | null>(null);
/** Which MD surface receives format toolbar actions. */
const mdFocus = ref<"source" | "wysiwyg">("source");

const editBg = ref(localStorage.getItem(BG_EDIT_KEY) || "#0a0e14");
const previewBg = ref(localStorage.getItem(BG_PREVIEW_KEY) || "#101820");

let scrollLock = false;
/** Last user-driven side for MD split sync. Programmatic scrolls must not flip this. */
let mdDriver: "source" | "preview" = "source";

const active = computed(() => tabs.value.find((tab) => tab.id === activeId.value) ?? null);
const hasTab = computed(() => active.value != null);
const showEdit = computed(() => {
  const a = active.value;
  if (!a) return false;
  return a.viewMode === "edit" || a.viewMode === "split";
});
const showPreview = computed(() => {
  const a = active.value;
  if (!a) return false;
  return a.viewMode === "preview" || a.viewMode === "split";
});
const kind = computed(() => previewKind(active.value?.path ?? null));
const isSplit = computed(() => active.value?.viewMode === "split");
const isMd = computed(() => kind.value === "markdown");
/** Markdown right pane: TipTap (editable) in preview and split. */
const useMdWysiwyg = computed(() => kind.value === "markdown" && showPreview.value);
/** MD format helpers work on source or WYSIWYG when either pane is visible. */
const canEditMd = computed(() => isMd.value && (showEdit.value || showPreview.value));
/** Toolbar targets TipTap when preview-only, or when the right pane has focus in split. */
const useWysiwygToolbar = computed(
  () =>
    useMdWysiwyg.value &&
    (active.value?.viewMode === "preview" || mdFocus.value === "wysiwyg"),
);

/** Source path from conversion log or asset_instance.source_path. */
const sourceProvenance = ref<ConversionSourceDto | null>(null);
const sourceMenuOpen = ref(false);
const sourceBtnRef = ref<HTMLButtonElement | null>(null);
const sourceMenuStyle = ref<Record<string, string>>({});
const canViewSource = computed(() => Boolean(sourceProvenance.value?.path));
const sourceIsUrl = computed(() => {
  const p = (sourceProvenance.value?.path || "").trim();
  return p.startsWith("http://") || p.startsWith("https://");
});

async function resolveSourceProvenance(mdPath: string): Promise<ConversionSourceDto | null> {
  const asset = await assetFindByPath(mdPath);
  if (asset?.sourcePath) {
    return {
      path: asset.sourcePath,
      recordedMtime: asset.sourceMtime,
      recordedSize: asset.sourceSize,
      currentMtime: null,
      currentSize: null,
      exists: asset.sourceExists,
      stale: asset.sourceStale,
    };
  }
  return conversionLatestSource(mdPath);
}

async function refreshSourceStaleBanner() {
  const path = active.value?.path;
  if (!path) {
    sourceProvenance.value = null;
    sourceMenuOpen.value = false;
    return;
  }
  try {
    sourceProvenance.value = await resolveSourceProvenance(path);
  } catch (e) {
    sourceProvenance.value = null;
    statusError.value = String(e);
  }
}

async function promptReconvertIfStale() {
  await refreshSourceStaleBanner();
  if (!sourceProvenance.value?.stale) return;
  const ok = await ask(t("sourceStaleAsk"), {
    title: t("sourceStaleAskTitle"),
    kind: "warning",
    okLabel: t("sourceReconvert"),
    cancelLabel: t("cancel"),
  });
  if (ok) await reconvertFromSource();
}

function placeSourceMenu() {
  const btn = sourceBtnRef.value;
  if (!btn) return;
  const rect = btn.getBoundingClientRect();
  const pad = 8;
  const width = 200;
  let left = rect.right - width;
  if (left < pad) left = pad;
  if (left + width > window.innerWidth - pad) left = window.innerWidth - width - pad;
  sourceMenuStyle.value = {
    position: "fixed",
    top: `${Math.round(rect.bottom + 4)}px`,
    left: `${Math.round(left)}px`,
    zIndex: "9999",
    minWidth: `${width}px`,
  };
}

function placeRecentMenu() {
  const btn = recentBtnRef.value;
  if (!btn) return;
  const rect = btn.getBoundingClientRect();
  const pad = 8;
  const width = 320;
  let left = rect.left;
  if (left + width > window.innerWidth - pad) left = window.innerWidth - width - pad;
  if (left < pad) left = pad;
  recentMenuStyle.value = {
    position: "fixed",
    top: `${Math.round(rect.bottom + 4)}px`,
    left: `${Math.round(left)}px`,
    zIndex: "9999",
    minWidth: `${width}px`,
    maxWidth: "420px",
  };
}

async function toggleRecentMenu() {
  charsOpen.value = false;
  tableOpen.value = false;
  sourceMenuOpen.value = false;
  recentOpen.value = !recentOpen.value;
  if (recentOpen.value) {
    await refreshRecent();
    void nextTick(() => placeRecentMenu());
  }
}

async function toggleSourceMenu() {
  if (!canViewSource.value) return;
  recentOpen.value = false;
  charsOpen.value = false;
  tableOpen.value = false;
  sourceMenuOpen.value = !sourceMenuOpen.value;
  if (sourceMenuOpen.value) {
    void nextTick(() => placeSourceMenu());
  }
}

async function openSourceInOs() {
  const p = sourceProvenance.value?.path;
  sourceMenuOpen.value = false;
  if (!p) return;
  try {
    await openInOs(p);
  } catch (e) {
    statusError.value = String(e);
  }
}

async function revealSourceInOs() {
  const p = sourceProvenance.value?.path;
  sourceMenuOpen.value = false;
  if (!p) return;
  try {
    await revealInOs(p);
  } catch (e) {
    statusError.value = String(e);
  }
}

async function reconvertFromSource() {
  const prov = sourceProvenance.value;
  const tab = active.value;
  const out = tab?.path;
  if (!prov?.path || !out || !tab) return;
  convertBusy = true;
  convertOpen.value = true;
  convertCancelling.value = false;
  try {
    const tool = await convertToolForPath(prov.path);
    convertTool.value = tool;
    convertInputName.value = prov.path.split(/[/\\]/).pop() || prov.path;
    convertOutputPath.value = out;
    convertPhase.value = t("convertRunning", { tool });
    convertDetail.value = "";
    await convertRun(prov.path, out);
    convertOpen.value = false;
    statusError.value = t("convertDone");
    const content = await readTextFile(out);
    tab.content = content;
    tab.dirty = false;
    await refreshSourceStaleBanner();
    await assetBrowserRef.value?.refresh();
  } catch (e) {
    convertOpen.value = false;
    if (convertCancelling.value) {
      statusError.value = t("convertCancelled");
    } else {
      statusError.value = t("convertFailed", { msg: String(e) });
    }
  } finally {
    convertBusy = false;
    convertCancelling.value = false;
  }
}

const previewFg = computed(() => contrastText(previewBg.value));
const previewMuted = computed(() => contrastMuted(previewBg.value));
const previewBorder = computed(() => contrastBorder(previewBg.value));
const previewHlBorder = computed(() => contrastHighlightBorder(previewBg.value));
const previewHlFg = computed(() => contrastHighlightFg(previewBg.value));

function syncPreviewScroll(ratio: number) {
  switch (kind.value) {
    case "html":
      htmlPreviewRef.value?.setScrollRatio(ratio);
      break;
    case "json":
    case "xml":
      treePreviewRef.value?.setScrollRatio(ratio);
      break;
    default:
      previewEditorRef.value?.setScrollRatio(ratio);
  }
}

function setActive(id: string | null) {
  activeId.value = id;
  statusError.value = "";
}

function createUntitled() {
  const t = newTab();
  tabs.value.push(t);
  activeId.value = t.id;
}

async function refreshRecent() {
  try {
    recent.value = await listRecent();
  } catch {
    recent.value = [];
  }
}

function openBrokenIndex(asset: AssetDto) {
  selectedAssetId.value = asset.id;
  brokenIndexAsset.value = asset;
  brokenIndexOpen.value = true;
  statusError.value = t("assetMissing", { path: asset.absolutePath ?? "" });
}

async function openPath(path: string) {
  if (!isSupportedPath(path)) {
    try {
      if (await convertIsSupported(path)) {
        await beginConvert(path);
        return;
      }
    } catch {
      /* fall through to unsupported */
    }
    statusError.value = t("unsupportedType", { name: path.split(/[/\\]/).pop() || path });
    return;
  }
  try {
    const content = await readTextFile(path);
    const existing = tabs.value.find((t) => t.path === path);
    if (existing) {
      activeId.value = existing.id;
      await promptReconvertIfStale();
      return;
    }
    const tab = newTab({
      path,
      title: titleFromPath(path, 0),
      content,
      dirty: false,
      language: languageFromPath(path),
      viewMode: isMarkdownPath(path) || previewKind(path) !== "code" ? "split" : "edit",
    });
    tabs.value.push(tab);
    activeId.value = tab.id;
    recent.value = await pushRecent(path);
    statusError.value = "";
    await promptReconvertIfStale();
  } catch (e) {
    const asset = await assetFindByPath(path);
    if (asset) {
      openBrokenIndex(asset);
      return;
    }
    statusError.value = String(e);
  }
}

async function openFile() {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: openDialogFilters(),
    ...(settingsDraft.value.defaultOpenDir
      ? { defaultPath: settingsDraft.value.defaultOpenDir }
      : {}),
  });
  if (typeof selected === "string") {
    await openPath(selected);
  }
}

async function saveActive(forceAs = false) {
  const tab = active.value;
  if (!tab) return;
  let path = tab.path;
  if (!path || forceAs) {
    const fallbackName = `${tab.title}.md`;
    const saveDefault = path
      ? path
      : settingsDraft.value.defaultSaveDir
        ? `${settingsDraft.value.defaultSaveDir.replace(/\/$/, "")}/${fallbackName}`
        : fallbackName;
    const picked = await save({
      defaultPath: saveDefault,
      filters: openDialogFilters(),
    });
    if (!picked) return;
    path = picked;
  }
  try {
    await writeTextFile(path, tab.content);
    tab.path = path;
    tab.title = titleFromPath(path, 0);
    tab.language = languageFromPath(path);
    tab.dirty = false;
    recent.value = await pushRecent(path);
    statusError.value = "";
  } catch (e) {
    statusError.value = String(e);
  }
}

const registerOpen = ref(false);
const registerPath = ref("");
const registerRemark = ref("");
const registerSource = ref("");
const registerBusy = ref(false);
const registerInputRef = ref<HTMLInputElement | null>(null);

async function generateAsset() {
  const tab = active.value;
  if (!tab?.path) {
    statusError.value = t("assetNeedPath");
    return;
  }
  if (!isSupportedPath(tab.path)) {
    statusError.value = t("assetUnsupported");
    return;
  }
  registerPath.value = tab.path;
  registerRemark.value = fileNameFromPath(tab.path);
  registerSource.value = "";
  try {
    const fromLog = await conversionLatestSource(tab.path);
    if (fromLog?.path) registerSource.value = fromLog.path;
  } catch {
    /* no log / not in Tauri */
  }
  // Web clips: frontmatter `source:` / `url:` → provenance without inbox prompts.
  if (!registerSource.value) {
    const fm = tab.content.match(/^---\r?\n([\s\S]*?)\r?\n---/);
    const block = fm?.[1] ?? "";
    const m = block.match(/^(?:source|url|source_url)\s*:\s*["']?(https?:\/\/\S+?)["']?\s*$/m);
    if (m?.[1]) registerSource.value = m[1];
  }
  registerOpen.value = true;
  await nextTick();
  registerInputRef.value?.focus();
  registerInputRef.value?.select();
}

async function importFolder(dir?: string) {
  const selected =
    dir ??
    (await open({
      title: t("folderImportTitle"),
      directory: true,
      multiple: false,
    }));
  if (typeof selected !== "string" || !selected) return;
  const includeSubdirs = await ask(t("folderImportSubdirs"), {
    title: t("folderImport"),
    kind: "info",
    okLabel: t("folderImportYesSubdirs"),
    cancelLabel: t("folderImportNoSubdirs"),
  });
  try {
    const r = await folderImport(selected, includeSubdirs);
    statusError.value = t("folderImportDone", {
      created: String(r.created),
      mounted: String(r.mounted),
      skipped: String(r.skippedUnsupported + r.depthSkipped),
      failed: String(r.convertFailed),
    });
    await assetBrowserRef.value?.refresh();
  } catch (e) {
    statusError.value = String(e);
  }
}

const convertOpen = ref(false);
const convertTool = ref("");
const convertInputName = ref("");
const convertOutputPath = ref("");
const convertPhase = ref("");
const convertDetail = ref("");
const convertCancelling = ref(false);
let convertBusy = false;

async function beginConvert(inputPath: string) {
  if (convertBusy || convertOpen.value) {
    statusError.value = t("convertFailed", { msg: "busy" });
    return;
  }
  let tool = "";
  try {
    tool = await convertToolForPath(inputPath);
  } catch (e) {
    statusError.value = t("convertNotConvertible", {
      name: inputPath.split(/[/\\]/).pop() || inputPath,
    });
    return;
  }
  let defaultOut = "";
  try {
    defaultOut = await convertDefaultOutput(inputPath);
  } catch (e) {
    statusError.value = String(e);
    return;
  }
  const picked = await save({
    title: t("convertPickOutput"),
    defaultPath: defaultOut,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
  if (!picked) return;

  convertBusy = true;
  convertOpen.value = true;
  convertTool.value = tool;
  convertInputName.value = inputPath.split(/[/\\]/).pop() || inputPath;
  convertOutputPath.value = picked;
  convertPhase.value = t("convertRunning", { tool });
  convertDetail.value = "";
  convertCancelling.value = false;
  statusError.value = "";

  try {
    const result = await convertRun(inputPath, picked);
    convertOpen.value = false;
    statusError.value = t("convertDone");
    await openPath(result.outputPath);
  } catch (e) {
    const msg = String(e);
    convertOpen.value = false;
    if (convertCancelling.value) {
      statusError.value = t("convertCancelled");
    } else {
      statusError.value = t("convertFailed", { msg });
    }
  } finally {
    convertBusy = false;
    convertCancelling.value = false;
  }
}

async function onConvertCancel() {
  if (!convertOpen.value || convertCancelling.value) return;
  convertCancelling.value = true;
  convertPhase.value = t("convertCancelling");
  try {
    await convertCancel();
  } catch (e) {
    statusError.value = String(e);
  }
}

async function convertFilePicker() {
  const selected = await open({
    multiple: false,
    directory: false,
    ...(settingsDraft.value.defaultOpenDir
      ? { defaultPath: settingsDraft.value.defaultOpenDir }
      : {}),
  });
  if (typeof selected !== "string") return;
  if (isSupportedPath(selected)) {
    await openPath(selected);
    return;
  }
  try {
    if (await convertIsSupported(selected)) {
      await beginConvert(selected);
      return;
    }
  } catch {
    /* fall through */
  }
  statusError.value = t("convertNotConvertible", {
    name: selected.split(/[/\\]/).pop() || selected,
  });
}

function cancelRegister() {
  if (registerBusy.value) return;
  registerOpen.value = false;
}

async function confirmRegister() {
  if (registerBusy.value || !registerPath.value) return;
  if (!isSupportedPath(registerPath.value)) {
    statusError.value = t("assetUnsupported");
    return;
  }
  const remark = registerRemark.value.trim() || fileNameFromPath(registerPath.value);
  const source = registerSource.value.trim();
  registerBusy.value = true;
  try {
    const created = await assetCreateFromPath(registerPath.value, {
      displayName: remark,
      sourcePath: source.length ? source : null,
    });
    selectedAssetId.value = created.id;
    statusError.value = t("assetCreated");
    registerOpen.value = false;
    await assetBrowserRef.value?.refresh();
  } catch (e) {
    statusError.value = String(e);
  } finally {
    registerBusy.value = false;
  }
}

async function saveNewVersion() {
  const tab = active.value;
  if (!tab?.path) {
    statusError.value = t("assetNeedPath");
    return;
  }
  try {
    let asset = await assetFindByPath(tab.path);
    if (!asset) {
      statusError.value = t("assetNeedRegistered");
      return;
    }
    asset = await assetSaveNewVersion(asset.id, tab.content);
    if (asset.absolutePath) {
      tab.path = asset.absolutePath;
      tab.title = titleFromPath(asset.absolutePath, 0);
      tab.language = languageFromPath(asset.absolutePath);
      tab.dirty = false;
      recent.value = await pushRecent(asset.absolutePath);
    }
    selectedAssetId.value = asset.id;
    statusError.value = "";
    await assetBrowserRef.value?.refresh();
  } catch (e) {
    statusError.value = String(e);
  }
}

const brokenIndexOpen = ref(false);
const brokenIndexAsset = ref<AssetDto | null>(null);
const brokenIndexBusy = ref(false);

function closeBrokenIndex() {
  if (brokenIndexBusy.value) return;
  brokenIndexOpen.value = false;
  brokenIndexAsset.value = null;
}

async function openAssetFromTree(asset: AssetDto) {
  selectedAssetId.value = asset.id;
  if (!asset.absolutePath || indexInvalid(asset)) {
    openBrokenIndex(asset);
    return;
  }
  await openPath(asset.absolutePath);
}

async function brokenIndexRelocate() {
  const asset = brokenIndexAsset.value;
  if (!asset || brokenIndexBusy.value) return;
  const selected = await open({
    title: t("assetRelocateTitle"),
    defaultPath: parentDirOfPath(asset.absolutePath),
    multiple: false,
    directory: false,
    filters: openDialogFilters(),
  });
  if (typeof selected !== "string") return;
  brokenIndexBusy.value = true;
  try {
    const updated = await assetRelocate(asset.id, selected);
    brokenIndexOpen.value = false;
    brokenIndexAsset.value = null;
    statusError.value = "";
    await assetBrowserRef.value?.refresh();
    selectedAssetId.value = updated.id;
    if (updated.absolutePath) await openPath(updated.absolutePath);
  } catch (e) {
    statusError.value = String(e);
  } finally {
    brokenIndexBusy.value = false;
  }
}

async function brokenIndexDelete() {
  const asset = brokenIndexAsset.value;
  if (!asset || brokenIndexBusy.value) return;
  const ok = await ask(t("assetDeleteConfirm", { name: asset.displayName }), {
    title: t("assetDeleteTitle"),
    kind: "warning",
    okLabel: t("assetIndexDelete"),
    cancelLabel: t("cancel"),
  });
  if (!ok) return;
  brokenIndexBusy.value = true;
  try {
    await assetDelete(asset.id);
    if (selectedAssetId.value === asset.id) selectedAssetId.value = null;
    brokenIndexOpen.value = false;
    brokenIndexAsset.value = null;
    statusError.value = "";
    await assetBrowserRef.value?.refresh();
  } catch (e) {
    statusError.value = String(e);
  } finally {
    brokenIndexBusy.value = false;
  }
}

async function brokenIndexRebuild() {
  const asset = brokenIndexAsset.value;
  if (!asset || brokenIndexBusy.value) return;
  if (!asset.sourcePath || !asset.sourceExists) {
    statusError.value = t("assetIndexRebuildNeedSource");
    return;
  }
  if (/^https?:\/\//i.test(asset.sourcePath)) {
    statusError.value = t("assetIndexRebuildNeedSource");
    return;
  }
  if (!asset.absolutePath) {
    statusError.value = t("assetIndexBrokenTitle");
    return;
  }
  const picked = await save({
    title: t("convertPickOutput"),
    defaultPath: asset.absolutePath,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
  if (!picked) return;

  brokenIndexBusy.value = true;
  convertBusy = true;
  convertOpen.value = true;
  convertCancelling.value = false;
  try {
    const tool = await convertToolForPath(asset.sourcePath);
    convertTool.value = tool;
    convertInputName.value = asset.sourcePath.split(/[/\\]/).pop() || asset.sourcePath;
    convertOutputPath.value = picked;
    convertPhase.value = t("convertRunning", { tool });
    convertDetail.value = "";
    await convertRun(asset.sourcePath, picked);
    let updated = asset;
    if (picked !== asset.absolutePath) {
      updated = await assetRelocate(asset.id, picked);
    }
    convertOpen.value = false;
    brokenIndexOpen.value = false;
    brokenIndexAsset.value = null;
    statusError.value = t("assetIndexRebuilt");
    await assetBrowserRef.value?.refresh();
    selectedAssetId.value = updated.id;
    await openPath(picked);
  } catch (e) {
    convertOpen.value = false;
    if (convertCancelling.value) {
      statusError.value = t("convertCancelled");
    } else {
      statusError.value = t("convertFailed", { msg: String(e) });
    }
  } finally {
    convertBusy = false;
    convertCancelling.value = false;
    brokenIndexBusy.value = false;
  }
}

async function saveNewVersionForAsset(asset: AssetDto) {
  await openAssetFromTree(asset);
  await nextTick();
  await saveNewVersion();
}

async function closeTab(id: string | null) {
  if (!id) return;
  const idx = tabs.value.findIndex((t) => t.id === id);
  if (idx < 0) return;
  const tab = tabs.value[idx];
  if (tab.dirty) {
    const ok = await ask(t("unsavedClose", { title: tab.title }), {
      title: t("unsavedTitle"),
      kind: "warning",
      okLabel: t("close"),
      cancelLabel: t("cancel"),
    });
    if (!ok) return;
  }
  tabs.value.splice(idx, 1);
  if (tabs.value.length === 0) {
    activeId.value = null;
    sourceProvenance.value = null;
    sourceMenuOpen.value = false;
    return;
  }
  if (activeId.value === id) {
    const next = tabs.value[Math.min(idx, tabs.value.length - 1)];
    activeId.value = next.id;
  }
}

function onContent(v: string) {
  const tab = active.value;
  if (!tab) return;
  if (tab.content === v) return;
  tab.content = v;
  tab.dirty = true;
}

/** TipTap → source: ignore while typing in Monaco so markdown rewrite cannot setValue the left pane. */
function onWysiwygContent(v: string) {
  if (isSplit.value && mdFocus.value === "source") return;
  onContent(v);
}

/** Left → right (TipTap block aligned to source span). */
function syncMdPreviewFromSource(line: number) {
  if (!useMdWysiwyg.value || !isSplit.value || line < 1) return;
  const viewportTop = editorRef.value?.getLineViewportTop(line) ?? 0;
  mdWysiwygRef.value?.alignToSourceLine(line, Math.max(0, viewportTop));
}

function onCursor(line: number, col: number) {
  const tab = active.value;
  if (!tab) return;
  tab.cursorLine = line;
  tab.cursorCol = col;
  if (!showPreview.value) return;
  if (useMdWysiwyg.value && isSplit.value) {
    if (mdDriver === "preview") return;
    mdDriver = "source";
    syncMdPreviewFromSource(line);
  } else if (kind.value === "code") {
    void nextTick(() => previewEditorRef.value?.revealLine(line));
  }
}

function onSourceFocus() {
  mdFocus.value = "source";
}
function onWysiwygFocus() {
  mdFocus.value = "wysiwyg";
}

function setViewMode(mode: ViewMode) {
  const tab = active.value;
  if (!tab) return;
  tab.viewMode = mode;
  if (mode === "preview") mdFocus.value = "wysiwyg";
  if (mode === "edit") mdFocus.value = "source";
}

const VIEW_MODE_CYCLE: ViewMode[] = ["edit", "preview", "split"];
/** Dedup menu accelerator + capture keydown so ⌘E cannot skip a mode. */
let viewCycleGuardUntil = 0;

function cycleViewMode() {
  const tab = active.value;
  if (!tab) return;
  const now = performance.now();
  if (now < viewCycleGuardUntil) return;
  viewCycleGuardUntil = now + 120;
  const i = VIEW_MODE_CYCLE.indexOf(tab.viewMode);
  setViewMode(VIEW_MODE_CYCLE[(i < 0 ? 0 : i + 1) % VIEW_MODE_CYCLE.length]!);
}

function runEditorAction(id: string) {
  editorRef.value?.trigger(id);
}

function mdBold() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleBold();
  else editorRef.value?.insertAround("**", "**", "bold");
}
function mdItalic() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleItalic();
  else editorRef.value?.insertAround("*", "*", "italic");
}
function mdStrike() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleStrike();
  else editorRef.value?.insertAround("~~", "~~", "text");
}
function mdCode() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleCode();
  else editorRef.value?.insertAround("`", "`", "code");
}
function mdCodeBlock() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleCodeBlock();
  else editorRef.value?.insertAround("```\n", "\n```", "code");
}
function mdLink() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.setLink();
  else editorRef.value?.insertAround("[", "](url)", "text");
}
function mdImage() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.setImage();
  else editorRef.value?.insertAround("![", "](url)", "alt");
}
function mdQuote() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleBlockquote();
  else editorRef.value?.insertLinePrefix("> ");
}
function mdUl() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleBulletList();
  else editorRef.value?.insertLinePrefix("- ");
}
function mdOl() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleOrderedList();
  else editorRef.value?.toggleOrderedList();
}
function mdTask() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.toggleTaskList();
  else editorRef.value?.insertLinePrefix("- [ ] ");
}
function mdHr() {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.setHorizontalRule();
  else editorRef.value?.insertText("\n---\n");
}
function formatSql() {
  editorRef.value?.formatSql();
}
const isSql = computed(() => active.value?.language === "sql");
/** rows/cols = 含表头的总行数 × 列数（至少 1×1 → 仅表头）。 */
function buildMdTable(rows: number, cols: number): string {
  const r = Math.min(TABLE_PICK_MAX, Math.max(1, Math.floor(rows)));
  const c = Math.min(TABLE_PICK_MAX, Math.max(1, Math.floor(cols)));
  const header = Array.from({ length: c }, (_, i) => t("colN", { n: i + 1 })).join(" | ");
  const sep = Array.from({ length: c }, () => "---").join(" | ");
  const bodyCount = Math.max(0, r - 1);
  const body = Array.from({ length: bodyCount }, () =>
    Array.from({ length: c }, () => " ").join(" | "),
  );
  const lines = [`| ${header} |`, `| ${sep} |`, ...body.map((line) => `| ${line} |`)];
  return `${lines.join("\n")}\n`;
}

function placeTablePicker() {
  const btn = tableBtnRef.value;
  if (!btn) return;
  const rect = btn.getBoundingClientRect();
  const pad = 8;
  const width = 220;
  let left = rect.right - width;
  if (left < pad) left = pad;
  if (left + width > window.innerWidth - pad) left = window.innerWidth - width - pad;
  tablePickerStyle.value = {
    position: "fixed",
    top: `${Math.round(rect.bottom + 4)}px`,
    left: `${Math.round(left)}px`,
    zIndex: "9999",
  };
}

function toggleTablePicker() {
  charsOpen.value = false;
  recentOpen.value = false;
  tableOpen.value = !tableOpen.value;
  if (tableOpen.value) {
    void nextTick(() => placeTablePicker());
  }
}

function placeCharsMenu() {
  const btn = charsBtnRef.value;
  if (!btn) return;
  const rect = btn.getBoundingClientRect();
  const pad = 8;
  const width = 280;
  let left = rect.right - width;
  if (left < pad) left = pad;
  if (left + width > window.innerWidth - pad) left = window.innerWidth - width - pad;
  charsMenuStyle.value = {
    position: "fixed",
    top: `${Math.round(rect.bottom + 4)}px`,
    left: `${Math.round(left)}px`,
    width: `${width}px`,
    zIndex: "9999",
  };
}

function toggleCharsMenu() {
  tableOpen.value = false;
  recentOpen.value = false;
  formulaOpen.value = false;
  charsOpen.value = !charsOpen.value;
  if (charsOpen.value) {
    void nextTick(() => placeCharsMenu());
  }
}

function openFormulaEditor(opts?: {
  latex?: string;
  mode?: MathMode;
  range?: MathRange | null;
}) {
  const tab = active.value;
  if (!tab || !isMd.value) return;
  charsOpen.value = false;
  tableOpen.value = false;
  recentOpen.value = false;

  let range = opts?.range ?? null;
  let latex = opts?.latex ?? "";
  let mode: MathMode = opts?.mode ?? "block";

  if (!range && !opts?.latex) {
    const content = tab.content ?? "";
    const offset = editorRef.value?.getCursorOffset?.() ?? -1;
    if (offset >= 0) {
      range = findMathAtOffset(content, offset);
      if (range) {
        latex = range.latex;
        mode = range.mode;
      } else {
        const sel = editorRef.value?.getSelectedText?.() ?? "";
        if (sel.trim()) latex = sel.trim();
      }
    }
  }

  formulaRange.value = range;
  formulaLatex.value = latex;
  formulaMode.value = mode;
  formulaEditing.value = !!range;
  formulaOpen.value = true;
}

function closeFormulaEditor() {
  formulaOpen.value = false;
  formulaRange.value = null;
  formulaEditing.value = false;
}

function onFormulaConfirm(payload: { latex: string; mode: MathMode }) {
  const tab = active.value;
  if (!tab) return;
  const wrapped = wrapMath(payload.latex, payload.mode);
  const range = formulaRange.value;

  if (range) {
    if (editorRef.value && showEdit.value) {
      editorRef.value.replaceOffsetRange(range.start, range.end, wrapped);
    } else {
      onContent(replaceMathRange(tab.content, range, payload.latex, payload.mode));
    }
  } else if (editorRef.value && showEdit.value) {
    const insert =
      payload.mode === "block"
        ? (tab.content.endsWith("\n") || tab.content.length === 0 ? "" : "\n") + wrapped + "\n"
        : wrapped;
    // When selection is empty, insertText replaces empty sel (= insert at cursor).
    editorRef.value.insertText(insert);
  } else {
    const base = tab.content ?? "";
    const sep = base.length === 0 || base.endsWith("\n") ? "" : "\n";
    onContent(base + sep + wrapped + (payload.mode === "block" ? "\n" : ""));
  }
  closeFormulaEditor();
}

function onEditMathFromPreview(payload: {
  latex: string;
  mode: MathMode;
  preferStart?: number;
}) {
  const tab = active.value;
  if (!tab) return;
  const range = findMathByLatex(tab.content, payload.latex, payload.mode, payload.preferStart);
  openFormulaEditor({
    latex: payload.latex,
    mode: payload.mode,
    range,
  });
}

function insertMdTable(rows: number, cols: number) {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.insertTable(rows, cols);
  else editorRef.value?.insertText(buildMdTable(rows, cols));
  tableOpen.value = false;
  tableHover.value = { rows: 0, cols: 0 };
}

function onTableCellEnter(rows: number, cols: number) {
  tableHover.value = { rows, cols };
}

function tableCellRow(idx: number) {
  return Math.ceil(idx / TABLE_PICK_MAX);
}
function tableCellCol(idx: number) {
  return ((idx - 1) % TABLE_PICK_MAX) + 1;
}
const headingSelect = ref("");

function setHeading(level: number) {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.setHeading(level);
  else editorRef.value?.setHeadingLevel(level);
}

function onHeadingChange(e: Event) {
  const raw = (e.target as HTMLSelectElement).value;
  if (raw === "") return;
  const level = Number(raw);
  if (!Number.isFinite(level)) return;
  setHeading(level);
  // reset to placeholder so the same level can be re-applied
  headingSelect.value = "";
  (e.target as HTMLSelectElement).value = "";
}
function indentLines() {
  editorRef.value?.indentLines();
}
function outdentLines() {
  editorRef.value?.outdentLines();
}
function insertMdChar(text: string) {
  if (useWysiwygToolbar.value) mdWysiwygRef.value?.insertText(text);
  else editorRef.value?.insertText(text);
  charsOpen.value = false;
}
function onFontSizeChange(e: Event) {
  const n = Number((e.target as HTMLSelectElement).value);
  if (!Number.isFinite(n)) return;
  fontSize.value = n;
  localStorage.setItem(FONT_SIZE_KEY, String(n));
}

function scrollRatio(info: ScrollInfo): number {
  const max = Math.max(1, info.scrollHeight - info.height);
  return info.scrollTop / max;
}

function onEditScroll(info: ScrollInfo) {
  if (!isSplit.value || scrollLock) return;
  if (useMdWysiwyg.value) {
    if (mdDriver === "preview") return;
    mdDriver = "source";
    const tab = active.value;
    if (!tab) return;
    const line = editorRef.value?.getAnchorLine() ?? tab.cursorLine;
    tab.cursorLine = line;
    syncMdPreviewFromSource(line);
    return;
  }
  scrollLock = true;
  syncPreviewScroll(scrollRatio(info));
  requestAnimationFrame(() => {
    scrollLock = false;
  });
}

function onPreviewEditorScroll(info: ScrollInfo) {
  if (!isSplit.value || scrollLock) return;
  scrollLock = true;
  editorRef.value?.setScrollRatio(scrollRatio(info));
  requestAnimationFrame(() => {
    scrollLock = false;
  });
}

function onMdPreviewScroll(info: {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  anchorLine?: number;
  anchorViewportTop?: number;
}) {
  if (!isSplit.value || scrollLock) return;
  if (useMdWysiwyg.value) {
    if (mdDriver === "source") return;
    if (!info.anchorLine || info.anchorLine < 1) return;
    const tab = active.value;
    if (!tab) return;
    mdDriver = "preview";
    tab.cursorLine = info.anchorLine;
    editorRef.value?.alignToLine(info.anchorLine, info.anchorViewportTop ?? 0);
    // Position without reveal (alignToLine owns scroll).
    editorRef.value?.getEditor?.()?.setPosition({
      lineNumber: info.anchorLine,
      column: 1,
    });
    return;
  }
  scrollLock = true;
  const max = Math.max(1, info.scrollHeight - info.clientHeight);
  editorRef.value?.setScrollRatio(info.scrollTop / max);
  requestAnimationFrame(() => {
    scrollLock = false;
  });
}

function onMdLineClick(line: number, viewportTop = 0) {
  if (line < 1) return;
  const tab = active.value;
  if (!tab) return;
  mdDriver = "preview";
  mdFocus.value = "wysiwyg";
  tab.cursorLine = line;
  const ed = editorRef.value?.getEditor?.();
  if (!ed) return;
  // Align first — setPosition must not fight scroll (no reveal).
  if (isSplit.value) {
    editorRef.value?.alignToLine(line, Math.max(0, viewportTop));
  }
  ed.setPosition({ lineNumber: line, column: 1 });
}

function onMdPreviewPointer() {
  if (useMdWysiwyg.value) mdDriver = "preview";
}

function onSourcePointer() {
  if (useMdWysiwyg.value) mdDriver = "source";
}

function onEditBg(e: Event) {
  const v = (e.target as HTMLInputElement).value;
  editBg.value = v;
  localStorage.setItem(BG_EDIT_KEY, v);
}

function onPreviewBg(e: Event) {
  const v = (e.target as HTMLInputElement).value;
  previewBg.value = v;
  localStorage.setItem(BG_PREVIEW_KEY, v);
}

function onSplitDown(e: MouseEvent) {
  e.preventDefault();
  dragging.value = true;
}

function onSplitMove(e: MouseEvent) {
  if (!dragging.value) return;
  const pane = document.querySelector(".main-pane") as HTMLElement | null;
  if (!pane) return;
  const rect = pane.getBoundingClientRect();
  const ratio = (e.clientX - rect.left) / rect.width;
  splitRatio.value = Math.min(0.8, Math.max(0.2, ratio));
}

function onSplitUp() {
  dragging.value = false;
}

async function installWebClipper() {
  try {
    await clipperOpenInstall();
    statusError.value = t("installClipperDone");
    if (settingsOpen.value) await loadSettings();
  } catch (e) {
    statusError.value = String(e);
  }
}

const inboxOpen = ref(false);
const inboxBusy = ref(false);
const inboxEntries = ref<InboxEntry[]>([]);
const inboxQuery = ref("");
const inboxFilter = ref<"all" | "unregistered" | "registered">("all");
type InboxSortKey = "name" | "mtime" | "source" | "kind" | "registered";
const inboxSortKey = ref<InboxSortKey>("mtime");
const inboxSortDir = ref<"asc" | "desc">("desc");

function toggleInboxSort(key: InboxSortKey) {
  if (inboxSortKey.value === key) {
    inboxSortDir.value = inboxSortDir.value === "desc" ? "asc" : "desc";
  } else {
    inboxSortKey.value = key;
    inboxSortDir.value = key === "name" || key === "source" ? "asc" : "desc";
  }
}

function inboxKindLabel(kind: string) {
  if (kind === "clip") return t("inboxKindClip");
  if (kind === "convert") return t("inboxKindConvert");
  return t("inboxKindUnknown");
}

function inboxSourceLabel(source: string) {
  const s = source.trim();
  if (!s) return t("inboxSourceNone");
  try {
    if (/^https?:\/\//i.test(s)) {
      const u = new URL(s);
      return u.hostname + (u.pathname === "/" ? "" : u.pathname);
    }
  } catch {
    /* fall through */
  }
  const parts = s.split(/[/\\]/);
  return parts[parts.length - 1] || s;
}

function formatInboxMtime(sec: number) {
  if (!sec) return t("inboxSourceNone");
  const d = new Date(sec * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

const inboxVisible = computed(() => {
  const q = inboxQuery.value.trim().toLowerCase();
  const rows = inboxEntries.value.filter((e) => {
    if (inboxFilter.value === "registered" && !e.registered) return false;
    if (inboxFilter.value === "unregistered" && e.registered) return false;
    if (!q) return true;
    return e.name.toLowerCase().includes(q) || e.source.toLowerCase().includes(q);
  });
  const key = inboxSortKey.value;
  const dir = inboxSortDir.value === "asc" ? 1 : -1;
  return [...rows].sort((a, b) => {
    let cmp = 0;
    if (key === "mtime") cmp = a.mtime - b.mtime;
    else if (key === "registered") cmp = Number(a.registered) - Number(b.registered);
    else if (key === "kind") cmp = a.kind.localeCompare(b.kind);
    else if (key === "source") cmp = a.source.localeCompare(b.source, undefined, { sensitivity: "base" });
    else cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    return cmp * dir || a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  });
});

async function refreshInbox() {
  inboxBusy.value = true;
  try {
    inboxEntries.value = await clipperListInbox();
  } catch (e) {
    statusError.value = String(e);
    inboxEntries.value = [];
  } finally {
    inboxBusy.value = false;
  }
}

async function openInboxBrowser() {
  inboxOpen.value = true;
  inboxQuery.value = "";
  inboxFilter.value = "all";
  inboxSortKey.value = "mtime";
  inboxSortDir.value = "desc";
  try {
    await loadSettings();
  } catch {
    /* hint may be stale; listing still works */
  }
  await refreshInbox();
}

async function openInboxEntry(entry: InboxEntry) {
  inboxOpen.value = false;
  await openPath(entry.path);
}

const mcpInstallBusy = ref(false);
const mcpDiscoverBusy = ref(false);
const mcpDiscovered = ref<DiscoveredAgent[]>([]);
/** Selected installable agent ids */
const mcpSelected = ref<Record<string, boolean>>({});

type SettingsTab = "dirs" | "clipper" | "mcp" | "skill" | "shortcuts" | "about";

const shortcutRows = computed(() => [
  { keys: "⌘S", action: t("save"), note: "" },
  { keys: "⌘N", action: t("new"), note: "" },
  { keys: "⌘O", action: t("open"), note: "" },
  { keys: "⌘F", action: t("find"), note: "" },
  { keys: "⌘Z / ⇧⌘Z", action: `${t("undo")} / ${t("redo")}`, note: "" },
  { keys: "⌘B", action: t("bold"), note: "" },
  { keys: "⌘I", action: t("italic"), note: "" },
  { keys: "⌘D", action: t("strike"), note: "" },
  { keys: "⌘L", action: t("link"), note: "" },
  { keys: "⇧⌘K", action: t("codeBlock"), note: "" },
  { keys: "⇧⌘T", action: t("table"), note: "" },
  { keys: "⌘E", action: `${t("edit")} ↔ ${t("preview")} ↔ ${t("split")}`, note: "" },
  { keys: "⌘K", action: t("assetSearchPlaceholder"), note: "" },
  { keys: "⌘U", action: t("shortcutUnsupported"), note: t("shortcutUnsupported") },
]);

/** Skip app shortcuts in dialogs / tree search; still handle them inside Monaco / TipTap. */
function shouldSkipAppShortcut(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.closest(".monaco-editor, .ProseMirror")) return false;
  if (el.closest(".ab-input, .ab-search-input, .settings-modal, .register-modal, .inbox-modal")) {
    return true;
  }
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

function onAppKeydown(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey;
  if (!mod) return;
  // Prefer e.code — macOS WebView treats ⌘E as "Use Selection for Find" (feels like ⌘F)
  // if we don't capture it before the system/WebKit default.
  const code = e.code;
  const shift = e.shiftKey;
  const skip = shouldSkipAppShortcut(e.target);

  // ⌘K — focus asset tree search (not format; ⇧⌘K is code block below)
  if (code === "KeyK" && !shift) {
    if (skip) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    assetBrowserRef.value?.focusSearch?.();
    return;
  }

  // ⌘E — cycle edit → preview → split (must beat macOS/WebKit "find with selection")
  if (code === "KeyE" && !shift) {
    if (skip) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (!active.value) return;
    // Close Monaco find widget if it was left open from prior ⌘F
    editorRef.value?.trigger("closeFindWidget");
    cycleViewMode();
    return;
  }

  if (skip) return;
  if (!hasTab.value) return;

  if (code === "KeyB" && !shift) {
    e.preventDefault();
    mdBold();
    return;
  }
  if (code === "KeyI" && !shift) {
    e.preventDefault();
    mdItalic();
    return;
  }
  if (code === "KeyD" && !shift) {
    e.preventDefault();
    mdStrike();
    return;
  }
  if (code === "KeyL" && !shift) {
    e.preventDefault();
    mdLink();
    return;
  }
  if (code === "KeyK" && shift) {
    e.preventDefault();
    mdCodeBlock();
    return;
  }
  if (code === "KeyT" && shift) {
    e.preventDefault();
    toggleTablePicker();
    return;
  }
}

const settingsOpen = ref(false);
const settingsBusy = ref(false);
const settingsTab = ref<SettingsTab>("dirs");
const settingsView = ref<SettingsView | null>(null);
const settingsDraft = ref<AppSettings>({
  inboxDir: "",
  defaultOpenDir: "",
  defaultSaveDir: "",
});
const skillView = ref<McpSkillView | null>(null);
const skillBusy = ref(false);

async function loadSettings() {
  try {
    const view = await settingsGet();
    settingsView.value = view;
    settingsDraft.value = { ...view.settings };
  } catch (e) {
    statusError.value = String(e);
  }
}

async function loadSkill() {
  skillBusy.value = true;
  try {
    skillView.value = await mcpSkillGet();
  } catch (e) {
    skillView.value = null;
    statusError.value = String(e);
  } finally {
    skillBusy.value = false;
  }
}

async function openSystemSettings() {
  settingsOpen.value = true;
  settingsTab.value = "dirs";
  settingsBusy.value = true;
  try {
    await Promise.all([loadSettings(), refreshMcpAgents(), loadSkill()]);
  } finally {
    settingsBusy.value = false;
  }
}

/** Edit SKILL.md in the main Monolith workbench (normal save), not in settings. */
async function openSkillInMonolith() {
  skillBusy.value = true;
  try {
    const view = skillView.value ?? (await mcpSkillGet());
    skillView.value = view;
    settingsOpen.value = false;
    await openPath(view.path);
  } catch (e) {
    statusError.value = String(e);
  } finally {
    skillBusy.value = false;
  }
}

/** Push the on-disk App Support SKILL.md to discovered Agent skill dirs. */
async function reloadSkill() {
  skillBusy.value = true;
  try {
    const agents = mcpSelectedIds.value;
    const result = await mcpSkillReload(agents);
    await loadSkill();
    statusError.value = `${t("settingsSkillReloaded")}\n${result.notes.join("\n")}`;
  } catch (e) {
    statusError.value = String(e);
  } finally {
    skillBusy.value = false;
  }
}

async function pickSettingsDir(field: keyof AppSettings) {
  const selected = await open({
    multiple: false,
    directory: true,
    defaultPath: settingsDraft.value[field] || undefined,
  });
  if (typeof selected === "string") {
    settingsDraft.value = { ...settingsDraft.value, [field]: selected };
  }
}

function resetInboxDefault() {
  const db = settingsView.value?.assetsDbPath || settingsDraft.value.inboxDir || "";
  const m = db.match(/^(\/Users\/[^/]+)\//);
  if (m) {
    settingsDraft.value = {
      ...settingsDraft.value,
      inboxDir: `${m[1]}/Downloads/MonolithInbox`,
    };
  }
}

async function saveSystemSettings() {
  settingsBusy.value = true;
  try {
    const view = await settingsSave({ ...settingsDraft.value });
    settingsView.value = view;
    settingsDraft.value = { ...view.settings };
    statusError.value = t("settingsSaved");
    settingsOpen.value = false;
  } catch (e) {
    statusError.value = String(e);
  } finally {
    settingsBusy.value = false;
  }
}

const mcpInstallable = computed(() => mcpDiscovered.value.filter((a) => a.installable));
const mcpSelectedIds = computed(() =>
  mcpInstallable.value.filter((a) => mcpSelected.value[a.id]).map((a) => a.id),
);

async function refreshMcpAgents() {
  mcpDiscoverBusy.value = true;
  try {
    mcpDiscovered.value = await mcpDiscoverAgents();
    const next: Record<string, boolean> = {};
    for (const a of mcpDiscovered.value) {
      if (a.installable) {
        // Default: select all installable (keep prior choice if still present).
        next[a.id] = mcpSelected.value[a.id] ?? true;
      }
    }
    mcpSelected.value = next;
  } catch (e) {
    mcpDiscovered.value = [];
    mcpSelected.value = {};
    statusError.value = String(e);
  } finally {
    mcpDiscoverBusy.value = false;
  }
}

async function runMcpInstall() {
  const agents = mcpSelectedIds.value;
  if (!agents.length) return;
  mcpInstallBusy.value = true;
  try {
    const result = await mcpInstallForAgents(agents);
    statusError.value = `${t("installMcpDone")}: ${result.binaryPath}\n${result.notes.join("\n")}`;
    await refreshMcpAgents();
    await loadSettings();
  } catch (e) {
    statusError.value = String(e);
  } finally {
    mcpInstallBusy.value = false;
  }
}

async function handleMenu(action: string) {
  if (action.startsWith("open-recent:")) {
    await openPath(action.slice("open-recent:".length));
    return;
  }
  switch (action) {
    case "file-new":
      createUntitled();
      break;
    case "file-open":
      await openFile();
      break;
    case "tools-browse-inbox":
      await openInboxBrowser();
      break;
    case "assets-folder-import":
      await importFolder();
      break;
    case "assets-generate":
      await generateAsset();
      break;
    case "assets-new-version":
      await saveNewVersion();
      break;
    case "app-settings":
      await openSystemSettings();
      break;
    case "file-convert-md":
      await convertFilePicker();
      break;
    case "file-save":
      await saveActive(false);
      break;
    case "file-save-as":
      await saveActive(true);
      break;
    case "file-close":
      await closeTab(activeId.value);
      break;
    case "file-clear-recent":
      await clearRecent();
      await refreshRecent();
      break;
    case "edit-undo":
      runEditorAction("undo");
      break;
    case "edit-redo":
      runEditorAction("redo");
      break;
    case "edit-find":
      runEditorAction("actions.find");
      break;
    case "view-edit":
      setViewMode("edit");
      break;
    case "view-preview":
      setViewMode("preview");
      break;
    case "view-split":
      setViewMode("split");
      break;
    case "view-cycle-mode":
      if (active.value) {
        editorRef.value?.trigger("closeFindWidget");
        cycleViewMode();
      }
      break;
  }
}

const unlistens: UnlistenFn[] = [];

function onWinReposition() {
  if (tableOpen.value) placeTablePicker();
  if (sourceMenuOpen.value) placeSourceMenu();
  if (recentOpen.value) placeRecentMenu();
}

async function focusAppWindow() {
  try {
    const w = getCurrentWindow();
    await w.unminimize();
    await w.show();
    await w.setFocus();
  } catch {
    /* browser / non-Tauri */
  }
}

async function openPathsFromOs(paths: string[]) {
  if (!paths.length) return;
  await focusAppWindow();
  const dirs: string[] = [];
  const rest: string[] = [];
  for (const p of paths) {
    if (!p) continue;
    try {
      if (await pathIsDir(p)) dirs.push(p);
      else rest.push(p);
    } catch {
      rest.push(p);
    }
  }
  for (const dir of dirs) {
    await importFolder(dir);
  }
  if (!rest.length) return;
  const supported = rest.filter((p) => isSupportedPath(p));
  const others = rest.filter((p) => !isSupportedPath(p));
  for (const path of supported) {
    await openPath(path);
  }
  const convertible: string[] = [];
  const rejected: string[] = [];
  for (const path of others) {
    try {
      if (await convertIsSupported(path)) convertible.push(path);
      else rejected.push(path);
    } catch {
      rejected.push(path);
    }
  }
  // One convert at a time (modal lock); take the first convertible.
  if (convertible.length) {
    await beginConvert(convertible[0]);
  }
  if (rejected.length && !supported.length && !convertible.length) {
    statusError.value = t("unsupportedType", {
      name: rejected[0].split(/[/\\]/).pop() || rejected[0],
    });
  } else if (rejected.length || convertible.length > 1) {
    const n = rejected.length + Math.max(0, convertible.length - 1);
    if (n > 0) statusError.value = t("dropPartialUnsupported", { n });
  }
}

const fileDropActive = ref(false);
const fileDropHint = ref<"files" | "folder">("files");

async function markDropHint(paths: string[]) {
  fileDropHint.value = "files";
  for (const p of paths) {
    try {
      if (await pathIsDir(p)) {
        fileDropHint.value = "folder";
        return;
      }
    } catch {
      /* ignore */
    }
  }
}

onMounted(async () => {
  await refreshRecent();
  await loadSettings();
  window.addEventListener("mousemove", onSplitMove);
  window.addEventListener("mouseup", onSplitUp);
  window.addEventListener("resize", onWinReposition);
  window.addEventListener("keydown", onAppKeydown, true);
  unlistens.push(
    await listen<string>("menu-action", (e) => {
      void handleMenu(e.payload);
    }),
  );
  unlistens.push(
    await listen<{ phase?: string; message?: string; tool?: string; status?: string }>(
      "convert-progress",
      (e) => {
        const p = e.payload ?? {};
        if (p.phase) convertPhase.value = p.phase;
        if (p.message) convertDetail.value = p.message;
        if (p.tool) convertTool.value = p.tool;
      },
    ),
  );
  unlistens.push(
    await listen<string[]>("open-files", (e) => {
      void openPathsFromOs(e.payload ?? []);
    }),
  );
  try {
    const unlistenDrop = await getCurrentWindow().onDragDropEvent((event) => {
      const kind = event.payload.type;
      if (kind === "enter") {
        fileDropActive.value = event.payload.paths.length > 0;
        void markDropHint(event.payload.paths ?? []);
        return;
      }
      if (kind === "over") return;
      if (kind === "leave") {
        fileDropActive.value = false;
        return;
      }
      if (kind === "drop") {
        fileDropActive.value = false;
        const paths = event.payload.paths ?? [];
        if (paths.length) void openPathsFromOs(paths);
      }
    });
    unlistens.push(unlistenDrop);
  } catch {
    /* browser / non-Tauri */
  }
  try {
    const pending = await takePendingOpens();
    await openPathsFromOs(pending);
  } catch {
    /* not in Tauri */
  }
});

onUnmounted(() => {
  window.removeEventListener("mousemove", onSplitMove);
  window.removeEventListener("mouseup", onSplitUp);
  window.removeEventListener("resize", onWinReposition);
  window.removeEventListener("keydown", onAppKeydown, true);
  for (const u of unlistens) void u();
  unlistens.length = 0;
});

watch(activeId, async () => {
  await nextTick();
  if (active.value) editorRef.value?.focus();
  void refreshSourceStaleBanner();
});

watch(
  () => active.value?.path,
  () => {
    void refreshSourceStaleBanner();
  },
);
</script>

<template>
  <div
    class="app"
    :class="{ 'app--file-drop': fileDropActive }"
    @click="recentOpen = false; charsOpen = false; tableOpen = false; sourceMenuOpen = false"
  >
    <div v-if="fileDropActive" class="file-drop-overlay" aria-hidden="true">
      {{ fileDropHint === "folder" ? t("dropFolderHint") : t("dropConvertHint") }}
    </div>

    <FormulaEditorModal
      :open="formulaOpen"
      :latex="formulaLatex"
      :mode="formulaMode"
      :editing="formulaEditing"
      @close="closeFormulaEditor"
      @confirm="onFormulaConfirm"
    />

    <ConvertModal
      :open="convertOpen"
      :tool="convertTool"
      :input-name="convertInputName"
      :output-path="convertOutputPath"
      :phase="convertPhase"
      :detail="convertDetail"
      :cancelling="convertCancelling"
      @cancel="onConvertCancel"
    />

    <div
      v-if="registerOpen"
      class="register-modal-backdrop"
      @click.self="cancelRegister"
    >
      <div class="register-modal" role="dialog" @keydown.escape.prevent="cancelRegister">
        <h2 class="register-modal-title">{{ t("generateAsset") }}</h2>
        <p class="register-modal-hint">{{ t("assetRemarkHint") }}</p>
        <label class="register-field">
          <span>{{ t("assetRemark") }}</span>
          <input
            ref="registerInputRef"
            v-model="registerRemark"
            type="text"
            :placeholder="t('assetRemarkPrompt')"
            :disabled="registerBusy"
            @keydown.enter.prevent="confirmRegister"
          />
        </label>
        <div class="register-filename">
          <span class="register-filename-label">{{ t("assetFileName") }}</span>
          <span class="register-filename-value" :title="registerPath">
            {{ fileNameFromPath(registerPath) }}
          </span>
        </div>
        <label class="register-field" style="margin-top: 12px">
          <span>{{ t("detailsSource") }}</span>
          <input
            v-model="registerSource"
            type="text"
            :placeholder="t('assetSourcePrompt')"
            :disabled="registerBusy"
          />
        </label>
        <p class="register-modal-hint" style="margin-top: 6px">{{ t("assetSourceHint") }}</p>
        <div class="register-actions">
          <button type="button" :disabled="registerBusy" @click="cancelRegister">
            {{ t("assetRegisterCancel") }}
          </button>
          <button
            type="button"
            class="register-primary"
            :disabled="registerBusy"
            @click="confirmRegister"
          >
            {{ t("assetRegisterConfirm") }}
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="inboxOpen"
      class="register-modal-backdrop"
      @click.self="inboxOpen = false"
    >
      <div
        class="register-modal inbox-modal"
        role="dialog"
        @keydown.escape.prevent="inboxOpen = false"
      >
        <header class="inbox-head">
          <div class="inbox-head-copy">
            <h2 class="register-modal-title">{{ t("browseInbox") }}</h2>
            <p class="inbox-path" :title="settingsDraft.inboxDir || ''">
              {{ settingsDraft.inboxDir || "~/Downloads/MonolithInbox" }}
            </p>
          </div>
          <div class="inbox-head-actions">
            <span class="inbox-count">{{ t("inboxCount", { shown: inboxVisible.length, total: inboxEntries.length }) }}</span>
            <button type="button" class="inbox-ghost" :disabled="inboxBusy" @click="refreshInbox">
              {{ t("inboxRefresh") }}
            </button>
            <button type="button" class="settings-close" :aria-label="t('cancel')" @click="inboxOpen = false">
              ✕
            </button>
          </div>
        </header>
        <div class="inbox-toolbar">
          <input
            v-model="inboxQuery"
            type="search"
            class="inbox-search"
            :placeholder="t('inboxSearchPlaceholder')"
            :disabled="inboxBusy"
          />
          <div class="inbox-seg" role="tablist">
            <button
              type="button"
              :class="{ active: inboxFilter === 'all' }"
              @click="inboxFilter = 'all'"
            >
              {{ t("inboxFilterAll") }}
            </button>
            <button
              type="button"
              :class="{ active: inboxFilter === 'unregistered' }"
              @click="inboxFilter = 'unregistered'"
            >
              {{ t("inboxUnregistered") }}
            </button>
            <button
              type="button"
              :class="{ active: inboxFilter === 'registered' }"
              @click="inboxFilter = 'registered'"
            >
              {{ t("inboxRegistered") }}
            </button>
          </div>
        </div>
        <div v-if="inboxBusy" class="inbox-empty">…</div>
        <div v-else-if="!inboxEntries.length" class="inbox-empty">{{ t("browseInboxEmpty") }}</div>
        <div v-else-if="!inboxVisible.length" class="inbox-empty">{{ t("inboxNoMatch") }}</div>
        <div v-else class="inbox-table-wrap">
          <table class="inbox-table">
            <colgroup>
              <col class="inbox-col-name" />
              <col class="inbox-col-mtime" />
              <col class="inbox-col-source" />
              <col class="inbox-col-kind" />
              <col class="inbox-col-status" />
            </colgroup>
            <thead>
              <tr>
                <th>
                  <button type="button" @click="toggleInboxSort('name')">
                    {{ t("inboxColName") }}
                    <span v-if="inboxSortKey === 'name'" class="inbox-sort">{{ inboxSortDir === "asc" ? "↑" : "↓" }}</span>
                  </button>
                </th>
                <th>
                  <button type="button" @click="toggleInboxSort('mtime')">
                    {{ t("inboxColMtime") }}
                    <span v-if="inboxSortKey === 'mtime'" class="inbox-sort">{{ inboxSortDir === "asc" ? "↑" : "↓" }}</span>
                  </button>
                </th>
                <th>
                  <button type="button" @click="toggleInboxSort('source')">
                    {{ t("inboxColSource") }}
                    <span v-if="inboxSortKey === 'source'" class="inbox-sort">{{ inboxSortDir === "asc" ? "↑" : "↓" }}</span>
                  </button>
                </th>
                <th>
                  <button type="button" @click="toggleInboxSort('kind')">
                    {{ t("inboxColKind") }}
                    <span v-if="inboxSortKey === 'kind'" class="inbox-sort">{{ inboxSortDir === "asc" ? "↑" : "↓" }}</span>
                  </button>
                </th>
                <th>
                  <button type="button" @click="toggleInboxSort('registered')">
                    {{ t("inboxColStatus") }}
                    <span v-if="inboxSortKey === 'registered'" class="inbox-sort">{{ inboxSortDir === "asc" ? "↑" : "↓" }}</span>
                  </button>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="entry in inboxVisible"
                :key="entry.path"
                tabindex="0"
                @click="openInboxEntry(entry)"
                @keydown.enter.prevent="openInboxEntry(entry)"
              >
                <td class="inbox-name" :title="entry.path">{{ entry.name }}</td>
                <td class="inbox-mtime">{{ formatInboxMtime(entry.mtime) }}</td>
                <td class="inbox-source" :title="entry.source || ''">{{ inboxSourceLabel(entry.source) }}</td>
                <td>
                  <span class="inbox-kind" :class="'inbox-kind--' + entry.kind">{{ inboxKindLabel(entry.kind) }}</span>
                </td>
                <td>
                  <span
                    class="inbox-badge"
                    :class="entry.registered ? 'inbox-badge--yes' : 'inbox-badge--no'"
                  >
                    {{ entry.registered ? t("inboxRegistered") : t("inboxUnregistered") }}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <div
      v-if="settingsOpen"
      class="register-modal-backdrop"
      @click.self="settingsOpen = false"
    >
      <div
        class="register-modal settings-modal"
        role="dialog"
        @keydown.escape.prevent="settingsOpen = false"
      >
        <header class="settings-head">
          <h2 class="register-modal-title">{{ t("systemSettings") }}</h2>
          <button type="button" class="settings-close" :aria-label="t('cancel')" @click="settingsOpen = false">
            ✕
          </button>
        </header>
        <div class="settings-body">
          <nav class="settings-nav" aria-label="settings">
            <button
              type="button"
              :class="{ active: settingsTab === 'dirs' }"
              @click="settingsTab = 'dirs'"
            >
              {{ t("settingsTabDirs") }}
            </button>
            <button
              type="button"
              :class="{ active: settingsTab === 'clipper' }"
              @click="settingsTab = 'clipper'"
            >
              {{ t("settingsTabClipper") }}
            </button>
            <button
              type="button"
              :class="{ active: settingsTab === 'mcp' }"
              @click="settingsTab = 'mcp'"
            >
              {{ t("settingsTabMcp") }}
            </button>
            <button
              type="button"
              :class="{ active: settingsTab === 'skill' }"
              @click="settingsTab = 'skill'"
            >
              {{ t("settingsTabSkill") }}
            </button>
            <button
              type="button"
              :class="{ active: settingsTab === 'shortcuts' }"
              @click="settingsTab = 'shortcuts'"
            >
              {{ t("settingsTabShortcuts") }}
            </button>
            <button
              type="button"
              :class="{ active: settingsTab === 'about' }"
              @click="settingsTab = 'about'"
            >
              {{ t("settingsTabAbout") }}
            </button>
          </nav>
          <div class="settings-pane">
            <div v-if="settingsBusy && !settingsView" class="inbox-empty">…</div>
            <template v-else>
              <section v-show="settingsTab === 'dirs'" class="settings-panel">
                <p class="settings-lead">{{ t("settingsDirsHint") }}</p>
                <label class="settings-field">
                  <span>{{ t("bgEditTitle") }}</span>
                  <div class="settings-row">
                    <input type="color" :value="editBg" :disabled="settingsBusy" @input="onEditBg" />
                  </div>
                </label>
                <label class="settings-field">
                  <span>{{ t("bgPreviewTitle") }}</span>
                  <div class="settings-row">
                    <input type="color" :value="previewBg" :disabled="settingsBusy" @input="onPreviewBg" />
                  </div>
                </label>
                <label class="settings-field">
                  <span>{{ t("settingsInboxDir") }}</span>
                  <p class="settings-lead">{{ t("settingsInboxHint") }}</p>
                  <div class="settings-row">
                    <input v-model="settingsDraft.inboxDir" type="text" :disabled="settingsBusy" />
                    <button type="button" :disabled="settingsBusy" @click="pickSettingsDir('inboxDir')">
                      {{ t("settingsBrowse") }}
                    </button>
                    <button type="button" :disabled="settingsBusy" @click="resetInboxDefault">
                      {{ t("settingsResetInbox") }}
                    </button>
                  </div>
                </label>
                <label class="settings-field">
                  <span>{{ t("settingsOpenDir") }}</span>
                  <div class="settings-row">
                    <input
                      v-model="settingsDraft.defaultOpenDir"
                      type="text"
                      :placeholder="t('settingsOpenDirHint')"
                      :disabled="settingsBusy"
                    />
                    <button type="button" :disabled="settingsBusy" @click="pickSettingsDir('defaultOpenDir')">
                      {{ t("settingsBrowse") }}
                    </button>
                  </div>
                </label>
                <label class="settings-field">
                  <span>{{ t("settingsSaveDir") }}</span>
                  <div class="settings-row">
                    <input
                      v-model="settingsDraft.defaultSaveDir"
                      type="text"
                      :placeholder="t('settingsSaveDirHint')"
                      :disabled="settingsBusy"
                    />
                    <button type="button" :disabled="settingsBusy" @click="pickSettingsDir('defaultSaveDir')">
                      {{ t("settingsBrowse") }}
                    </button>
                  </div>
                </label>
                <div class="settings-pane-actions">
                  <button
                    type="button"
                    class="register-primary"
                    :disabled="settingsBusy || !settingsDraft.inboxDir.trim()"
                    @click="saveSystemSettings"
                  >
                    {{ t("settingsSave") }}
                  </button>
                </div>
              </section>

              <section v-show="settingsTab === 'clipper'" class="settings-panel">
                <p class="settings-lead">{{ t("settingsClipperHint") }}</p>
                <dl v-if="settingsView" class="settings-readonly">
                  <div>
                    <dt>{{ t("settingsClipperDir") }}</dt>
                    <dd>{{ settingsView.clipperReleaseDir }}</dd>
                  </div>
                </dl>
                <div class="settings-pane-actions">
                  <button type="button" class="register-primary" :disabled="settingsBusy" @click="installWebClipper">
                    {{ t("installClipper") }}
                  </button>
                </div>
              </section>

              <section v-show="settingsTab === 'mcp'" class="settings-panel">
                <p class="settings-lead">{{ t("settingsMcpHint") }}</p>
                <dl v-if="settingsView" class="settings-readonly">
                  <div>
                    <dt>{{ t("settingsMcpBin") }}</dt>
                    <dd>{{ settingsView.mcpBinaryPath }}</dd>
                  </div>
                </dl>
                <div v-if="mcpDiscoverBusy" class="inbox-empty">{{ t("installMcpScanning") }}</div>
                <div v-else class="mcp-list">
                  <label
                    v-for="agent in mcpDiscovered"
                    :key="agent.id"
                    class="mcp-check"
                    :class="{ 'mcp-check--disabled': !agent.installable }"
                  >
                    <input
                      v-if="agent.installable"
                      v-model="mcpSelected[agent.id]"
                      type="checkbox"
                      :disabled="mcpInstallBusy"
                    />
                    <input v-else type="checkbox" disabled :checked="false" />
                    <span class="mcp-check-label">
                      {{ agent.name }}
                      <span v-if="agent.alreadyHasMonolith" class="mcp-badge">{{ t("installMcpAlready") }}</span>
                      <span v-else-if="!agent.installable" class="mcp-badge mcp-badge--warn">
                        {{ t("installMcpUnsupported") }}
                      </span>
                    </span>
                    <span v-if="agent.skipReason" class="mcp-check-hint">{{ agent.skipReason }}</span>
                    <span v-else-if="agent.mcpConfigPath" class="mcp-check-hint">{{ agent.mcpConfigPath }}</span>
                  </label>
                  <div v-if="!mcpDiscovered.length" class="inbox-empty">{{ t("installMcpEmpty") }}</div>
                </div>
                <div class="settings-pane-actions">
                  <button type="button" :disabled="mcpInstallBusy || mcpDiscoverBusy" @click="refreshMcpAgents">
                    {{ t("installMcpRefresh") }}
                  </button>
                  <button
                    type="button"
                    class="register-primary"
                    :disabled="mcpInstallBusy || mcpDiscoverBusy || !mcpSelectedIds.length"
                    @click="runMcpInstall"
                  >
                    {{ t("installMcpRun") }}
                  </button>
                </div>
              </section>

              <section v-show="settingsTab === 'skill'" class="settings-panel">
                <p class="settings-lead">{{ t("settingsSkillHint") }}</p>
                <dl v-if="skillView" class="settings-readonly">
                  <div>
                    <dt>{{ t("settingsSkillPath") }}</dt>
                    <dd>{{ skillView.path }}</dd>
                  </div>
                  <div v-if="skillView.reloadTargets.length">
                    <dt>{{ t("settingsSkillTargets") }}</dt>
                    <dd>
                      <ul class="settings-target-list">
                        <li v-for="p in skillView.reloadTargets" :key="p">{{ p }}</li>
                      </ul>
                    </dd>
                  </div>
                </dl>
                <div class="settings-pane-actions">
                  <button
                    type="button"
                    class="register-primary"
                    :disabled="skillBusy || !skillView"
                    @click="openSkillInMonolith"
                  >
                    {{ t("settingsSkillOpen") }}
                  </button>
                  <button type="button" :disabled="skillBusy || !skillView" @click="reloadSkill">
                    {{ t("settingsSkillReload") }}
                  </button>
                </div>
              </section>

              <section v-show="settingsTab === 'shortcuts'" class="settings-panel">
                <p class="settings-lead">{{ t("settingsShortcutsHint") }}</p>
                <table class="settings-shortcut-table">
                  <tbody>
                    <tr
                      v-for="row in shortcutRows"
                      :key="row.keys"
                      :class="{ 'settings-shortcut-table--muted': !!row.note }"
                    >
                      <td class="settings-shortcut-keys">{{ row.keys }}</td>
                      <td>{{ row.action }}</td>
                    </tr>
                  </tbody>
                </table>
              </section>

              <section v-show="settingsTab === 'about'" class="settings-panel">
                <dl v-if="settingsView" class="settings-readonly">
                  <div>
                    <dt>{{ t("settingsAssetsDb") }}</dt>
                    <dd>{{ settingsView.assetsDbPath }}</dd>
                  </div>
                  <div>
                    <dt>{{ t("settingsAppData") }}</dt>
                    <dd>{{ settingsView.appDataDir }}</dd>
                  </div>
                  <div>
                    <dt>{{ t("settingsMcpBin") }}</dt>
                    <dd>{{ settingsView.mcpBinaryPath }}</dd>
                  </div>
                  <div>
                    <dt>{{ t("settingsClipperDir") }}</dt>
                    <dd>{{ settingsView.clipperReleaseDir }}</dd>
                  </div>
                  <div v-if="skillView">
                    <dt>{{ t("settingsSkillPath") }}</dt>
                    <dd>{{ skillView.path }}</dd>
                  </div>
                </dl>
              </section>
            </template>
          </div>
        </div>
      </div>
    </div>

    <!-- File / Assets / View toolbar (icon-only; tooltips follow OS locale via i18n) -->
    <header class="toolbar toolbar-file" @click.stop>
      <div class="tb-seg">
        <div class="tb-group">
          <button
            type="button"
            class="tb-icon-btn"
            :title="`${t('new')} (⌘N)`"
            :aria-label="`${t('new')} (⌘N)`"
            @click="createUntitled"
          >
            <ToolbarIcon name="filePlus" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="`${t('open')} (⌘O)`"
            :aria-label="`${t('open')} (⌘O)`"
            @click="openFile"
          >
            <ToolbarIcon name="folderOpen" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="t('browseInboxTitle')"
            :aria-label="t('browseInbox')"
            @click="openInboxBrowser"
          >
            <ToolbarIcon name="inbox" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="t('folderImportTitle')"
            :aria-label="t('folderImport')"
            @click="importFolder()"
          >
            <ToolbarIcon name="folderImport" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="t('convertToMd')"
            :aria-label="t('convertToMd')"
            @click="convertFilePicker"
          >
            <ToolbarIcon name="convert" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="`${t('save')} (⌘S)`"
            :aria-label="`${t('save')} (⌘S)`"
            @click="saveActive(false)"
          >
            <ToolbarIcon name="save" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="`${t('saveAs')} (⇧⌘S)`"
            :aria-label="`${t('saveAs')} (⇧⌘S)`"
            @click="saveActive(true)"
          >
            <ToolbarIcon name="saveAs" />
          </button>
          <div class="recent-wrap">
            <button
              ref="recentBtnRef"
              type="button"
              class="tb-icon-btn tb-icon-btn--menu"
              :title="t('recent')"
              :aria-label="t('recent')"
              @click.stop="toggleRecentMenu"
            >
              <ToolbarIcon name="history" />
              <ToolbarIcon name="chevronDown" :size="12" />
            </button>
          </div>
        </div>
      </div>

      <div class="tb-seg">
        <div class="tb-group">
          <button
            type="button"
            class="tb-icon-btn"
            :title="t('generateAssetTitle')"
            :aria-label="t('generateAsset')"
            @click="generateAsset"
          >
            <ToolbarIcon name="assetPlus" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :title="t('saveNewVersionTitle')"
            :aria-label="t('saveNewVersion')"
            @click="saveNewVersion"
          >
            <ToolbarIcon name="version" />
          </button>
        </div>
      </div>

      <div class="tb-seg tb-seg--trail">
        <div class="tb-group">
          <button
            type="button"
            class="tb-icon-btn"
            :disabled="!hasTab"
            :class="{ active: active?.viewMode === 'edit' }"
            :title="t('edit')"
            :aria-label="t('edit')"
            @click="setViewMode('edit')"
          >
            <ToolbarIcon name="edit" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :disabled="!hasTab"
            :class="{ active: active?.viewMode === 'preview' }"
            :title="t('preview')"
            :aria-label="t('preview')"
            @click="setViewMode('preview')"
          >
            <ToolbarIcon name="eye" />
          </button>
          <button
            type="button"
            class="tb-icon-btn"
            :disabled="!hasTab"
            :class="{ active: active?.viewMode === 'split' }"
            :title="t('split')"
            :aria-label="t('split')"
            @click="setViewMode('split')"
          >
            <ToolbarIcon name="columns" />
          </button>
          <div class="recent-wrap">
            <button
              ref="sourceBtnRef"
              type="button"
              class="tb-icon-btn tb-icon-btn--menu"
              :disabled="!canViewSource"
              :title="sourceProvenance?.path || t('viewSource')"
              :aria-label="t('viewSource')"
              @click.stop="toggleSourceMenu"
            >
              <ToolbarIcon name="fileSearch" />
              <ToolbarIcon name="chevronDown" :size="12" />
            </button>
          </div>
        </div>
      </div>

      <div class="tb-seg tb-seg--settings">
        <div class="tb-group">
          <button
            type="button"
            class="tb-icon-btn"
            :title="t('systemSettingsTitle')"
            :aria-label="t('systemSettings')"
            @click="openSystemSettings"
          >
            <ToolbarIcon name="settings" />
          </button>
        </div>
      </div>
    </header>

    <div class="workbench" :class="{ 'workbench--resizing': resizingAssets }">
      <div
        class="asset-pane"
        :class="{ 'asset-pane--collapsed': assetsCollapsed }"
        :style="{ width: assetPaneWidth + 'px', flexBasis: assetPaneWidth + 'px' }"
      >
        <button
          v-if="assetsCollapsed"
          type="button"
          class="asset-rail"
          :title="t('expandAssets')"
          :aria-label="t('expandAssets')"
          @click="toggleAssetsSidebar"
        >
          <span class="asset-rail-label">{{ t("assets") }}</span>
          <span class="asset-rail-chevron">›</span>
        </button>
        <AssetBrowser
          v-show="!assetsCollapsed"
          ref="assetBrowserRef"
          class="asset-side"
          :selected-asset-id="selectedAssetId"
          @open-asset="openAssetFromTree"
          @open-path="openPath"
          @select-asset="selectedAssetId = $event"
          @error="statusError = $event"
          @status="statusError = $event"
          @request-save-new-version="saveNewVersionForAsset"
          @collapse="toggleAssetsSidebar"
        />
        <div
          v-if="!assetsCollapsed"
          class="asset-resizer"
          :title="t('resizeAssets')"
          @mousedown="onAssetResizeDown"
        />
      </div>
      <div class="editor-col">
    <!-- Format toolbar -->
    <header class="toolbar toolbar-format" @click.stop>
      <div class="tb-group">
        <button
          type="button"
          class="tb-icon-btn"
          :title="t('undo')"
          :aria-label="t('undo')"
          :disabled="!showEdit"
          @click="runEditorAction('undo')"
        >
          <ToolbarIcon name="undo" />
        </button>
        <button
          type="button"
          class="tb-icon-btn"
          :title="t('redo')"
          :aria-label="t('redo')"
          :disabled="!showEdit"
          @click="runEditorAction('redo')"
        >
          <ToolbarIcon name="redo" />
        </button>
        <button
          type="button"
          class="tb-icon-btn"
          :title="t('find')"
          :aria-label="t('find')"
          :disabled="!showEdit"
          @click="runEditorAction('actions.find')"
        >
          <ToolbarIcon name="search" />
        </button>
        <button
          type="button"
          class="tb-icon-btn"
          :title="t('formatSqlTitle')"
          :aria-label="t('formatSql')"
          :disabled="!showEdit || !isSql"
          @click="formatSql"
        >
          <ToolbarIcon name="braces" />
        </button>
      </div>

      <div class="tb-group">
        <button
          type="button"
          class="tb-icon-btn"
          :title="t('indentMoreTitle')"
          :aria-label="t('indentMoreTitle')"
          :disabled="!showEdit"
          @click="indentLines"
        >
          <ToolbarIcon name="indent" />
        </button>
        <button
          type="button"
          class="tb-icon-btn"
          :title="t('indentLessTitle')"
          :aria-label="t('indentLessTitle')"
          :disabled="!showEdit"
          @click="outdentLines"
        >
          <ToolbarIcon name="outdent" />
        </button>
      </div>

      <div class="tb-group">
        <select
          class="tb-select"
          :value="fontSize"
          :disabled="!showEdit"
          :title="t('fontSizeTitle')"
          :aria-label="t('fontSizeTitle')"
          @change="onFontSizeChange"
        >
          <option v-for="s in FONT_SIZES" :key="s" :value="s">{{ s }}px</option>
        </select>
      </div>

      <div class="tb-group">
        <select
          class="tb-select tb-select-heading"
          :disabled="!(canEditMd || showEdit)"
          :title="t('heading')"
          :aria-label="t('heading')"
          :value="headingSelect"
          @change="onHeadingChange"
        >
          <option value="">{{ t("headingPlaceholder") }}</option>
          <option value="0">{{ t("paragraph") }}</option>
          <option value="1">{{ t("h1") }}</option>
          <option value="2">{{ t("h2") }}</option>
          <option value="3">{{ t("h3") }}</option>
          <option value="4">{{ t("h4") }}</option>
          <option value="5">{{ t("h5") }}</option>
          <option value="6">{{ t("h6") }}</option>
        </select>
      </div>

      <!-- MD format groups: B I S | code | link/img | lists | quote/table/hr -->
      <div class="tb-group md-helpers">
        <button type="button" class="tb-icon-btn" :title="`${t('bold')} (⌘B)`" :aria-label="t('bold')" :disabled="!canEditMd" @click="mdBold">
          <ToolbarIcon name="bold" />
        </button>
        <button type="button" class="tb-icon-btn" :title="`${t('italic')} (⌘I)`" :aria-label="t('italic')" :disabled="!canEditMd" @click="mdItalic">
          <ToolbarIcon name="italic" />
        </button>
        <button type="button" class="tb-icon-btn" :title="`${t('strike')} (⌘D)`" :aria-label="t('strike')" :disabled="!canEditMd" @click="mdStrike">
          <ToolbarIcon name="strike" />
        </button>
      </div>

      <div class="tb-group md-helpers">
        <button type="button" class="tb-icon-btn" :title="t('inlineCode')" :aria-label="t('inlineCode')" :disabled="!canEditMd" @click="mdCode">
          <ToolbarIcon name="code" />
        </button>
        <button type="button" class="tb-icon-btn" :title="`${t('codeBlock')} (⇧⌘K)`" :aria-label="t('codeBlock')" :disabled="!canEditMd" @click="mdCodeBlock">
          <ToolbarIcon name="codeBlock" />
        </button>
      </div>

      <div class="tb-group md-helpers">
        <button type="button" class="tb-icon-btn" :title="`${t('link')} (⌘L)`" :aria-label="t('link')" :disabled="!canEditMd" @click="mdLink">
          <ToolbarIcon name="link" />
        </button>
        <button type="button" class="tb-icon-btn" :title="t('image')" :aria-label="t('image')" :disabled="!canEditMd" @click="mdImage">
          <ToolbarIcon name="image" />
        </button>
      </div>

      <div class="tb-group md-helpers">
        <button type="button" class="tb-icon-btn" :title="t('ul')" :aria-label="t('ul')" :disabled="!canEditMd" @click="mdUl">
          <ToolbarIcon name="list" />
        </button>
        <button type="button" class="tb-icon-btn" :title="t('ol')" :aria-label="t('ol')" :disabled="!canEditMd" @click="mdOl">
          <ToolbarIcon name="listOrdered" />
        </button>
        <button type="button" class="tb-icon-btn" :title="t('task')" :aria-label="t('task')" :disabled="!canEditMd" @click="mdTask">
          <ToolbarIcon name="checkSquare" />
        </button>
      </div>

      <div class="tb-group md-helpers">
        <button type="button" class="tb-icon-btn" :title="t('quote')" :aria-label="t('quote')" :disabled="!canEditMd" @click="mdQuote">
          <ToolbarIcon name="quote" />
        </button>
        <div class="table-wrap">
          <button
            ref="tableBtnRef"
            type="button"
            class="tb-icon-btn tb-icon-btn--menu"
            :title="`${t('tableInsertTitle')} (⇧⌘T)`"
            :aria-label="t('table')"
            :disabled="!canEditMd"
            @click.stop="toggleTablePicker"
          >
            <ToolbarIcon name="table" />
            <ToolbarIcon name="chevronDown" :size="12" />
          </button>
        </div>
        <button type="button" class="tb-icon-btn" :title="t('hr')" :aria-label="t('hr')" :disabled="!canEditMd" @click="mdHr">
          <ToolbarIcon name="minus" />
        </button>
        <div class="chars-wrap">
          <button
            ref="charsBtnRef"
            type="button"
            class="tb-icon-btn tb-icon-btn--menu"
            :class="{ active: charsOpen }"
            :title="t('charsTitle')"
            :aria-label="t('charsMenu')"
            :disabled="!canEditMd"
            @click.stop="toggleCharsMenu"
          >
            <ToolbarIcon name="omega" />
            <ToolbarIcon name="chevronDown" :size="12" />
          </button>
        </div>
        <button
          type="button"
          class="tb-icon-btn"
          :class="{ active: formulaOpen }"
          :title="t('formulaTitle')"
          :aria-label="t('formula')"
          :disabled="!canEditMd"
          @click.stop="openFormulaEditor()"
        >
          <ToolbarIcon name="sigma" />
        </button>
      </div>
    </header>
    <div class="tabbar">
      <button
        v-for="tabItem in tabs"
        :key="tabItem.id"
        type="button"
        class="tab"
        :class="{ active: tabItem.id === activeId }"
        @click="setActive(tabItem.id)"
      >
        <span class="tab-title">{{ tabItem.title }}{{ tabItem.dirty ? " •" : "" }}</span>
        <span
          class="tab-close"
          :title="t('close')"
          :aria-label="t('close')"
          @click.stop="closeTab(tabItem.id)"
        >
          <ToolbarIcon name="x" :size="12" />
        </span>
      </button>
      <button
        type="button"
        class="tab-add"
        :title="t('new')"
        :aria-label="t('new')"
        @click="createUntitled"
      >
        <ToolbarIcon name="plus" :size="14" />
      </button>
    </div>

    <main class="main-pane">
      <div v-if="!active" class="empty-workbench">
        <p class="empty-workbench-title">{{ t("emptyWorkbench") }}</p>
        <p class="empty-workbench-hint">{{ t("emptyWorkbenchHint") }}</p>
      </div>
      <template v-else>
        <section
          v-show="showEdit"
          class="pane edit-pane"
          :style="
            isSplit
              ? { width: splitRatio * 100 + '%', flex: 'none', background: editBg }
              : { background: editBg }
          "
        >
          <MonacoEditor
            :key="active.id + '-edit'"
            ref="editorRef"
            :model-value="active.content"
            :language="active.language"
            :background="editBg"
            :font-size="fontSize"
            :highlight-line="isSplit && mdFocus === 'wysiwyg' ? active.cursorLine : 0"
            @update:model-value="onContent"
            @cursor="onCursor"
            @scroll="onEditScroll"
            @focus="onSourceFocus"
            @pointerdown="onSourcePointer"
            @wheel.passive="onSourcePointer"
          />
        </section>

        <div v-if="isSplit" class="splitter" @mousedown="onSplitDown" />

        <section
          v-show="showPreview"
          class="pane preview-pane"
          :style="
            isSplit
              ? { width: (1 - splitRatio) * 100 + '%', flex: 'none', background: previewBg }
              : { background: previewBg }
          "
        >
          <MarkdownWysiwyg
            v-if="useMdWysiwyg"
            :key="active.id + '-md-wysiwyg'"
            ref="mdWysiwygRef"
            :content="active.content"
            :highlight-line="isSplit ? active.cursorLine : 0"
            :background="previewBg"
            :foreground="previewFg"
            :muted="previewMuted"
            :border="previewBorder"
            :highlight-bg="previewHlBorder"
            :highlight-fg="previewHlFg"
            :placeholder="t('wysiwygPlaceholder')"
            @update:content="onWysiwygContent"
            @focus="onWysiwygFocus"
            @scroll="onMdPreviewScroll"
            @line-click="onMdLineClick"
            @edit-math="onEditMathFromPreview"
            @pointerdown="onMdPreviewPointer"
            @wheel.passive="onMdPreviewPointer"
          />
          <HtmlPreview
            v-else-if="kind === 'html'"
            :key="active.id + '-html'"
            ref="htmlPreviewRef"
            :content="active.content"
            :background="previewBg"
            @scroll="onMdPreviewScroll"
          />
          <TreePreview
            v-else-if="kind === 'json' || kind === 'xml'"
            :key="active.id + '-' + kind"
            ref="treePreviewRef"
            :content="active.content"
            :kind="kind"
            :background="previewBg"
            :foreground="previewFg"
            :muted="previewMuted"
            :border="previewBorder"
            @scroll="onMdPreviewScroll"
          />
          <MonacoEditor
            v-else
            :key="active.id + '-preview'"
            ref="previewEditorRef"
            :model-value="active.content"
            :language="active.language"
            read-only
            :background="previewBg"
            :highlight-line="active.cursorLine"
            @scroll="onPreviewEditorScroll"
          />
        </section>
      </template>
    </main>

    <footer class="statusbar">
      <template v-if="active">
        <span class="sb-path" :title="active.path || ''">{{ active.path || t("unsaved") }}</span>
        <span>{{ active.language }}</span>
        <span>{{ t("lnCol", { line: active.cursorLine, col: active.cursorCol }) }}</span>
        <span :class="{ dirty: active.dirty }">{{ active.dirty ? t("modified") : t("saved") }}</span>
        <span
          v-if="sourceProvenance?.stale"
          class="sb-stale"
          :title="sourceProvenance.path"
        >
          {{ t("sourceStale") }}
          <button
            type="button"
            class="sb-stale-btn tb-icon-btn"
            :title="t('sourceReconvert')"
            :aria-label="t('sourceReconvert')"
            @click="reconvertFromSource"
          >
            <ToolbarIcon name="reconvert" :size="13" />
          </button>
        </span>
        <span v-else-if="sourceProvenance && !sourceProvenance.exists" class="sb-stale">
          {{ t("sourceMissing") }}
        </span>
      </template>
      <span v-else class="sb-path">{{ t("emptyWorkbench") }}</span>
      <span v-if="statusError" class="sb-err">{{ statusError }}</span>
    </footer>
      </div>
    </div>

    <!-- Teleport：避免被工具栏 overflow 裁切 -->
    <Teleport to="body">
      <div
        v-if="recentOpen"
        class="recent-menu recent-menu-flyout"
        :style="recentMenuStyle"
        @click.stop
      >
        <button
          v-for="p in recent"
          :key="p"
          type="button"
          class="recent-item"
          :title="p"
          @click="openPath(p); recentOpen = false"
        >
          {{ p.split(/[/\\]/).pop() }}
          <span class="recent-path">{{ p }}</span>
        </button>
        <div v-if="!recent.length" class="recent-empty">{{ t("recentEmpty") }}</div>
        <button
          v-if="recent.length"
          type="button"
          class="recent-clear"
          @click="clearRecent().then(refreshRecent); recentOpen = false"
        >
          {{ t("clearRecent") }}
        </button>
      </div>
      <div
        v-if="charsOpen"
        class="chars-menu chars-menu-flyout"
        :style="charsMenuStyle"
        @click.stop
      >
        <button
          v-for="c in mdChars"
          :key="c.label + c.title"
          type="button"
          class="char-item"
          :title="c.title"
          @click="insertMdChar(c.text)"
        >
          {{ c.label }}
        </button>
        <button
          type="button"
          class="char-item"
          :title="t('softBreak')"
          @click="insertMdChar('  \n')"
        >
          ↵
        </button>
        <button type="button" class="char-item" title="NBSP" @click="insertMdChar('\u00A0')">
          NBSP
        </button>
        <button type="button" class="char-item" title="ZWSP" @click="insertMdChar('\u200B')">
          ZWSP
        </button>
      </div>
      <div
        v-if="tableOpen"
        class="table-picker"
        :style="tablePickerStyle"
        @click.stop
        @mouseleave="tableHover = { rows: 0, cols: 0 }"
      >
        <div class="table-picker-label">
          {{
            tableHover.rows
              ? t("tablePick", { rows: tableHover.rows, cols: tableHover.cols })
              : t("tablePickEmpty")
          }}
        </div>
        <div class="table-grid" :style="{ '--cols': String(TABLE_PICK_MAX) }">
          <button
            v-for="idx in TABLE_PICK_MAX * TABLE_PICK_MAX"
            :key="idx"
            type="button"
            class="table-cell"
            :class="{
              on:
                tableHover.rows > 0 &&
                tableCellRow(idx) <= tableHover.rows &&
                tableCellCol(idx) <= tableHover.cols,
            }"
            :aria-label="`${tableCellRow(idx)}行 ${tableCellCol(idx)}列`"
            @mouseenter="onTableCellEnter(tableCellRow(idx), tableCellCol(idx))"
            @click="insertMdTable(tableCellRow(idx), tableCellCol(idx))"
          ></button>
        </div>
        <p class="table-picker-hint">{{ t("tablePickHint") }}</p>
      </div>
      <div
        v-if="sourceMenuOpen && canViewSource"
        class="source-menu-flyout"
        :style="sourceMenuStyle"
        @click.stop
      >
        <button type="button" class="source-menu-item" @click="openSourceInOs">
          {{ sourceIsUrl ? t("openSourceInBrowser") : t("compareSourceOpen") }}
        </button>
        <button
          v-if="!sourceIsUrl"
          type="button"
          class="source-menu-item"
          @click="revealSourceInOs"
        >
          {{ t("ctxReveal") }}
        </button>
      </div>
      <div
        v-if="brokenIndexOpen && brokenIndexAsset"
        class="broken-index-backdrop"
        @click.self="closeBrokenIndex"
      >
        <div
          class="register-modal"
          role="dialog"
          @keydown.escape.prevent="closeBrokenIndex"
        >
          <h2 class="register-modal-title">{{ t("assetIndexBrokenTitle") }}</h2>
          <p class="register-modal-hint">{{ t("assetIndexBrokenHint") }}</p>
          <div class="register-filename">
            <span class="register-filename-label">{{ t("assetIndexBrokenPath") }}</span>
            <span
              class="register-filename-value"
              :title="brokenIndexAsset.absolutePath || ''"
            >
              {{ brokenIndexAsset.absolutePath || "" }}
            </span>
          </div>
          <div v-if="brokenIndexAsset.sourcePath" class="register-filename">
            <span class="register-filename-label">{{ t("assetIndexBrokenSource") }}</span>
            <span
              class="register-filename-value"
              :title="brokenIndexAsset.sourcePath"
            >
              {{ brokenIndexAsset.sourcePath }}
              <template v-if="!brokenIndexAsset.sourceExists">
                {{ t("sourceMissing") }}
              </template>
            </span>
          </div>
          <div class="broken-index-actions">
            <button
              type="button"
              class="register-primary"
              :disabled="brokenIndexBusy"
              @click="brokenIndexRelocate"
            >
              {{ t("assetIndexRelocate") }}
            </button>
            <button
              type="button"
              :disabled="brokenIndexBusy || !brokenIndexAsset.sourceExists"
              :title="
                brokenIndexAsset.sourceExists
                  ? t('assetIndexRebuild')
                  : t('assetIndexRebuildNeedSource')
              "
              @click="brokenIndexRebuild"
            >
              {{ t("assetIndexRebuild") }}
            </button>
            <button
              type="button"
              class="broken-index-danger"
              :disabled="brokenIndexBusy"
              @click="brokenIndexDelete"
            >
              {{ t("assetIndexDelete") }}
            </button>
            <button type="button" :disabled="brokenIndexBusy" @click="closeBrokenIndex">
              {{ t("cancel") }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.app {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-0);
}

.app--file-drop {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}

.file-drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  background: rgba(18, 21, 26, 0.72);
  color: var(--text-1);
  font-size: var(--text-title);
  font-weight: var(--fw-semibold);
  letter-spacing: 0.02em;
}

.register-modal-backdrop {
  position: absolute;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(8, 10, 14, 0.55);
  -webkit-app-region: no-drag;
}

.register-modal.inbox-modal {
  width: min(1120px, calc(100% - 56px));
  height: min(86vh, 760px);
  max-height: calc(100% - 48px);
  padding: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.inbox-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--hairline);
  flex: 0 0 auto;
}
.inbox-head-copy {
  min-width: 0;
}
.inbox-head .register-modal-title {
  margin: 0 0 6px;
  font-size: 18px;
  letter-spacing: 0.01em;
}
.inbox-path {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-3);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.inbox-head-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
}
.inbox-count {
  font-variant-numeric: tabular-nums;
  font-size: var(--text-sm);
  color: var(--text-3);
  padding-right: 4px;
}
.inbox-ghost {
  height: 32px;
  padding: 0 12px;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: var(--bg-2);
  color: var(--text-1);
  cursor: pointer;
}
.inbox-ghost:disabled {
  opacity: 0.5;
  cursor: default;
}
.inbox-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 24px;
  flex-wrap: nowrap;
}
.inbox-search {
  flex: 1 1 auto;
  min-width: 200px;
  height: 36px;
  padding: 0 14px;
  border: 1px solid var(--hairline);
  border-radius: 8px;
  background: var(--bg-0);
  color: inherit;
  font: inherit;
  font-size: var(--text-md);
}
.inbox-search:focus {
  outline: none;
  border-color: var(--accent);
}
.inbox-seg {
  display: flex;
  flex: 0 0 auto;
  padding: 3px;
  gap: 2px;
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: var(--bg-0);
}
.inbox-seg button {
  height: 30px;
  padding: 0 14px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--text-2);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}
.inbox-seg button.active {
  background: var(--accent-muted);
  color: var(--accent);
}
.inbox-table-wrap {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  margin: 0 24px 24px;
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: var(--bg-0);
}
.inbox-table {
  width: 100%;
  min-width: 820px;
  table-layout: fixed;
  border-collapse: collapse;
  font-size: var(--text-md);
}
.inbox-col-name { width: 36%; }
.inbox-col-mtime { width: 16%; }
.inbox-col-source { width: 28%; }
.inbox-col-kind { width: 9%; }
.inbox-col-status { width: 11%; }
.inbox-table th,
.inbox-table td {
  padding: 12px 16px;
  text-align: left;
  border-bottom: 1px solid var(--hairline);
  vertical-align: middle;
}
.inbox-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: #0e1620;
  font-weight: var(--fw-medium);
  font-size: var(--text-xs);
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-3);
  white-space: nowrap;
}
.inbox-table th button {
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  font-weight: inherit;
  letter-spacing: inherit;
  text-transform: inherit;
  cursor: pointer;
  padding: 0;
}
.inbox-sort {
  margin-left: 4px;
  color: var(--accent);
}
.inbox-table tbody tr {
  cursor: pointer;
}
.inbox-table tbody tr:hover {
  background: var(--bg-2);
}
.inbox-table tbody tr:last-child td {
  border-bottom: 0;
}
.inbox-table .inbox-name,
.inbox-table .inbox-source {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.inbox-table .inbox-name {
  color: var(--text-1);
  font-weight: var(--fw-medium);
}
.inbox-mtime,
.inbox-source {
  white-space: nowrap;
  color: var(--text-2);
  font-variant-numeric: tabular-nums;
}
.inbox-kind {
  display: inline-block;
  font-size: var(--text-xs);
  padding: 3px 8px;
  border-radius: 999px;
  background: var(--bg-2);
  color: var(--text-2);
}
.inbox-kind--clip {
  background: var(--accent-muted);
  color: var(--accent);
}
.inbox-kind--convert {
  background: var(--bg-2);
  color: var(--text-1);
}
.inbox-badge {
  flex: 0 0 auto;
  font-size: var(--text-xs);
  padding: 2px 8px;
  border-radius: 999px;
}
.inbox-badge--yes {
  background: var(--ok-muted);
  color: var(--ok);
}
.inbox-badge--no {
  background: var(--warn-muted);
  color: var(--warn);
}
.inbox-empty {
  flex: 1 1 auto;
  min-height: 0;
  padding: 24px 0;
  text-align: center;
  color: var(--text-3);
  font-size: var(--text-sm);
}
.mcp-check {
  display: grid;
  grid-template-columns: auto 1fr;
  column-gap: 8px;
  row-gap: 2px;
  align-items: start;
  margin: 10px 0;
  font-size: var(--text-sm);
  color: var(--text-2);
}
.mcp-check > input {
  margin-top: 3px;
}
.mcp-check-label {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.mcp-check-hint {
  grid-column: 2;
  font-size: var(--text-xs);
  color: var(--text-3);
  word-break: break-all;
}
.mcp-check--disabled {
  opacity: 0.72;
}
.mcp-badge {
  font-size: var(--text-xs);
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--ok-muted);
  color: var(--ok);
}
.mcp-badge--warn {
  background: var(--warn-muted);
  color: var(--warn);
}

.broken-index-backdrop {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(8, 10, 14, 0.55);
  -webkit-app-region: no-drag;
}

.register-modal {
  width: min(420px, calc(100% - 48px));
  padding: 18px 18px 14px;
  border-radius: 10px;
  border: 1px solid var(--hairline);
  background: var(--bg-1);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
  color: var(--text-1);
  font-size: var(--text-sm);
  font-family: var(--font-sans);
  line-height: var(--lh-body);
}

.register-modal-title {
  margin: 0 0 6px;
  font-size: var(--text-title);
  font-weight: var(--fw-semibold);
  line-height: var(--lh-tight);
  color: var(--text-1);
}

.register-modal-hint {
  margin: 0 0 14px;
  font-size: var(--text-sm);
  color: var(--text-3);
  line-height: var(--lh-body);
}

.register-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: var(--text-sm);
  color: #aeb6c8;
}

.register-field input {
  height: 32px;
  padding: 0 10px;
  border: 1px solid var(--hairline);
  border-radius: 6px;
  background: var(--bg-0);
  color: var(--text-1);
  font: inherit;
  outline: none;
}

.register-field input:focus {
  border-color: var(--accent);
}

.register-filename {
  display: flex;
  gap: 8px;
  align-items: baseline;
  margin-top: 10px;
  font-size: var(--text-sm);
}

.register-filename-label {
  color: var(--text-3);
  flex: 0 0 auto;
}

.register-filename-value {
  color: var(--text-2);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.register-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.broken-index-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 16px;
}

.broken-index-actions button {
  height: 32px;
  padding: 0 12px;
  border: 1px solid var(--hairline);
  border-radius: 6px;
  background: var(--bg-2);
  color: var(--text-1);
  cursor: pointer;
  font: inherit;
  font-size: var(--text-sm);
  font-weight: var(--fw-medium);
  text-align: center;
}

.broken-index-actions button:hover:not(:disabled) {
  border-color: #4a5568;
  color: #fff;
}

.broken-index-actions button:disabled {
  opacity: 0.45;
  cursor: default;
}

.broken-index-actions .register-primary {
  background: var(--accent-muted);
  border-color: var(--accent);
  color: var(--text-1);
}

.broken-index-actions .broken-index-danger {
  background: var(--danger-muted);
  border-color: #8a4050;
  color: #f0c0c8;
}

.register-actions button {
  height: 30px;
  padding: 0 12px;
  border: 1px solid var(--hairline);
  border-radius: 6px;
  background: var(--bg-2);
  color: var(--text-1);
  cursor: pointer;
  font-size: var(--text-sm);
  font-weight: var(--fw-medium);
  font: inherit;
}

.register-actions button:hover:not(:disabled) {
  border-color: #4a5568;
  color: #fff;
}

.register-actions button:disabled {
  opacity: 0.45;
  cursor: default;
}

.register-actions .register-primary {
  background: var(--accent-muted);
  border-color: var(--accent);
  color: var(--text-1);
}

.workbench {
  display: flex;
  flex: 1;
  min-height: 0;
}

.workbench--resizing {
  cursor: col-resize;
  user-select: none;
}

.asset-pane {
  position: relative;
  flex: 0 0 auto;
  display: flex;
  min-width: 0;
  height: 100%;
  transition: width 0.18s ease, flex-basis 0.18s ease;
  border-right: 1px solid var(--hairline);
  background: var(--bg-0);
}

.workbench--resizing .asset-pane {
  transition: none;
}

.asset-pane--collapsed {
  overflow: hidden;
}

.asset-side {
  flex: 1;
  min-width: 0;
  height: 100%;
}

.asset-resizer {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 3;
}

.asset-resizer:hover,
.workbench--resizing .asset-resizer {
  background: rgba(80, 140, 220, 0.35);
}

.asset-rail {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 10px;
  width: 100%;
  height: 100%;
  padding: 10px 0;
  border: none;
  background: var(--bg-1);
  color: var(--text-2);
  cursor: pointer;
}

.asset-rail:hover {
  background: var(--bg-2);
  color: #fff;
}

.asset-rail-label {
  writing-mode: vertical-rl;
  text-orientation: mixed;
  letter-spacing: 0.12em;
  font-size: var(--text-sm);
  font-weight: 600;
}

.asset-rail-chevron {
  font-size: var(--text-icon);
  line-height: 1;
  opacity: 0.75;
}

.editor-col {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.toolbar {
  display: flex;
  flex-wrap: nowrap;
  gap: 10px;
  align-items: center;
  padding: 4px 10px;
  background: var(--bg-1);
  border-bottom: 1px solid var(--hairline);
  -webkit-app-region: drag;
  min-height: var(--toolbar-h);
}

.toolbar-file {
  overflow-x: auto;
  gap: 0;
}

.tb-seg {
  display: flex;
  gap: 10px;
  align-items: center;
  padding: 0 12px;
  -webkit-app-region: no-drag;
}

.tb-seg + .tb-seg {
  border-left: 1px solid var(--hairline);
}

.tb-seg--trail {
  margin-left: auto;
}
.tb-seg--settings {
  /* stays at far right after trail (trail already has margin-left: auto) */
  flex: 0 0 auto;
}
.settings-modal {
  width: min(820px, calc(100% - 40px));
  height: min(78vh, 640px);
  max-height: min(88vh, 720px);
  padding: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.settings-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px 10px;
  border-bottom: 1px solid var(--hairline);
  flex: 0 0 auto;
}
.settings-head .register-modal-title {
  margin: 0;
}
.settings-close {
  width: 28px;
  height: 28px;
  border: 1px solid var(--hairline);
  background: var(--bg-2);
  color: var(--text-2);
  border-radius: 6px;
  cursor: pointer;
  -webkit-app-region: no-drag;
}
.settings-body {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 132px 1fr;
}
.settings-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 8px;
  border-right: 1px solid var(--hairline);
  background: var(--bg-1);
  overflow-y: auto;
}
.settings-nav button {
  text-align: left;
  height: 32px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-2);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
  -webkit-app-region: no-drag;
}
.settings-nav button:hover {
  background: var(--bg-2);
  color: var(--text-1);
}
.settings-nav button.active {
  background: var(--accent-muted);
  color: var(--accent);
  box-shadow: inset 2px 0 0 0 var(--accent);
}
.settings-pane {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 14px 16px 16px;
}
.settings-panel {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-height: 100%;
}
.settings-lead {
  margin: 0 0 10px;
  font-size: var(--text-sm);
  color: var(--text-3);
  line-height: 1.45;
}
.settings-pane-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: auto;
  padding-top: 14px;
}
.settings-target-list {
  margin: 4px 0 0;
  padding-left: 16px;
  word-break: break-all;
}
.settings-shortcut-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm);
}
.settings-shortcut-table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--hairline);
  color: var(--text-2);
}
.settings-shortcut-keys {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text-1);
  white-space: nowrap;
  width: 7.5rem;
}
.settings-shortcut-table--muted td {
  color: var(--text-3);
}
.mcp-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  margin: 4px 0;
}
.settings-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 10px 0;
  font-size: var(--text-sm);
  color: var(--text-2);
}
.settings-row {
  display: flex;
  gap: 6px;
  align-items: center;
}
.settings-row input {
  flex: 1;
  min-width: 0;
  height: 28px;
  border: 1px solid var(--hairline);
  background: var(--bg-0);
  color: var(--text-1);
  border-radius: 4px;
  padding: 0 8px;
  font: inherit;
}
.settings-row button {
  flex: 0 0 auto;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--hairline);
  background: var(--bg-2);
  color: var(--text-1);
  border-radius: 4px;
  font: inherit;
  cursor: pointer;
  -webkit-app-region: no-drag;
}
.settings-row button:hover:not(:disabled) {
  border-color: var(--accent);
}
.settings-readonly {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: var(--text-xs);
  color: var(--text-3);
}
.settings-readonly dt {
  color: var(--text-2);
  margin: 0;
}
.settings-readonly dd {
  margin: 2px 0 0;
  word-break: break-all;
  color: var(--text-2);
}

.toolbar-format {
  background: var(--bg-1);
  overflow: visible;
  flex-wrap: wrap;
}

.tb-select {
  height: 26px;
  border: 1px solid var(--hairline);
  background: var(--bg-2);
  color: var(--text-1);
  border-radius: 4px;
  font-size: var(--text-sm);
  padding: 0 6px;
  -webkit-app-region: no-drag;
}

.tb-select-heading {
  min-width: 7.5rem;
}

.chars-wrap,
.table-wrap {
  position: relative;
}

</style>

<!-- Teleport 到 body，不能用 scoped -->
<style>
.source-menu-flyout,
.recent-menu-flyout {
  padding: 4px;
  background: var(--bg-2);
  border: 1px solid var(--hairline);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  -webkit-app-region: no-drag;
}

.recent-menu-flyout {
  max-height: 320px;
  overflow: auto;
}

.recent-menu-flyout .recent-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  width: 100%;
  gap: 2px;
  text-align: left;
  height: auto;
  min-height: 36px;
  padding: 6px 10px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-1);
  cursor: pointer;
  font: inherit;
  font-size: var(--text-sm);
}

.recent-menu-flyout .recent-item:hover {
  background: var(--bg-2);
  color: #fff;
}

.recent-menu-flyout .recent-path {
  font-size: var(--text-xs);
  color: #7a8494;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.recent-menu-flyout .recent-empty,
.recent-menu-flyout .recent-clear {
  width: 100%;
  text-align: left;
  font-size: var(--text-sm);
  color: #8b93a3;
  padding: 6px 10px;
  border: none;
  border-radius: 4px;
  background: transparent;
  cursor: pointer;
  font: inherit;
}

.recent-menu-flyout .recent-clear:hover {
  background: var(--bg-2);
  color: #fff;
}

.source-menu-item {
  display: block;
  width: 100%;
  text-align: left;
  height: 30px;
  padding: 0 10px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-1);
  cursor: pointer;
  font: inherit;
  font-size: var(--text-sm);
}

.source-menu-item:hover {
  background: var(--bg-2);
  color: #fff;
}

.table-picker {
  padding: 10px;
  background: var(--bg-2);
  border: 1px solid var(--hairline);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  min-width: 200px;
  -webkit-app-region: no-drag;
}

.table-picker-label {
  font-size: var(--text-sm);
  color: var(--text-1);
  margin-bottom: 8px;
  text-align: center;
}

.table-grid {
  display: grid;
  grid-template-columns: repeat(var(--cols, 8), 18px);
  gap: 3px;
  justify-content: center;
}

.table-cell {
  appearance: none;
  width: 18px !important;
  height: 18px !important;
  min-width: 18px !important;
  padding: 0 !important;
  border: 1px solid #3d4656 !important;
  background: var(--bg-2) !important;
  border-radius: 2px !important;
  cursor: pointer;
}

.table-cell.on {
  background: #4a6a8e !important;
  border-color: #7aa0c8 !important;
}

.table-picker-hint {
  margin: 8px 0 0;
  font-size: var(--text-xs);
  color: #7a8494;
  text-align: center;
}

.chars-menu,
.chars-menu-flyout {
  display: grid;
  grid-template-columns: repeat(6, minmax(36px, 1fr));
  gap: 4px;
  padding: 8px;
  box-sizing: border-box;
  background: var(--bg-2);
  border: 1px solid var(--hairline);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  -webkit-app-region: no-drag;
}

.chars-menu-flyout .char-item,
.char-item {
  min-width: 36px;
  min-height: 32px;
  padding: 6px 4px;
  border: 1px solid var(--hairline);
  border-radius: 4px;
  background: transparent;
  color: var(--text-1);
  cursor: pointer;
  font: inherit;
  font-size: var(--text-sm);
}
.chars-menu-flyout .char-item:hover {
  background: color-mix(in srgb, var(--text-1) 12%, transparent);
}

.toolbar button,
.toolbar label,
.toolbar input {
  -webkit-app-region: no-drag;
}

.tb-group {
  display: flex;
  gap: 4px;
  align-items: center;
  padding-right: 10px;
  border-right: 1px solid var(--hairline);
  flex: none;
}

.tb-group:last-child {
  border-right: none;
}

.tb-label {
  font-size: var(--text-xs);
  color: var(--text-3);
  margin-right: 2px;
  user-select: none;
}

.toolbar button {
  appearance: none;
  border: 1px solid var(--hairline);
  background: var(--bg-2);
  color: var(--text-1);
  border-radius: var(--radius);
  padding: 4px 9px;
  font-size: var(--text-sm);
  cursor: pointer;
}

.toolbar .tb-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 1px;
  width: 28px;
  height: 26px;
  padding: 0;
  color: var(--text-2);
}

.toolbar .tb-icon-btn--menu {
  width: auto;
  min-width: 28px;
  padding: 0 4px;
}

.tb-label {
  display: none;
}

.toolbar button:hover:not(:disabled) {
  background: var(--accent-muted);
  border-color: var(--accent);
  color: var(--accent);
}

.toolbar button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.toolbar button.active {
  background: var(--accent-muted);
  border-color: var(--accent);
  color: var(--accent);
}

.color-field {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 26px;
  padding: 0 4px;
  border: 1px solid var(--hairline);
  border-radius: 4px;
  background: var(--bg-2);
  color: #c8ceda;
  font-size: var(--text-xs);
}

.color-field input[type="color"] {
  width: 28px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--hairline);
  border-radius: 4px;
  background: transparent;
  cursor: pointer;
}

.recent-wrap {
  position: relative;
}

.recent-menu {
  min-width: 280px;
  max-width: 420px;
  max-height: 320px;
  overflow: auto;
  background: var(--bg-2);
  border: 1px solid var(--hairline);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  padding: 4px;
  -webkit-app-region: no-drag;
}

.recent-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  width: 100%;
  text-align: left;
  gap: 2px;
}

.recent-path {
  font-size: var(--text-xs);
  color: #7a8494;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.recent-empty,
.recent-clear {
  width: 100%;
  text-align: left;
  font-size: var(--text-sm);
  color: #8b93a3;
}

.tabbar {
  display: flex;
  align-items: stretch;
  gap: 2px;
  padding: 0 6px;
  background: var(--bg-0);
  border-bottom: 1px solid var(--hairline);
  overflow-x: auto;
  min-height: 34px;
}

.tab {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  color: var(--text-3);
  padding: 0 10px;
  font-size: var(--text-sm);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  white-space: nowrap;
}

.tab.active {
  color: var(--accent);
  background: var(--accent-muted);
  border-bottom-color: var(--accent);
}

.tab-close {
  opacity: 0.55;
  font-size: var(--text-md);
  line-height: 1;
  padding: 0 2px;
}

.tab-close:hover {
  opacity: 1;
  color: #fff;
}

.tab-add {
  border: none;
  background: transparent;
  color: #8b93a3;
  padding: 0 10px;
  cursor: pointer;
  font-size: var(--text-icon);
}

.main-pane {
  flex: 1;
  display: flex;
  min-height: 0;
  background: #1a1d23;
}

.empty-workbench {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: #8b93a3;
  user-select: none;
  padding: 24px;
  text-align: center;
}
.empty-workbench-title {
  margin: 0;
  font-size: var(--text-title);
  font-weight: var(--fw-semibold);
  line-height: var(--lh-tight);
  color: #c0c6d0;
}
.empty-workbench-hint {
  margin: 0;
  font-size: var(--text-sm);
  max-width: 360px;
  line-height: var(--lh-body);
}

.pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
  position: relative;
}

.splitter {
  width: 5px;
  cursor: col-resize;
  background: var(--hairline);
  flex: none;
}

.splitter:hover {
  background: #4a5a72;
}

.statusbar {
  display: flex;
  gap: 16px;
  align-items: center;
  padding: 4px 12px;
  font-size: var(--text-xs);
  color: #8b93a3;
  background: var(--bg-0);
  border-top: 1px solid var(--hairline);
  min-height: 24px;
}

.sb-path {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.dirty {
  color: #d4a35c;
}

.sb-err {
  color: #e07070;
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sb-stale {
  flex: 0 0 auto;
  color: #e0a040;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-sm);
}
.sb-stale-btn {
  border: 1px solid #a07830;
  background: #3a2e18;
  color: #f0d090;
  border-radius: 4px;
  padding: 1px 8px;
  font-size: var(--text-xs);
  cursor: pointer;
}
.sb-stale-btn:hover {
  opacity: 0.9;
}

.md-helpers button {
  min-width: 28px;
}

</style>
