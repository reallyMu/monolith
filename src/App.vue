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
import type { EditorTab, ViewMode } from "./types";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/messages";
import {
  clearRecent,
  listRecent,
  pushRecent,
  readTextFile,
  takePendingOpens,
  writeTextFile,
} from "./utils/api";
import {
  assetCreateFromPath,
  assetFindByPath,
  assetSaveNewVersion,
  type AssetDto,
} from "./utils/assetsApi";
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

const BG_EDIT_KEY = "monolith.bg.edit";
const BG_PREVIEW_KEY = "monolith.bg.preview";
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

const tabs = ref<EditorTab[]>([newTab()]);
const activeId = ref(tabs.value[0].id);
const recent = ref<string[]>([]);
const recentOpen = ref(false);
const charsOpen = ref(false);
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

const editBg = ref(localStorage.getItem(BG_EDIT_KEY) || "#1a1d23");
const previewBg = ref(localStorage.getItem(BG_PREVIEW_KEY) || "#15181e");

let scrollLock = false;
/** Last user-driven side for MD split sync. Programmatic scrolls must not flip this. */
let mdDriver: "source" | "preview" = "source";

const active = computed(() => tabs.value.find((tab) => tab.id === activeId.value) ?? tabs.value[0]);
const showEdit = computed(() => active.value.viewMode === "edit" || active.value.viewMode === "split");
const showPreview = computed(
  () => active.value.viewMode === "preview" || active.value.viewMode === "split",
);
const kind = computed(() => previewKind(active.value.path));
const isSplit = computed(() => active.value.viewMode === "split");
const isMd = computed(() => kind.value === "markdown");
/** Markdown right pane: TipTap (editable) in preview and split. */
const useMdWysiwyg = computed(() => kind.value === "markdown" && showPreview.value);
/** MD format helpers work on source or WYSIWYG when either pane is visible. */
const canEditMd = computed(() => isMd.value && (showEdit.value || showPreview.value));
/** Toolbar targets TipTap when preview-only, or when the right pane has focus in split. */
const useWysiwygToolbar = computed(
  () =>
    useMdWysiwyg.value &&
    (active.value.viewMode === "preview" || mdFocus.value === "wysiwyg"),
);

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

