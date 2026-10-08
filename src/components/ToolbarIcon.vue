<script setup lang="ts">
import { computed } from "vue";

/**
 * Compact stroke icons for toolbars (Lucide-like paths, no icon package).
 * Tooltips/labels come from i18n via the parent button's title / aria-label.
 */
export type ToolbarIconName =
  | "filePlus"
  | "folderOpen"
  | "convert"
  | "save"
  | "saveAs"
  | "history"
  | "assetPlus"
  | "version"
  | "edit"
  | "eye"
  | "columns"
  | "fileSearch"
  | "undo"
  | "redo"
  | "search"
  | "braces"
  | "indent"
  | "outdent"
  | "bold"
  | "italic"
  | "strike"
  | "code"
  | "codeBlock"
  | "link"
  | "image"
  | "quote"
  | "list"
  | "listOrdered"
  | "checkSquare"
  | "minus"
  | "table"
  | "omega"
  | "sigma"
  | "chevronDown"
  | "refresh"
  | "collapseLeft"
  | "expandRight"
  | "plus"
  | "pencil"
  | "x"
  | "relocate"
  | "trash"
  | "reconvert"
  | "download"
  | "inbox"
  | "folderImport"
  | "plug"
  | "settings"
  | "bgLeft"
  | "bgRight";

type Seg =
  | { tag: "path"; d: string; opacity?: number }
  | { tag: "line"; x1: number; y1: number; x2: number; y2: number }
  | { tag: "polyline"; points: string }
  | { tag: "circle"; cx: number; cy: number; r: number }
  | { tag: "rect"; x: number; y: number; width: number; height: number; rx?: number; opacity?: number };

const props = withDefaults(
  defineProps<{
    name: ToolbarIconName;
    size?: number;
  }>(),
  { size: 15 },
);

