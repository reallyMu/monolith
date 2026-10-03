<script setup lang="ts">
import type { TreeNode } from "./treeTypes";

defineProps<{
  node: TreeNode;
  expanded: Set<string>;
}>();

const emit = defineEmits<{
  toggle: [id: string];
}>();
</script>

<template>
  <li class="tree-node">
    <button
      type="button"
      class="tree-row"
      :class="node.children?.length ? 'has-kids' : 'leaf'"
      @click="node.children?.length && emit('toggle', node.id)"
    >
      <span class="twist">{{ node.children?.length ? (expanded.has(node.id) ? "▾" : "▸") : "·" }}</span>
      <span class="lbl">{{ node.label }}</span>
    </button>
    <ul v-if="node.children?.length && expanded.has(node.id)" class="tree-children">
      <TreeNodeItem
        v-for="c in node.children"
        :key="c.id"
        :node="c"
        :expanded="expanded"
        @toggle="emit('toggle', $event)"
      />
    </ul>
  </li>
</template>

<style scoped>
.tree-children {
  list-style: none;
  margin: 0;
  padding: 0 0 0 1.1em;
  border-left: 1px solid var(--border);
  margin-left: 0.45em;
}

.tree-row {
  display: flex;
  gap: 6px;
  align-items: flex-start;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  color: inherit;
  padding: 2px 4px;
  border-radius: 3px;
  cursor: default;
  font: inherit;
}

.tree-row.has-kids {
  cursor: pointer;
}

.tree-row.has-kids:hover {
  background: color-mix(in srgb, var(--muted) 18%, transparent);
}

.twist {
  color: var(--muted);
  width: 1em;
  flex: none;
  user-select: none;
}

.lbl {
  word-break: break-word;
}
</style>