function setActive(id: string) {
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

async function openPath(path: string) {
  if (!isSupportedPath(path)) {
    statusError.value = t("unsupportedType", { name: path.split(/[/\\]/).pop() || path });
    return;
  }
  const existing = tabs.value.find((t) => t.path === path);
  if (existing) {
    activeId.value = existing.id;
    return;
  }
  try {
    const content = await readTextFile(path);
    const t = newTab({
      path,
      title: titleFromPath(path, 0),
      content,
      dirty: false,
      language: languageFromPath(path),
      viewMode: isMarkdownPath(path) || previewKind(path) !== "code" ? "split" : "edit",
    });
    tabs.value.push(t);
    activeId.value = t.id;
    recent.value = await pushRecent(path);
    statusError.value = "";
  } catch (e) {
    statusError.value = String(e);
  }
}

async function openFile() {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: openDialogFilters(),
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
    const picked = await save({
      defaultPath: path ?? `${tab.title}.md`,
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

async function generateAsset() {
  const tab = active.value;
  if (!tab?.path) {
    statusError.value = t("assetNeedPath");
    return;
  }
  try {
    const created = await assetCreateFromPath(tab.path);
    selectedAssetId.value = created.id;
    statusError.value = t("assetCreated");
    await assetBrowserRef.value?.refresh();
  } catch (e) {
    const msg = String(e);
    const m = /ALREADY_REGISTERED:(\d+)/.exec(msg);
    if (m) {
      selectedAssetId.value = Number(m[1]);
      statusError.value = t("assetAlready");
      await assetBrowserRef.value?.refresh();
      return;
    }
    statusError.value = msg;
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

async function openAssetFromTree(asset: AssetDto) {
  if (!asset.absolutePath) return;
  if (!asset.fileExists) {
    statusError.value = t("assetMissing", { path: asset.absolutePath });
    selectedAssetId.value = asset.id;
    return;
  }
  selectedAssetId.value = asset.id;
  await openPath(asset.absolutePath);
}

async function closeTab(id: string) {
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
    const t = newTab();
    tabs.value.push(t);
    activeId.value = t.id;
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
  active.value.viewMode = mode;
  if (mode === "preview") mdFocus.value = "wysiwyg";
  if (mode === "edit") mdFocus.value = "source";
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
const isSql = computed(() => active.value.language === "sql");
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
    const line = editorRef.value?.getAnchorLine() ?? active.value.cursorLine;
    active.value.cursorLine = line;
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
    mdDriver = "preview";
    active.value.cursorLine = info.anchorLine;
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
  mdDriver = "preview";
  mdFocus.value = "wysiwyg";
  active.value.cursorLine = line;
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
  }
}

const unlistens: UnlistenFn[] = [];

function onWinReposition() {
  if (tableOpen.value) placeTablePicker();
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
  for (const path of paths) {
    if (path) await openPath(path);
  }
}

onMounted(async () => {
  await refreshRecent();
  window.addEventListener("mousemove", onSplitMove);
  window.addEventListener("mouseup", onSplitUp);
  window.addEventListener("resize", onWinReposition);
  unlistens.push(
    await listen<string>("menu-action", (e) => {
      void handleMenu(e.payload);
    }),
  );
  unlistens.push(
    await listen<string[]>("open-files", (e) => {
      void openPathsFromOs(e.payload ?? []);
    }),
  );
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
  for (const u of unlistens) void u();
  unlistens.length = 0;
});

watch(activeId, async () => {
  await nextTick();
  editorRef.value?.focus();
});
</script>

<template>
  <div class="app" @click="recentOpen = false; charsOpen = false; tableOpen = false">
    <!-- File toolbar -->
    <header class="toolbar toolbar-file" @click.stop>
      <div class="tb-group">
        <span class="tb-label">{{ t("file") }}</span>
        <button type="button" :title="`${t('new')} (⌘N)`" @click="createUntitled">{{ t("new") }}</button>
        <button type="button" :title="`${t('open')} (⌘O)`" @click="openFile">{{ t("open") }}</button>
        <button type="button" :title="`${t('save')} (⌘S)`" @click="saveActive(false)">{{ t("save") }}</button>
        <button type="button" :title="`${t('saveAs')} (⇧⌘S)`" @click="saveActive(true)">{{ t("saveAs") }}</button>
        <button type="button" :title="t('generateAssetTitle')" @click="generateAsset">
          {{ t("generateAsset") }}
        </button>
        <button type="button" :title="t('saveNewVersionTitle')" @click="saveNewVersion">
          {{ t("saveNewVersion") }}
        </button>
        <button
          type="button"
          :class="{ active: !assetsCollapsed }"
          :title="t('toggleAssets')"
          @click="toggleAssetsSidebar"
        >
          {{ t("toggleAssets") }}
        </button>
        <div class="recent-wrap">
          <button type="button" :title="t('recent')" @click.stop="recentOpen = !recentOpen">
            {{ t("recent") }} ▾
          </button>
          <div v-if="recentOpen" class="recent-menu">
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
        </div>
      </div>
      <div class="tb-right">
        <div class="tb-group">
          <span class="tb-label">{{ t("view") }}</span>
          <button
            type="button"
            :class="{ active: active.viewMode === 'edit' }"
            @click="setViewMode('edit')"
          >
            {{ t("edit") }}
          </button>
          <button
            type="button"
            :class="{ active: active.viewMode === 'preview' }"
            @click="setViewMode('preview')"
          >
            {{ t("preview") }}
          </button>
          <button
            type="button"
            :class="{ active: active.viewMode === 'split' }"
            @click="setViewMode('split')"
          >
            {{ t("split") }}
          </button>
        </div>
        <div class="tb-group tb-colors">
          <span class="tb-label">{{ t("background") }}</span>
          <label class="color-field" :title="t('bgEditTitle')">
            {{ t("bgLeft") }}
            <input type="color" :value="editBg" @input="onEditBg" />
          </label>
          <label class="color-field" :title="t('bgPreviewTitle')">
            {{ t("bgRight") }}
            <input type="color" :value="previewBg" @input="onPreviewBg" />
          </label>
        </div>
      </div>
    </header>

    <!-- Format toolbar -->
    <header class="toolbar toolbar-format" @click.stop>
      <div class="tb-group">
        <span class="tb-label">{{ t("edit") }}</span>
        <button type="button" :title="t('undo')" :disabled="!showEdit" @click="runEditorAction('undo')">
          {{ t("undo") }}
        </button>
        <button type="button" :title="t('redo')" :disabled="!showEdit" @click="runEditorAction('redo')">
          {{ t("redo") }}
        </button>
        <button
          type="button"
          :title="t('find')"
          :disabled="!showEdit"
          @click="runEditorAction('actions.find')"
        >
          {{ t("find") }}
        </button>
        <button
          type="button"
          :title="t('formatSqlTitle')"
          :disabled="!showEdit || !isSql"
          @click="formatSql"
        >
          {{ t("formatSql") }}
        </button>
      </div>

      <div class="tb-group">
        <span class="tb-label">{{ t("heading") }}</span>
        <select
          class="tb-select tb-select-heading"
          :disabled="!(canEditMd || showEdit)"
          :title="t('heading')"
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

      <div class="tb-group">
        <span class="tb-label">{{ t("indent") }}</span>
        <button type="button" :title="t('indentMoreTitle')" :disabled="!showEdit" @click="indentLines">
          {{ t("indentMore") }}
        </button>
        <button type="button" :title="t('indentLessTitle')" :disabled="!showEdit" @click="outdentLines">
          {{ t("indentLess") }}
        </button>
      </div>

      <div class="tb-group">
        <span class="tb-label">{{ t("fontSize") }}</span>
        <select
          class="tb-select"
          :value="fontSize"
          :disabled="!showEdit"
          :title="t('fontSizeTitle')"
          @change="onFontSizeChange"
        >
          <option v-for="s in FONT_SIZES" :key="s" :value="s">{{ s }}px</option>
        </select>
      </div>

      <div class="tb-group md-helpers">
        <span class="tb-label">{{ t("format") }}</span>
        <button type="button" :title="t('bold')" :disabled="!canEditMd" @click="mdBold"><b>B</b></button>
        <button type="button" :title="t('italic')" :disabled="!canEditMd" @click="mdItalic">
          <i>I</i>
        </button>
        <button type="button" :title="t('strike')" :disabled="!canEditMd" @click="mdStrike">
          <s>S</s>
        </button>
        <button type="button" :title="t('inlineCode')" :disabled="!canEditMd" @click="mdCode">`</button>
        <button type="button" :title="t('codeBlock')" :disabled="!canEditMd" @click="mdCodeBlock">
          ```
        </button>
        <button type="button" :title="t('link')" :disabled="!canEditMd" @click="mdLink">{{ t("link") }}</button>
        <button type="button" :title="t('image')" :disabled="!canEditMd" @click="mdImage">{{ t("image") }}</button>
        <button type="button" :title="t('quote')" :disabled="!canEditMd" @click="mdQuote">❝</button>
        <button type="button" :title="t('ul')" :disabled="!canEditMd" @click="mdUl">•</button>
        <button type="button" :title="t('ol')" :disabled="!canEditMd" @click="mdOl">1.</button>
        <button type="button" :title="t('task')" :disabled="!canEditMd" @click="mdTask">☑</button>
        <button type="button" :title="t('hr')" :disabled="!canEditMd" @click="mdHr">—</button>
        <div class="table-wrap">
          <button
            ref="tableBtnRef"
            type="button"
            :title="t('tableInsertTitle')"
            :disabled="!canEditMd"
            @click.stop="toggleTablePicker"
          >
            {{ t("table") }} ▾
          </button>
        </div>
      </div>

      <div class="tb-group chars-wrap">
        <span class="tb-label">{{ t("chars") }}</span>
        <button
          type="button"
          :title="t('charsTitle')"
          :disabled="!canEditMd"
          @click.stop="charsOpen = !charsOpen"
        >
          {{ t("charsMenu") }} ▾
        </button>
        <div v-if="charsOpen" class="chars-menu" @click.stop>
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
          @select-asset="selectedAssetId = $event"
          @error="statusError = $event"
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
        <span class="tab-close" title="Close" @click.stop="closeTab(tabItem.id)">×</span>
      </button>
      <button type="button" class="tab-add" title="New tab" @click="createUntitled">+</button>
    </div>

    <main class="main-pane">
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
    </main>

    <footer class="statusbar">
      <span class="sb-path" :title="active.path || ''">{{ active.path || t("unsaved") }}</span>
      <span>{{ active.language }}</span>
      <span>{{ t("lnCol", { line: active.cursorLine, col: active.cursorCol }) }}</span>
      <span :class="{ dirty: active.dirty }">{{ active.dirty ? t("modified") : t("saved") }}</span>
      <span v-if="statusError" class="sb-err">{{ statusError }}</span>
    </footer>
      </div>
    </div>

    <!-- Teleport：避免被工具栏 overflow 裁切 -->
    <Teleport to="body">
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
    </Teleport>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #12151a;
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
  border-right: 1px solid #2a2f38;
  background: #12151a;
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
  background: #161a20;
  color: #c8ced8;
  cursor: pointer;
}

.asset-rail:hover {
  background: #1c2430;
  color: #fff;
}

.asset-rail-label {
  writing-mode: vertical-rl;
  text-orientation: mixed;
  letter-spacing: 0.12em;
  font-size: 12px;
  font-weight: 600;
}

.asset-rail-chevron {
  font-size: 16px;
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
  padding: 5px 10px;
  background: #1a1e26;
  border-bottom: 1px solid #2a303c;
  -webkit-app-region: drag;
  min-height: 34px;
}

.toolbar-file {
  overflow-x: auto;
}

.tb-right {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-left: auto;
  padding-left: 12px;
  border-left: 1px solid #2a303c;
  -webkit-app-region: no-drag;
}

.toolbar-format {
  background: #171b22;
  overflow: visible;
  flex-wrap: wrap;
}

.tb-select {
  height: 26px;
  border: 1px solid #323846;
  background: #222733;
  color: #d6d8de;
  border-radius: 4px;
  font-size: 12px;
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
.table-picker {
  padding: 10px;
  background: #1e2430;
  border: 1px solid #323846;
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  min-width: 200px;
  -webkit-app-region: no-drag;
}

.table-picker-label {
  font-size: 12px;
  color: #d6d8de;
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
  background: #222733 !important;
  border-radius: 2px !important;
  cursor: pointer;
}

.table-cell.on {
  background: #4a6a8e !important;
  border-color: #7aa0c8 !important;
}

.table-picker-hint {
  margin: 8px 0 0;
  font-size: 10px;
  color: #7a8494;
  text-align: center;
}

.chars-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 50;
  display: grid;
  grid-template-columns: repeat(6, minmax(36px, 1fr));
  gap: 4px;
  padding: 8px;
  min-width: 260px;
  background: #1e2430;
  border: 1px solid #323846;
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
}

.char-item {
  min-width: 36px !important;
  padding: 6px 4px !important;
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
  border-right: 1px solid #2a303c;
  flex: none;
}

.tb-group:last-child {
  border-right: none;
}

.tb-label {
  font-size: 11px;
  color: #6e7787;
  margin-right: 2px;
  user-select: none;
}

.toolbar button {
  appearance: none;
  border: 1px solid #323846;
  background: #222733;
  color: #d6d8de;
  border-radius: 4px;
  padding: 4px 9px;
  font-size: 12px;
  cursor: pointer;
}

.toolbar button:hover:not(:disabled) {
  background: #2c3342;
  border-color: #3d4656;
}

.toolbar button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.toolbar button.active {
  background: #3a4a63;
  border-color: #5a7294;
  color: #fff;
}

.color-field {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: #9aa3b2;
}

.color-field input[type="color"] {
  width: 28px;
  height: 22px;
  padding: 0;
  border: 1px solid #323846;
  border-radius: 4px;
  background: transparent;
  cursor: pointer;
}

.recent-wrap {
  position: relative;
}

.recent-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 40;
  min-width: 280px;
  max-width: 420px;
  max-height: 320px;
  overflow: auto;
  background: #1e2430;
  border: 1px solid #323846;
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  padding: 4px;
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
  font-size: 10px;
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
  font-size: 12px;
  color: #8b93a3;
}

.tabbar {
  display: flex;
  align-items: stretch;
  gap: 2px;
  padding: 0 6px;
  background: #161a21;
  border-bottom: 1px solid #2a303c;
  overflow-x: auto;
  min-height: 34px;
}

.tab {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  color: #9aa3b2;
  padding: 0 10px;
  font-size: 12px;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  white-space: nowrap;
}

.tab.active {
  color: #eef0f4;
  background: #1a1d23;
  border-bottom-color: #6b8caf;
}

.tab-close {
  opacity: 0.55;
  font-size: 14px;
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
  font-size: 16px;
}

.main-pane {
  flex: 1;
  display: flex;
  min-height: 0;
  background: #1a1d23;
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
  background: #2a303c;
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
  font-size: 11px;
  color: #8b93a3;
  background: #151920;
  border-top: 1px solid #2a303c;
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

.md-helpers button {
  min-width: 28px;
}
</style>