const PATHS: Record<ToolbarIconName, Seg[]> = {
  filePlus: [
    { tag: "path", d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" },
    { tag: "polyline", points: "14 2 14 8 20 8" },
    { tag: "line", x1: 12, y1: 18, x2: 12, y2: 12 },
    { tag: "line", x1: 9, y1: 15, x2: 15, y2: 15 },
  ],
  folderOpen: [
    { tag: "path", d: "M5 19a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v1" },
    { tag: "path", d: "M3 17h16a2 2 0 0 0 2-2V10H5.5" },
  ],
  convert: [
    { tag: "path", d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" },
    { tag: "polyline", points: "14 2 14 8 20 8" },
    { tag: "path", d: "M8 13h8" },
    { tag: "path", d: "M8 17h5" },
    { tag: "path", d: "m16 11 2 2-2 2" },
  ],
  save: [
    { tag: "path", d: "M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" },
    { tag: "polyline", points: "17 21 17 13 7 13 7 21" },
    { tag: "polyline", points: "7 3 7 8 15 8" },
  ],
  saveAs: [
    { tag: "path", d: "M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" },
    { tag: "polyline", points: "17 21 17 13 7 13 7 21" },
    { tag: "polyline", points: "7 3 7 8 15 8" },
    { tag: "line", x1: 12, y1: 11, x2: 12, y2: 17 },
    { tag: "line", x1: 9, y1: 14, x2: 15, y2: 14 },
  ],
  history: [
    { tag: "path", d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" },
    { tag: "path", d: "M3 3v5h5" },
    { tag: "path", d: "M12 7v5l4 2" },
  ],
  assetPlus: [
    { tag: "path", d: "M4 19.5A2.5 2.5 0 0 1 6.5 17H20" },
    { tag: "path", d: "M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" },
    { tag: "line", x1: 12, y1: 8, x2: 12, y2: 14 },
    { tag: "line", x1: 9, y1: 11, x2: 15, y2: 11 },
  ],
  version: [
    { tag: "rect", x: 8, y: 2, width: 12, height: 16, rx: 2 },
    { tag: "path", d: "M4 6v14a2 2 0 0 0 2 2h12" },
    { tag: "line", x1: 11, y1: 8, x2: 17, y2: 8 },
    { tag: "line", x1: 11, y1: 12, x2: 17, y2: 12 },
  ],
  edit: [
    { tag: "path", d: "M12 20h9" },
    { tag: "path", d: "M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" },
  ],
  eye: [
    { tag: "path", d: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" },
    { tag: "circle", cx: 12, cy: 12, r: 3 },
  ],
  columns: [
    { tag: "rect", x: 3, y: 3, width: 18, height: 18, rx: 2 },
    { tag: "line", x1: 12, y1: 3, x2: 12, y2: 21 },
  ],
  fileSearch: [
    { tag: "path", d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" },
    { tag: "polyline", points: "14 2 14 8 20 8" },
    { tag: "circle", cx: 11.5, cy: 14.5, r: 2.5 },
    { tag: "path", d: "m13.5 16.5 2 2" },
  ],
  undo: [
    { tag: "path", d: "M3 7v6h6" },
    { tag: "path", d: "M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6.7 3L3 13" },
  ],
  redo: [
    { tag: "path", d: "M21 7v6h-6" },
    { tag: "path", d: "M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6.7 3L21 13" },
  ],
  search: [
    { tag: "circle", cx: 11, cy: 11, r: 7 },
    { tag: "line", x1: 21, y1: 21, x2: 16.65, y2: 16.65 },
  ],
  braces: [
    { tag: "path", d: "M8 3H7a2 2 0 0 0-2 2v5a2 2 0 0 1-2 2 2 2 0 0 1 2 2v5c0 1.1.9 2 2 2h1" },
    { tag: "path", d: "M16 3h1a2 2 0 0 1 2 2v5a2 2 0 0 0 2 2 2 2 0 0 0-2 2v5a2 2 0 0 1-2 2h-1" },
  ],
  indent: [
    { tag: "line", x1: 3, y1: 6, x2: 21, y2: 6 },
    { tag: "line", x1: 9, y1: 12, x2: 21, y2: 12 },
    { tag: "line", x1: 3, y1: 18, x2: 21, y2: 18 },
    { tag: "polyline", points: "3 8 7 12 3 16" },
  ],
  outdent: [
    { tag: "line", x1: 3, y1: 6, x2: 21, y2: 6 },
    { tag: "line", x1: 9, y1: 12, x2: 21, y2: 12 },
    { tag: "line", x1: 3, y1: 18, x2: 21, y2: 18 },
    { tag: "polyline", points: "7 8 3 12 7 16" },
  ],
  bold: [
    { tag: "path", d: "M6 4h8a4 4 0 0 1 0 8H6z" },
    { tag: "path", d: "M6 12h9a4 4 0 0 1 0 8H6z" },
  ],
  italic: [
    { tag: "line", x1: 19, y1: 4, x2: 10, y2: 4 },
    { tag: "line", x1: 14, y1: 20, x2: 5, y2: 20 },
    { tag: "line", x1: 15, y1: 4, x2: 9, y2: 20 },
  ],
  strike: [
    { tag: "path", d: "M16 4H9a3 3 0 0 0-2.8 4" },
    { tag: "path", d: "M14 12a4 4 0 0 1 0 8H6" },
    { tag: "line", x1: 4, y1: 12, x2: 20, y2: 12 },
  ],
  code: [
    { tag: "polyline", points: "16 18 22 12 16 6" },
    { tag: "polyline", points: "8 6 2 12 8 18" },
  ],
  codeBlock: [
    { tag: "polyline", points: "16 18 22 12 16 6" },
    { tag: "polyline", points: "8 6 2 12 8 18" },
    { tag: "line", x1: 12, y1: 2, x2: 12, y2: 22 },
  ],
  link: [
    { tag: "path", d: "M10 13a5 5 0 0 0 7.5.5l2-2a5 5 0 0 0-7.1-7.1l-1.2 1.2" },
    { tag: "path", d: "M14 11a5 5 0 0 0-7.5-.5l-2 2a5 5 0 0 0 7.1 7.1l1.2-1.2" },
  ],
  image: [
    { tag: "rect", x: 3, y: 3, width: 18, height: 18, rx: 2 },
    { tag: "circle", cx: 9, cy: 9, r: 2 },
    { tag: "path", d: "m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21" },
  ],
  quote: [
    { tag: "path", d: "M3 21c3 0 7-2 7-8V5H5v8h3" },
    { tag: "path", d: "M14 21c3 0 7-2 7-8V5h-5v8h3" },
  ],
  list: [
    { tag: "line", x1: 8, y1: 6, x2: 21, y2: 6 },
    { tag: "line", x1: 8, y1: 12, x2: 21, y2: 12 },
    { tag: "line", x1: 8, y1: 18, x2: 21, y2: 18 },
    { tag: "line", x1: 3, y1: 6, x2: 3.01, y2: 6 },
    { tag: "line", x1: 3, y1: 12, x2: 3.01, y2: 12 },
    { tag: "line", x1: 3, y1: 18, x2: 3.01, y2: 18 },
  ],
  listOrdered: [
    { tag: "line", x1: 10, y1: 6, x2: 21, y2: 6 },
    { tag: "line", x1: 10, y1: 12, x2: 21, y2: 12 },
    { tag: "line", x1: 10, y1: 18, x2: 21, y2: 18 },
    { tag: "path", d: "M4 6h1v4" },
    { tag: "path", d: "M4 10h2" },
    { tag: "path", d: "M6 18H4c0-1 2-2 2-3s-1-1.5-2-1" },
  ],
  checkSquare: [
    { tag: "rect", x: 3, y: 3, width: 18, height: 18, rx: 2 },
    { tag: "path", d: "m9 12 2 2 4-4" },
  ],
  minus: [{ tag: "line", x1: 5, y1: 12, x2: 19, y2: 12 }],
  table: [
    { tag: "rect", x: 3, y: 3, width: 18, height: 18, rx: 2 },
    { tag: "line", x1: 3, y1: 9, x2: 21, y2: 9 },
    { tag: "line", x1: 3, y1: 15, x2: 21, y2: 15 },
    { tag: "line", x1: 9, y1: 3, x2: 9, y2: 21 },
    { tag: "line", x1: 15, y1: 3, x2: 15, y2: 21 },
  ],
  sigma: [
    { tag: "path", d: "M4 4h14l-7 8 7 8H4" },
  ],
  omega: [
    { tag: "path", d: "M4 19a6 6 0 0 1 6-6 6 6 0 0 1 6 6" },
    { tag: "path", d: "M12 5a5 5 0 0 1 5 5v3" },
    { tag: "path", d: "M12 5a5 5 0 0 0-5 5v3" },
  ],
  chevronDown: [{ tag: "polyline", points: "6 9 12 15 18 9" }],
  refresh: [
    { tag: "polyline", points: "23 4 23 10 17 10" },
    { tag: "polyline", points: "1 20 1 14 7 14" },
    { tag: "path", d: "M3.5 9a9 9 0 0 1 14.8-3.4L23 10" },
    { tag: "path", d: "M20.5 15a9 9 0 0 1-14.8 3.4L1 14" },
  ],
  collapseLeft: [{ tag: "polyline", points: "15 18 9 12 15 6" }],
  expandRight: [{ tag: "polyline", points: "9 18 15 12 9 6" }],
  plus: [
    { tag: "line", x1: 12, y1: 5, x2: 12, y2: 19 },
    { tag: "line", x1: 5, y1: 12, x2: 19, y2: 12 },
  ],
  pencil: [
    { tag: "path", d: "M12 20h9" },
    { tag: "path", d: "M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" },
  ],
  x: [
    { tag: "line", x1: 18, y1: 6, x2: 6, y2: 18 },
    { tag: "line", x1: 6, y1: 6, x2: 18, y2: 18 },
  ],
  relocate: [
    { tag: "path", d: "M18 8a4 4 0 0 0-8 0c0 4 4 8 4 8s4-4 4-8z" },
    { tag: "circle", cx: 14, cy: 8, r: 1.2 },
    { tag: "path", d: "M5 20h14" },
    { tag: "path", d: "M8 16v4" },
    { tag: "path", d: "M16 16v4" },
  ],
  trash: [
    { tag: "polyline", points: "3 6 5 6 21 6" },
    { tag: "path", d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" },
  ],
  reconvert: [
    { tag: "path", d: "M21 12a9 9 0 1 1-3-6.7" },
    { tag: "polyline", points: "21 3 21 9 15 9" },
    { tag: "path", d: "M8 12h4l-1.5 4" },
  ],
  download: [
    { tag: "path", d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" },
    { tag: "polyline", points: "7 10 12 15 17 10" },
    { tag: "line", x1: 12, y1: 15, x2: 12, y2: 3 },
  ],
  inbox: [
    { tag: "polyline", points: "22 12 16 12 14 15 10 15 8 12 2 12" },
    { tag: "path", d: "M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" },
  ],
  folderImport: [
    { tag: "path", d: "M5 19a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5z" },
    { tag: "line", x1: 12, y1: 10, x2: 12, y2: 16 },
    { tag: "polyline", points: "9 13 12 16 15 13" },
  ],
  settings: [
    { tag: "circle", cx: 12, cy: 12, r: 3 },
    {
      tag: "path",
      d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z",
    },
  ],
  plug: [
    { tag: "path", d: "M12 22v-5" },
    { tag: "path", d: "M9 8V2" },
    { tag: "path", d: "M15 8V2" },
    { tag: "path", d: "M18 8v5a6 6 0 0 1-12 0V8z" },
  ],
  bgLeft: [
    { tag: "rect", x: 3, y: 4, width: 8, height: 16, rx: 1 },
    { tag: "rect", x: 13, y: 4, width: 8, height: 16, rx: 1, opacity: 0.35 },
  ],
  bgRight: [
    { tag: "rect", x: 3, y: 4, width: 8, height: 16, rx: 1, opacity: 0.35 },
    { tag: "rect", x: 13, y: 4, width: 8, height: 16, rx: 1 },
  ],
};

const segs = computed(() => PATHS[props.name]);
</script>

<template>
  <svg
    class="tb-icon"
    xmlns="http://www.w3.org/2000/svg"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-for="(s, i) in segs" :key="i">
      <path v-if="s.tag === 'path'" :d="s.d" :opacity="s.opacity" />
      <line v-else-if="s.tag === 'line'" :x1="s.x1" :y1="s.y1" :x2="s.x2" :y2="s.y2" />
      <polyline v-else-if="s.tag === 'polyline'" :points="s.points" />
      <circle v-else-if="s.tag === 'circle'" :cx="s.cx" :cy="s.cy" :r="s.r" />
      <rect
        v-else-if="s.tag === 'rect'"
        :x="s.x"
        :y="s.y"
        :width="s.width"
        :height="s.height"
        :rx="s.rx"
        :opacity="s.opacity"
      />
    </template>
  </svg>
</template>

<style scoped>
.tb-icon {
  display: block;
  flex: none;
}
</style>
