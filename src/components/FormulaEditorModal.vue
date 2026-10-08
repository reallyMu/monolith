<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import katex from "katex";
import "katex/dist/katex.min.css";
import type { MathMode } from "../utils/mdMathRange";
import { useI18n } from "../i18n";

export type FormulaTemplate = { label: string; snip: string };

const props = withDefaults(
  defineProps<{
    open: boolean;
    latex?: string;
    mode?: MathMode;
    /** true → confirm label "Update" */
    editing?: boolean;
  }>(),
  {
    latex: "",
    mode: "block",
    editing: false,
  },
);

const emit = defineEmits<{
  close: [];
  confirm: [payload: { latex: string; mode: MathMode }];
}>();

const { t } = useI18n();

const localLatex = ref("");
const localMode = ref<MathMode>("block");
const ta = ref<HTMLTextAreaElement | null>(null);
const previewEl = ref<HTMLElement | null>(null);

const TEMPLATES: FormulaTemplate[] = [
  { label: "a/b", snip: "\\frac{a}{b}" },
  { label: "√", snip: "\\sqrt{x}" },
  { label: "x²", snip: "x^{2}" },
  { label: "xₙ", snip: "x_{n}" },
  { label: "∫", snip: "\\int_{a}^{b}" },
  { label: "∑", snip: "\\sum_{i=1}^{n}" },
  { label: "lim", snip: "\\lim_{x \\to 0}" },
  { label: "matrix", snip: "\\begin{matrix} a & b \\\\ c & d \\end{matrix}" },
  { label: "α", snip: "\\alpha" },
  { label: "β", snip: "\\beta" },
  { label: "π", snip: "\\pi" },
  { label: "θ", snip: "\\theta" },
  { label: "∞", snip: "\\infty" },
  { label: "±", snip: "\\pm" },
  { label: "→", snip: "\\rightarrow" },
];

const canConfirm = computed(() => localLatex.value.trim().length > 0);

watch(
  () => props.open,
  (v) => {
    if (!v) return;
    localLatex.value = props.latex ?? "";
    localMode.value = props.mode ?? "block";
    void nextTick(() => {
      ta.value?.focus();
      renderPreview();
    });
  },
);

watch([localLatex, localMode], () => {
  renderPreview();
});

function renderPreview() {
  const el = previewEl.value;
  if (!el) return;
  el.classList.remove("formula-modal__preview--err");
  const tex = localLatex.value.trim();
  if (!tex) {
    el.innerHTML = "";
    return;
  }
  try {
    katex.render(tex, el, {
      displayMode: localMode.value === "block",
      throwOnError: false,
      strict: "ignore",
    });
  } catch {
    el.classList.add("formula-modal__preview--err");
    el.textContent = tex;
  }
}

function insertSnippet(snip: string) {
  const el = ta.value;
  if (!el) {
    localLatex.value += snip;
    return;
  }
  const start = el.selectionStart ?? localLatex.value.length;
  const end = el.selectionEnd ?? start;
  const before = localLatex.value.slice(0, start);
  const after = localLatex.value.slice(end);
  localLatex.value = before + snip + after;
  void nextTick(() => {
    const pos = start + snip.length;
    el.focus();
    el.setSelectionRange(pos, pos);
  });
}

function onConfirm() {
  if (!canConfirm.value) return;
  emit("confirm", { latex: localLatex.value.trim(), mode: localMode.value });
}

