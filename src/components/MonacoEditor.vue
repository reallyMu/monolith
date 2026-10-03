<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type * as Monaco from "monaco-editor";
import { applyEditorTheme, ensureMonaco } from "../monaco";
import {
  contrastHighlightBg,
  contrastHighlightBorder,
  contrastHighlightFg,
  contrastHighlightGutter,
  contrastSelection,
  contrastText,
} from "../utils/contrast";
import { formatSqlText } from "../utils/formatSql";

export type ScrollInfo = {
  scrollTop: number;
  scrollLeft: number;
  scrollWidth: number;
  scrollHeight: number;
  width: number;
  height: number;
};

const props = withDefaults(
  defineProps<{
    modelValue: string;
    language: string;
    readOnly?: boolean;
    background?: string;
    highlightLine?: number;
    fontSize?: number;
  }>(),
  {
    readOnly: false,
    background: "#1a1d23",
    highlightLine: 0,
    fontSize: 13,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  cursor: [line: number, col: number];
  scroll: [info: ScrollInfo];
  focus: [];
  ready: [editor: Monaco.editor.IStandaloneCodeEditor];
}>();

const host = ref<HTMLDivElement | null>(null);
let editor: Monaco.editor.IStandaloneCodeEditor | null = null;
let applying = false;
let syncingScroll = false;
let syncScrollTimer: ReturnType<typeof setTimeout> | null = null;
let decoIds: string[] = [];

function beginSyncScroll(ms = 200) {
  syncingScroll = true;
  if (syncScrollTimer) clearTimeout(syncScrollTimer);
  syncScrollTimer = setTimeout(() => {
    syncingScroll = false;
    syncScrollTimer = null;
  }, ms);
}

function syncTheme() {
  const fg = contrastText(props.background);
  applyEditorTheme({
    background: props.background,
    foreground: fg,
    lineHighlightBg: contrastHighlightBg(props.background),
    lineHighlightBorder: contrastHighlightBorder(props.background),
    selectionBg: contrastSelection(props.background),
  });
  if (host.value) {
    host.value.style.setProperty("--hl-border", contrastHighlightBorder(props.background));
    host.value.style.setProperty("--hl-fg", contrastHighlightFg(props.background));
    host.value.style.setProperty("--hl-gutter", contrastHighlightGutter(props.background));
    host.value.style.setProperty("--fg", fg);
  }
}

function applyHighlight() {
  if (!editor) return;
  const line = props.highlightLine > 0 ? props.highlightLine : 0;
  decoIds = editor.deltaDecorations(
    decoIds,
    line
      ? [
          {
            range: {
              startLineNumber: line,
              startColumn: 1,
              endLineNumber: line,
              endColumn: 1,
            },
            options: {
              isWholeLine: true,
              className: "monolith-line-hl",
              linesDecorationsClassName: "monolith-line-hl-gutter",
              overviewRuler: undefined,
            },
          },
        ]
      : [],
  );
}

/** Format selection or whole buffer when language is sql. */
function formatSql() {
  if (!editor || props.readOnly) return false;
  if (props.language !== "sql") return false;
  const model = editor.getModel();
  const sel = editor.getSelection();
  if (!model || !sel) return false;
  const range = sel.isEmpty() ? model.getFullModelRange() : sel;
  const formatted = formatSqlText(model.getValueInRange(range));
  if (formatted == null) return false;
  editor.executeEdits("sql-format", [{ range, text: formatted, forceMoveMarkers: true }]);
  editor.focus();
  return true;
}

onMounted(() => {
  const monaco = ensureMonaco();
  if (!host.value) return;
  syncTheme();
  editor = monaco.editor.create(host.value, {
    value: props.modelValue,
    language: props.language,
    theme: "monolith-dark",
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: props.fontSize,
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    fontLigatures: false,
    disableLayerHinting: true,
    readOnly: props.readOnly,
    wordWrap: "on",
    scrollBeyondLastLine: false,
    // gutter only — avoid full-line border stacking with decorations (looks like ghosting)
    renderLineHighlight: "gutter",
    tabSize: 2,
    insertSpaces: true,
  });

  editor.addAction({
    id: "monolith.formatSql",
    label: "Format SQL",
    keybindings: [monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.KeyF],
    run: () => {
      formatSql();
    },
  });

  editor.onDidChangeModelContent(() => {
    if (!editor || applying || props.readOnly) return;
    emit("update:modelValue", editor.getValue());
  });

  editor.onDidChangeCursorPosition((e) => {
    emit("cursor", e.position.lineNumber, e.position.column);
  });

  editor.onDidFocusEditorWidget(() => {
    emit("focus");
  });

  editor.onDidScrollChange(() => {
    if (!editor || syncingScroll) return;
    emit("scroll", {
      scrollTop: editor.getScrollTop(),
      scrollLeft: editor.getScrollLeft(),
      scrollWidth: editor.getScrollWidth(),
      scrollHeight: editor.getScrollHeight(),
      width: editor.getLayoutInfo().width,
      height: editor.getLayoutInfo().height,
    });
  });

  applyHighlight();
  emit("ready", editor);
});

watch(
  () => props.modelValue,
  (v) => {
    if (!editor || editor.getValue() === v) return;
    applying = true;
    editor.setValue(v);
    applying = false;
  },
);

watch(
  () => props.language,
  (lang) => {
    if (!editor) return;
    const monaco = ensureMonaco();
    const model = editor.getModel();
    if (model) monaco.editor.setModelLanguage(model, lang);
  },
);

watch(
  () => props.readOnly,
  (ro) => {
    editor?.updateOptions({ readOnly: ro });
  },
);

watch(
  () => props.fontSize,
  (size) => {
    editor?.updateOptions({ fontSize: size });
  },
);

watch(
  () => props.background,
  () => {
    syncTheme();
  },
);

watch(
  () => props.highlightLine,
  () => applyHighlight(),
);

onBeforeUnmount(() => {
  if (syncScrollTimer) clearTimeout(syncScrollTimer);
  editor?.dispose();
  editor = null;
});

defineExpose({
  getEditor: () => editor,
  focus: () => editor?.focus(),
  formatSql,
  trigger: (handlerId: string) => {
    editor?.focus();
    editor?.trigger("monolith", handlerId, null);
  },
  setScrollTop: (top: number) => {
    if (!editor) return;
    beginSyncScroll();
    editor.setScrollTop(top);
  },
  setScrollRatio: (ratio: number) => {
    if (!editor) return;
    const max = Math.max(0, editor.getScrollHeight() - editor.getLayoutInfo().height);
    beginSyncScroll();
    editor.setScrollTop(Math.max(0, Math.min(max, ratio * max)));
  },
  /** First visible source line (1-based). */
  getAnchorLine: () => {
    if (!editor) return 1;
    return editor.getVisibleRanges()[0]?.startLineNumber ?? 1;
  },
  /** Viewport Y (px) of a source line's top within the editor. */
  getLineViewportTop: (line: number) => {
    if (!editor || line < 1) return 0;
    return editor.getScrolledVisiblePosition({ lineNumber: line, column: 1 })?.top ?? 0;
  },
  /** Scroll so `line` sits at `viewportTop` px in the editor viewport. */
  alignToLine: (line: number, viewportTop = 0) => {
    if (!editor || line < 1) return;
    const target = Math.max(0, viewportTop);
    beginSyncScroll();
    editor.setScrollTop(Math.max(0, editor.getTopForLineNumber(line) - target));
    // One correction pass — getTopForLineNumber and painted viewport can differ by a few px.
    requestAnimationFrame(() => {
      if (!editor) return;
      const actual = editor.getScrolledVisiblePosition({ lineNumber: line, column: 1 })?.top;
      if (actual == null) return;
      const err = actual - target;
      if (Math.abs(err) <= 1) return;
      beginSyncScroll();
      editor.setScrollTop(Math.max(0, editor.getScrollTop() + err));
    });
  },
  revealLine: (line: number) => {
    if (!editor || line < 1) return;
    beginSyncScroll();
    editor.revealLineInCenterIfOutsideViewport(line);
  },
  insertAround: (before: string, after: string, placeholder = "") => {
    if (!editor || props.readOnly) return;
    const sel = editor.getSelection();
    if (!sel) return;
    const selected = editor.getModel()?.getValueInRange(sel) || placeholder;
    const text = `${before}${selected}${after}`;
    editor.executeEdits("md-helper", [{ range: sel, text, forceMoveMarkers: true }]);
    editor.focus();
  },
  insertLinePrefix: (prefix: string) => {
    if (!editor || props.readOnly) return;
    const pos = editor.getPosition();
    if (!pos) return;
    const range = {
      startLineNumber: pos.lineNumber,
      startColumn: 1,
      endLineNumber: pos.lineNumber,
      endColumn: 1,
    };
    editor.executeEdits("md-helper", [{ range, text: prefix, forceMoveMarkers: true }]);
    editor.focus();
  },
  /**
   * Toggle / apply ordered list on selection.
   * Continues numbering from the previous contiguous `N.` item; multi-line → 1..n sequence.
   * Click again on already-numbered lines to strip numbering.
   */
  toggleOrderedList: () => {
    if (!editor || props.readOnly) return;
    const model = editor.getModel();
    const sel = editor.getSelection();
    if (!model || !sel) return;

    const startLine = sel.startLineNumber;
    const endLine = sel.endLineNumber;
    const olRe = /^(\s*)(\d+)\.\s+(.*)$/;
    const stripRe = /^(\s*)(?:[-*+]\s+(?:\[[ xX]\]\s+)?|\d+\.\s+)(.*)$/;

    let allOl = true;
    for (let line = startLine; line <= endLine; line++) {
      if (!olRe.test(model.getLineContent(line))) {
        allOl = false;
        break;
      }
    }

    const edits: Monaco.editor.IIdentifiedSingleEditOperation[] = [];

    if (allOl) {
      for (let line = startLine; line <= endLine; line++) {
        const raw = model.getLineContent(line);
        const m = raw.match(olRe);
        if (!m) continue;
        const next = `${m[1]}${m[3]}`;
        edits.push({
          range: {
            startLineNumber: line,
            startColumn: 1,
            endLineNumber: line,
            endColumn: raw.length + 1,
          },
          text: next,
        });
      }
      editor.executeEdits("md-ol-off", edits);
      editor.focus();
      return;
    }

    // Continue from previous contiguous ordered item
    let startNum = 1;
    if (startLine > 1) {
      const prev = model.getLineContent(startLine - 1).match(olRe);
      if (prev) startNum = Number(prev[2]) + 1;
    }

    let n = startNum;
    for (let line = startLine; line <= endLine; line++) {
      const raw = model.getLineContent(line);
      const stripped = raw.match(stripRe);
      const indent = stripped ? stripped[1] : (raw.match(/^\s*/)?.[0] ?? "");
      const body = stripped ? stripped[2] : raw.replace(/^\s*/, "");
      const next = `${indent}${n}. ${body}`;
      n += 1;
      edits.push({
        range: {
          startLineNumber: line,
          startColumn: 1,
          endLineNumber: line,
          endColumn: raw.length + 1,
        },
        text: next,
      });
    }

    // Renumber following contiguous OL lines so the list stays consistent
    let expect = n;
    for (let line = endLine + 1; line <= model.getLineCount(); line++) {
      const raw = model.getLineContent(line);
      const m = raw.match(olRe);
      if (!m) break;
      const next = `${m[1]}${expect}. ${m[3]}`;
      if (next !== raw) {
        edits.push({
          range: {
            startLineNumber: line,
            startColumn: 1,
            endLineNumber: line,
            endColumn: raw.length + 1,
          },
          text: next,
        });
      }
      expect += 1;
    }

    editor.executeEdits("md-ol-on", edits);
    editor.focus();
  },
  insertText: (text: string) => {
    if (!editor || props.readOnly) return;
    const sel = editor.getSelection();
    if (!sel) return;
    editor.executeEdits("md-helper", [{ range: sel, text, forceMoveMarkers: true }]);
    editor.focus();
  },
  /** Set ATX heading level 1–6 on selected lines (strips existing # prefix). */
  setHeadingLevel: (level: number) => {
    if (!editor || props.readOnly) return;
    const model = editor.getModel();
    const sel = editor.getSelection();
    if (!model || !sel) return;
    const lv = Math.min(6, Math.max(0, Math.floor(level)));
    const prefix = lv === 0 ? "" : "#".repeat(lv) + " ";
    const edits: Monaco.editor.IIdentifiedSingleEditOperation[] = [];
    for (let line = sel.startLineNumber; line <= sel.endLineNumber; line++) {
      const raw = model.getLineContent(line);
      const body = raw.replace(/^\s{0,3}#{1,6}\s+/, "");
      const next = prefix + body;
      edits.push({
        range: {
          startLineNumber: line,
          startColumn: 1,
          endLineNumber: line,
          endColumn: raw.length + 1,
        },
        text: next,
      });
    }
    editor.executeEdits("md-heading", edits);
    editor.focus();
  },
  indentLines: () => {
    if (!editor || props.readOnly) return;
    editor.focus();
    editor.trigger("monolith", "editor.action.indentLines", null);
  },
  outdentLines: () => {
    if (!editor || props.readOnly) return;
    editor.focus();
    editor.trigger("monolith", "editor.action.outdentLines", null);
  },
});
</script>

<template>
  <div ref="host" class="monaco-host" :style="{ background: background }" />
</template>

<style scoped>
.monaco-host {
  width: 100%;
  height: 100%;
  min-height: 0;
}
</style>

<style>
/* Box outline only — no solid fill (avoids loud red / wash). */
.monolith-line-hl {
  background: transparent !important;
  box-shadow: inset 0 0 0 1px var(--hl-border, #8aa4c499) !important;
}
.monolith-line-hl-gutter {
  background: transparent !important;
  border-left: 2px solid var(--hl-gutter, #8aa4c4aa);
  margin-left: 2px;
  box-sizing: border-box;
}
</style>
