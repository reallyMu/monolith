<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import Link from "@tiptap/extension-link";
import Image from "@tiptap/extension-image";
import Placeholder from "@tiptap/extension-placeholder";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import { Table } from "@tiptap/extension-table";
import { TableRow } from "@tiptap/extension-table-row";
import { TableCell } from "@tiptap/extension-table-cell";
import { TableHeader } from "@tiptap/extension-table-header";
import { Markdown } from "tiptap-markdown";
import type { Editor } from "@tiptap/core";
import {
  offsetTopWithin,
  sourceSpansFromMarkdown,
  spanForLine,
  type SourceSpan,
} from "../utils/mdSourceMap";
import { SourceHighlight, sourceHighlightKey } from "../utils/mdSourceHighlight";

const props = withDefaults(
  defineProps<{
    content: string;
    highlightLine?: number;
    background?: string;
    foreground?: string;
    muted?: string;
    border?: string;
    highlightBg?: string;
    highlightFg?: string;
    placeholder?: string;
    editable?: boolean;
  }>(),
  {
    highlightLine: 0,
    background: "#15181e",
    foreground: "#e6e8ee",
    muted: "#9aa3b2",
    border: "rgba(255,255,255,0.1)",
    highlightBg: "#8aa4c499",
    highlightFg: "#f2f5fa",
    placeholder: "",
    editable: true,
  },
);

const emit = defineEmits<{
  "update:content": [value: string];
  focus: [];
  scroll: [
    info: {
      scrollTop: number;
      scrollHeight: number;
      clientHeight: number;
      anchorLine: number;
      anchorViewportTop: number;
    },
  ];
  "line-click": [line: number, viewportTop: number];
}>();

const root = ref<HTMLDivElement | null>(null);
let applying = false;
let syncing = false;

function getMarkdown(ed: Editor): string {
  return (ed.storage as { markdown?: { getMarkdown: () => string } }).markdown?.getMarkdown?.() ?? "";
}

function normMd(s: string) {
  return s.replace(/\r\n/g, "\n").replace(/\n+$/, "");
}

const editor = useEditor({
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3, 4, 5, 6] } }),
    Link.configure({ openOnClick: false, autolink: true }),
    Image.configure({ inline: false, allowBase64: true }),
    Placeholder.configure({ placeholder: props.placeholder || "…" }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Table.configure({ resizable: true }),
    TableRow,
    TableHeader,
    TableCell,
    SourceHighlight,
    Markdown.configure({
      html: false,
      tightLists: true,
      bulletListMarker: "-",
      linkify: true,
      breaks: false,
      transformPastedText: true,
      transformCopiedText: true,
    }),
  ],
  content: props.content || "",
  editable: props.editable,
  editorProps: {
    attributes: { class: "md-wysiwyg-prose" },
    handleDOMEvents: {
      focus: () => {
        emit("focus");
        return false;
      },
    },
  },
  onUpdate: ({ editor: ed }) => {
    if (applying) return;
    emit("update:content", getMarkdown(ed));
  },
});

function proseEl(): HTMLElement | null {
  return root.value?.querySelector(".md-wysiwyg-prose") ?? null;
}

/** Top-level TipTap blocks (same order as sourceSpansFromMarkdown). */
function blocks(): HTMLElement[] {
  const prose = proseEl();
  if (!prose) return [];
  return Array.from(prose.children).filter((n): n is HTMLElement => n instanceof HTMLElement);
}

function spans(): SourceSpan[] {
  return sourceSpansFromMarkdown(props.content || "");
}

function blockIndexForLine(line: number): number {
  const s = spanForLine(spans(), line);
  if (!s) return -1;
  return spans().findIndex((x) => x.start === s.start && x.end === s.end);
}

function setActiveIndex(index: number) {
  const ed = editor.value;
  if (!ed) return;
  ed.view.dispatch(ed.state.tr.setMeta(sourceHighlightKey, { index }));
}

function setActive(line: number): HTMLElement | null {
  const list = blocks();
  const idx = line > 0 ? blockIndexForLine(line) : -1;
  setActiveIndex(idx);
  if (idx < 0 || idx >= list.length) return null;
  return list[idx];
}

function applyHighlight() {
  setActive(props.highlightLine > 0 ? props.highlightLine : 0);
}

function alignToSourceLine(line: number, viewportTop = 0) {
  const el = root.value;
  const block = setActive(line);
  if (!el || !block) return;
  const target = Math.max(0, viewportTop);
  syncing = true;
  el.scrollTop = Math.max(0, offsetTopWithin(block, el) - target);
  requestAnimationFrame(() => {
    if (!root.value || !block.isConnected) {
      syncing = false;
      return;
    }
    const actual = block.getBoundingClientRect().top - root.value.getBoundingClientRect().top;
    const err = actual - target;
    if (Math.abs(err) > 1) {
      root.value.scrollTop = Math.max(0, root.value.scrollTop + err);
    }
    requestAnimationFrame(() => {
      syncing = false;
    });
  });
}