function onKeydown(e: KeyboardEvent) {
  if (!props.open) return;
  if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && canConfirm.value) {
    e.preventDefault();
    onConfirm();
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="formula-modal-root" @mousedown.self="emit('close')">
      <div class="formula-modal" role="dialog" :aria-label="t('formulaTitle')" @mousedown.stop>
        <header class="formula-modal__head">
          <span class="formula-modal__title">{{ t("formulaTitle") }}</span>
          <div class="formula-modal__modes">
            <button
              type="button"
              class="formula-modal__mode"
              :class="{ active: localMode === 'block' }"
              @click="localMode = 'block'"
            >
              {{ t("formulaBlock") }}
            </button>
            <button
              type="button"
              class="formula-modal__mode"
              :class="{ active: localMode === 'inline' }"
              @click="localMode = 'inline'"
            >
              {{ t("formulaInline") }}
            </button>
          </div>
        </header>

        <div class="formula-modal__templates">
          <button
            v-for="tpl in TEMPLATES"
            :key="tpl.label + tpl.snip"
            type="button"
            class="formula-modal__tpl"
            :title="tpl.snip"
            @click="insertSnippet(tpl.snip)"
          >
            {{ tpl.label }}
          </button>
        </div>

        <textarea
          ref="ta"
          v-model="localLatex"
          class="formula-modal__ta"
          spellcheck="false"
          :placeholder="t('formulaPlaceholder')"
          rows="5"
        />

        <div ref="previewEl" class="formula-modal__preview" />

        <footer class="formula-modal__foot">
          <button type="button" class="formula-modal__btn" @click="emit('close')">
            {{ t("cancel") }}
          </button>
          <button
            type="button"
            class="formula-modal__btn formula-modal__btn--primary"
            :disabled="!canConfirm"
            @click="onConfirm"
          >
            {{ editing ? t("formulaUpdate") : t("formulaInsert") }}
          </button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.formula-modal-root {
  position: fixed;
  inset: 0;
  z-index: 10050;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.45);
}
.formula-modal {
  width: min(520px, calc(100vw - 32px));
  max-height: calc(100vh - 48px);
  overflow: auto;
  background: var(--panel, #1c2129);
  color: var(--text, #e6e8ee);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  padding: 14px 16px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.formula-modal__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.formula-modal__title {
  font-size: 14px;
  font-weight: 600;
}
.formula-modal__modes {
  display: flex;
  gap: 4px;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 6px;
  padding: 2px;
}
.formula-modal__mode {
  border: none;
  background: transparent;
  color: inherit;
  opacity: 0.7;
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 4px;
  cursor: pointer;
}
.formula-modal__mode.active {
  background: rgba(255, 255, 255, 0.12);
  opacity: 1;
}
.formula-modal__templates {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.formula-modal__tpl {
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.04);
  color: inherit;
  font-size: 12px;
  padding: 3px 8px;
  border-radius: 4px;
  cursor: pointer;
}
.formula-modal__tpl:hover {
  background: rgba(255, 255, 255, 0.1);
}
.formula-modal__ta {
  width: 100%;
  box-sizing: border-box;
  resize: vertical;
  min-height: 96px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 13px;
  line-height: 1.45;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(0, 0, 0, 0.25);
  color: inherit;
}
.formula-modal__preview {
  /* Fixed box so block/inline KaTeX height change does not resize the dialog. */
  height: 120px;
  min-height: 120px;
  max-height: 120px;
  box-sizing: border-box;
  padding: 10px 12px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(0, 0, 0, 0.2);
  overflow: auto;
  color: inherit;
  display: flex;
  align-items: center;
  justify-content: center;
}
.formula-modal__preview :deep(.katex-display) {
  margin: 0;
}
.formula-modal__preview--err {
  color: #e8a0a0;
  font-family: ui-monospace, Menlo, monospace;
  font-size: 12px;
  white-space: pre-wrap;
}
.formula-modal__foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.formula-modal__btn {
  border: 1px solid rgba(255, 255, 255, 0.14);
  background: rgba(255, 255, 255, 0.06);
  color: inherit;
  font-size: 13px;
  padding: 6px 14px;
  border-radius: 6px;
  cursor: pointer;
}
.formula-modal__btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.formula-modal__btn--primary {
  background: #3d6ea8;
  border-color: #4a7bb8;
}
.formula-modal__btn--primary:hover:not(:disabled) {
  background: #4a7bb8;
}
</style>
