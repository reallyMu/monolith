<script setup lang="ts">
import { computed, ref } from "vue";

const props = withDefaults(
  defineProps<{
    content: string;
    background?: string;
  }>(),
  { background: "#ffffff" },
);

const emit = defineEmits<{
  scroll: [info: { scrollTop: number; scrollHeight: number; clientHeight: number }];
}>();

const frame = ref<HTMLIFrameElement | null>(null);
let syncing = false;

/** Sandboxed document: no scripts; browser-like layout. */
const srcdoc = computed(() => {
  const body = props.content || "";
  // If already a full document, inject base style only via wrapper meta; else wrap.
  const looksFull = /<html[\s>]/i.test(body) || /<!doctype/i.test(body);
  if (looksFull) return body;
  // Browser-like page chrome (not the editor theme).
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><style>
    html,body{margin:0;padding:12px 16px;background:#fff;color:#222;
    font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;}
    img,video{max-width:100%;height:auto;}
  </style></head><body>${body}</body></html>`;
});

function onFrameLoad() {
  const doc = frame.value?.contentDocument;
  if (!doc) return;
  doc.addEventListener(
    "scroll",
    () => {
      if (syncing) return;
      const el = doc.documentElement;
      emit("scroll", {
        scrollTop: el.scrollTop || doc.body.scrollTop,
        scrollHeight: el.scrollHeight || doc.body.scrollHeight,
        clientHeight: el.clientHeight || doc.body.clientHeight,
      });
    },
    { passive: true },
  );
}

defineExpose({
  setScrollRatio: (ratio: number) => {
    const doc = frame.value?.contentDocument;
    if (!doc) return;
    const el = doc.documentElement;
    const max = Math.max(0, el.scrollHeight - el.clientHeight);
    syncing = true;
    el.scrollTop = Math.max(0, Math.min(max, ratio * max));
    if (doc.body) doc.body.scrollTop = el.scrollTop;
    requestAnimationFrame(() => {
      syncing = false;
    });
  },
});
</script>

<template>
  <iframe
    ref="frame"
    class="html-preview"
    title="HTML preview"
    sandbox="allow-same-origin"
    :srcdoc="srcdoc"
    @load="onFrameLoad"
  />
</template>

<style scoped>
.html-preview {
  display: block;
  width: 100%;
  height: 100%;
  border: 0;
  background: v-bind(background);
}
</style>