/**
 * Map a viewport Y to a source line inside the covering TipTap block.
 * Returns the same viewportTop so Monaco can park that line at the same Y
 * (mirrors left→right: line L at viewportTop V).
 */
function anchorAtViewport(y = 8): { line: number; viewportTop: number } {
  const el = root.value;
  const list = blocks();
  const spanList = spans();
  const viewportTop = Math.max(0, y);
  if (!el || !list.length || !spanList.length) return { line: 1, viewportTop };

  const targetY = el.scrollTop + viewportTop;
  let bestIdx = 0;
  for (let i = 0; i < list.length; i++) {
    const top = offsetTopWithin(list[i], el);
    const bottom = top + list[i].offsetHeight;
    if (targetY >= top && targetY < bottom) {
      bestIdx = i;
      break;
    }
    if (top <= targetY) bestIdx = i;
  }

  const span = spanList[Math.min(bestIdx, spanList.length - 1)];
  if (!span) return { line: 1, viewportTop };

  const top = offsetTopWithin(list[bestIdx], el);
  const height = Math.max(1, list[bestIdx].offsetHeight);
  const t = Math.min(1, Math.max(0, (targetY - top) / height));
  const line = Math.round(span.start + t * Math.max(0, span.end - span.start));
  return {
    line: Math.min(span.end, Math.max(span.start, line)),
    viewportTop,
  };
}

function onScroll() {
  if (!root.value || syncing) return;
  const anchor = anchorAtViewport(8);
  emit("scroll", {
    scrollTop: root.value.scrollTop,
    scrollHeight: root.value.scrollHeight,
    clientHeight: root.value.clientHeight,
    anchorLine: anchor.line,
    anchorViewportTop: anchor.viewportTop,
  });
}

function onClick(e: MouseEvent) {
  const el = root.value;
  const prose = proseEl();
  if (!el || !prose) return;
  let node = e.target as HTMLElement | null;
  while (node && node !== prose && node.parentElement !== prose) {
    node = node.parentElement;
  }
  if (!node || node.parentElement !== prose) return;
  const idx = blocks().indexOf(node);
  const span = spans()[idx];
  if (!span) return;
  const viewportTop = Math.max(0, e.clientY - el.getBoundingClientRect().top);
  const top = offsetTopWithin(node, el);
  const height = Math.max(1, node.offsetHeight);
  const t = Math.min(1, Math.max(0, (el.scrollTop + viewportTop - top) / height));
  const line = Math.round(span.start + t * Math.max(0, span.end - span.start));
  emit("line-click", Math.min(span.end, Math.max(span.start, line)), viewportTop);
}

watch(
  () => props.content,
  (v) => {
    const ed = editor.value;
    if (!ed) return;
    if (normMd(getMarkdown(ed)) === normMd(v || "")) {
      void nextTick(applyHighlight);
      return;
    }
    applying = true;
    ed.commands.setContent(v || "", { emitUpdate: false });
    applying = false;
    void nextTick(applyHighlight);
  },
);

watch(
  () => props.editable,
  (v) => editor.value?.setEditable(v),
);

watch(
  () => props.highlightLine,
  () => void nextTick(applyHighlight),
);

watch(
  root,
  (el, prev) => {
    prev?.removeEventListener("scroll", onScroll);
    el?.addEventListener("scroll", onScroll, { passive: true });
  },
);

onBeforeUnmount(() => {
  root.value?.removeEventListener("scroll", onScroll);
  editor.value?.destroy();
});

function run(fn: (ed: Editor) => void) {
  const ed = editor.value as Editor | null | undefined;
  if (!ed || !props.editable) return;
  fn(ed);
}

defineExpose({
  focus: () => editor.value?.commands.focus(),
  alignToSourceLine,
  anchorAtViewport,
  toggleBold: () => run((ed) => ed.chain().focus().toggleBold().run()),
  toggleItalic: () => run((ed) => ed.chain().focus().toggleItalic().run()),
  toggleStrike: () => run((ed) => ed.chain().focus().toggleStrike().run()),
  toggleCode: () => run((ed) => ed.chain().focus().toggleCode().run()),
  toggleCodeBlock: () => run((ed) => ed.chain().focus().toggleCodeBlock().run()),
  toggleBlockquote: () => run((ed) => ed.chain().focus().toggleBlockquote().run()),
  toggleBulletList: () => run((ed) => ed.chain().focus().toggleBulletList().run()),
  toggleOrderedList: () => run((ed) => ed.chain().focus().toggleOrderedList().run()),
  toggleTaskList: () => run((ed) => ed.chain().focus().toggleTaskList().run()),
  setHorizontalRule: () => run((ed) => ed.chain().focus().setHorizontalRule().run()),
  setHeading: (level: number) =>
    run((ed) => {
      if (level <= 0) ed.chain().focus().setParagraph().run();
      else ed.chain().focus().toggleHeading({ level: level as 1 | 2 | 3 | 4 | 5 | 6 }).run();
    }),
  setLink: () =>
    run((ed) => {
      const prev = ed.getAttributes("link").href as string | undefined;
      const url = window.prompt("URL", prev || "https://");
      if (url === null) return;
      if (!url) {
        ed.chain().focus().extendMarkRange("link").unsetLink().run();
        return;
      }
      ed.chain().focus().extendMarkRange("link").setLink({ href: url }).run();
    }),
  setImage: () =>
    run((ed) => {
      const url = window.prompt("Image URL", "https://");
      if (!url) return;
      ed.chain().focus().setImage({ src: url }).run();
    }),
  insertTable: (rows: number, cols: number) =>
    run((ed) => {
      ed.chain()
        .focus()
        .insertTable({
          rows: Math.max(1, Math.floor(rows)),
          cols: Math.max(1, Math.floor(cols)),
          withHeaderRow: true,
        })
        .run();
    }),
  insertText: (text: string) => run((ed) => ed.chain().focus().insertContent(text).run()),
});
</script>

