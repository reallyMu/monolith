<script setup lang="ts">
import { computed, ref, watch } from "vue";
import TreeNodeItem from "./TreeNodeItem.vue";
import type { TreeNode } from "./treeTypes";

const props = withDefaults(
  defineProps<{
    content: string;
    kind: "json" | "xml";
    background?: string;
    foreground?: string;
    muted?: string;
    border?: string;
  }>(),
  {
    background: "#15181e",
    foreground: "#e6e8ee",
    muted: "#9aa3b2",
    border: "rgba(255,255,255,0.1)",
  },
);

const emit = defineEmits<{
  scroll: [info: { scrollTop: number; scrollHeight: number; clientHeight: number }];
}>();

const root = ref<HTMLDivElement | null>(null);
const expanded = ref<Set<string>>(new Set());
let syncing = false;
let idSeq = 0;

function nextId(prefix: string) {
  return `${prefix}-${++idSeq}`;
}

function jsonToTree(value: unknown, key?: string): TreeNode {
  const id = nextId("j");
  if (value === null) return { id, label: key != null ? `${key}: null` : "null" };
  if (Array.isArray(value)) {
    return {
      id,
      label: key != null ? `${key}: Array(${value.length})` : `Array(${value.length})`,
      children: value.map((v, i) => jsonToTree(v, String(i))),
    };
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>);
    return {
      id,
      label: key != null ? `${key}: Object` : "Object",
      children: entries.map(([k, v]) => jsonToTree(v, k)),
    };
  }
  const lit = typeof value === "string" ? JSON.stringify(value) : String(value);
  return { id, label: key != null ? `${key}: ${lit}` : lit };
}

function xmlElementToTree(el: Element): TreeNode {
  const id = nextId("x");
  const attrs = [...el.attributes].map((a) => `@${a.name}="${a.value}"`).join(" ");
  const label = attrs ? `<${el.tagName} ${attrs}>` : `<${el.tagName}>`;
  const childEls = [...el.children];
  const text = [...el.childNodes]
    .filter((n) => n.nodeType === Node.TEXT_NODE)
    .map((n) => (n.textContent || "").trim())
    .filter(Boolean)
    .join(" ");
  const children: TreeNode[] = childEls.map(xmlElementToTree);
  if (text && !childEls.length) {
    children.push({
      id: nextId("t"),
      label: text.length > 120 ? `${text.slice(0, 120)}…` : text,
    });
  } else if (text && childEls.length) {
    children.unshift({
      id: nextId("t"),
      label: `#text ${text.length > 80 ? `${text.slice(0, 80)}…` : text}`,
    });
  }
  return { id, label, children: children.length ? children : undefined };
}

const parsed = computed(() => {
  idSeq = 0;
  try {
    if (props.kind === "json") {
      const data = JSON.parse(props.content || "null");
      return { ok: true as const, roots: [jsonToTree(data)], error: "" };
    }
    const doc = new DOMParser().parseFromString(props.content || "<root/>", "application/xml");
    const err = doc.querySelector("parsererror");
    if (err) {
      return {
        ok: false as const,
        roots: [] as TreeNode[],
        error: err.textContent?.trim() || "XML parse error",
      };
    }
    const el = doc.documentElement;
    return { ok: true as const, roots: el ? [xmlElementToTree(el)] : [], error: "" };
  } catch (e) {
    return { ok: false as const, roots: [] as TreeNode[], error: String(e) };
  }
});

watch(
  () => parsed.value,
  (p) => {
    const next = new Set<string>();
    const walk = (nodes: TreeNode[], depth: number) => {
      for (const n of nodes) {
        if (n.children?.length && depth < 2) {
          next.add(n.id);
          walk(n.children, depth + 1);
        }
      }
    };
    if (p.ok) walk(p.roots, 0);
    expanded.value = next;
  },
  { immediate: true },
);

function toggle(id: string) {
  const s = new Set(expanded.value);
  if (s.has(id)) s.delete(id);
  else s.add(id);
  expanded.value = s;
}

function onScroll() {
  const el = root.value;
  if (!el || syncing) return;
  emit("scroll", {
    scrollTop: el.scrollTop,
    scrollHeight: el.scrollHeight,
    clientHeight: el.clientHeight,
  });
}

defineExpose({
  setScrollRatio: (ratio: number) => {
    const el = root.value;
    if (!el) return;
    const max = Math.max(0, el.scrollHeight - el.clientHeight);
    syncing = true;
    el.scrollTop = Math.max(0, Math.min(max, ratio * max));
    requestAnimationFrame(() => {
      syncing = false;
    });
  },
});
</script>

<template>
  <div
    ref="root"
    class="tree-preview"
    :style="{
      background,
      color: foreground,
      '--muted': muted,
      '--border': border,
    }"
    @scroll.passive="onScroll"
  >
    <p v-if="!parsed.ok" class="tree-error">{{ parsed.error }}</p>
    <ul v-else class="tree-root">
      <TreeNodeItem
        v-for="n in parsed.roots"
        :key="n.id"
        :node="n"
        :expanded="expanded"
        @toggle="toggle"
      />
    </ul>
  </div>
</template>

<style scoped>
.tree-preview {
  height: 100%;
  overflow: auto;
  padding: 12px 14px;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: 1.45;
}

.tree-error {
  margin: 0;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: #b07070;
  white-space: pre-wrap;
}

.tree-root {
  list-style: none;
  margin: 0;
  padding: 0;
}
</style>