<template>
  <div
    ref="root"
    class="md-wysiwyg"
    :style="{
      background,
      color: foreground,
      '--muted': muted,
      '--border': border,
      '--hl-border': highlightBg,
      '--hl-fg': highlightFg,
      '--link': foreground,
    }"
    @mousedown="emit('focus')"
    @click="onClick"
  >
    <EditorContent :editor="editor" />
  </div>
</template>

<style scoped>
.md-wysiwyg {
  height: 100%;
  overflow: auto;
  padding: 20px 28px;
  font-family: "IBM Plex Sans", "Segoe UI", system-ui, sans-serif;
  font-size: 14px;
  line-height: 1.65;
}

.md-wysiwyg :deep(.md-wysiwyg-prose) {
  outline: none;
  min-height: 100%;
}

.md-wysiwyg :deep(.md-wysiwyg-prose > .is-active) {
  background: color-mix(in srgb, var(--hl-border) 22%, transparent) !important;
  color: var(--hl-fg);
  border-radius: 4px;
  box-shadow: inset 0 0 0 2px var(--hl-border);
}

.md-wysiwyg :deep(.md-wysiwyg-prose p.is-editor-empty:first-child::before) {
  color: var(--muted);
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
}

.md-wysiwyg :deep(h1),
.md-wysiwyg :deep(h2),
.md-wysiwyg :deep(h3),
.md-wysiwyg :deep(h4),
.md-wysiwyg :deep(h5),
.md-wysiwyg :deep(h6) {
  margin: 1.2em 0 0.5em;
  font-weight: 600;
  line-height: 1.3;
}

.md-wysiwyg :deep(h1) {
  font-size: 1.7em;
  border-bottom: 1px solid var(--border);
  padding-bottom: 0.3em;
}

.md-wysiwyg :deep(h2) {
  font-size: 1.35em;
  border-bottom: 1px solid var(--border);
  padding-bottom: 0.25em;
}

.md-wysiwyg :deep(p) {
  margin: 0.75em 0;
}

.md-wysiwyg :deep(a) {
  color: color-mix(in srgb, var(--link) 70%, #4a8fd4);
}

.md-wysiwyg :deep(code) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  background: color-mix(in srgb, var(--muted) 22%, transparent);
  padding: 0.15em 0.4em;
  border-radius: 3px;
  font-size: 0.9em;
}

.md-wysiwyg :deep(pre) {
  background: color-mix(in srgb, var(--muted) 14%, transparent);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 12px 14px;
  overflow: auto;
}

.md-wysiwyg :deep(pre code) {
  background: transparent;
  padding: 0;
}

.md-wysiwyg :deep(blockquote) {
  margin: 0.8em 0;
  padding: 0.2em 0 0.2em 1em;
  border-left: 3px solid var(--border);
  color: var(--muted);
}

.md-wysiwyg :deep(ul),
.md-wysiwyg :deep(ol) {
  padding-left: 1.5em;
}

.md-wysiwyg :deep(ul[data-type="taskList"]) {
  list-style: none;
  padding-left: 0;
}

.md-wysiwyg :deep(ul[data-type="taskList"] li) {
  display: flex;
  gap: 0.5em;
  align-items: flex-start;
}

.md-wysiwyg :deep(table) {
  border-collapse: collapse;
  margin: 1em 0;
  width: 100%;
}

.md-wysiwyg :deep(th),
.md-wysiwyg :deep(td) {
  border: 1px solid var(--border);
  padding: 6px 10px;
  min-width: 2em;
  vertical-align: top;
}

.md-wysiwyg :deep(th) {
  background: color-mix(in srgb, var(--muted) 16%, transparent);
  font-weight: 600;
}

.md-wysiwyg :deep(hr) {
  border: none;
  border-top: 1px solid var(--border);
  margin: 1.5em 0;
}

.md-wysiwyg :deep(img) {
  max-width: 100%;
}
</style>
