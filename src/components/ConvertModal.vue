<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../i18n";

const props = defineProps<{
  open: boolean;
  tool: string;
  inputName: string;
  outputPath: string;
  phase: string;
  detail: string;
  cancelling: boolean;
}>();

const emit = defineEmits<{
  cancel: [];
}>();

const { t } = useI18n();

const title = computed(() => {
  if (props.cancelling) return t("convertCancelling");
  return t("convertRunning", { tool: props.tool || "…" });
});
</script>

<template>
  <div v-if="open" class="cv-lock" role="alertdialog" aria-modal="true">
    <div class="cv-panel">
      <h2 class="cv-title">{{ title }}</h2>
      <p class="cv-sub">{{ inputName }}</p>
      <p class="cv-out" :title="outputPath">→ {{ outputPath }}</p>
      <div class="cv-bar" aria-hidden="true">
        <div class="cv-bar-ind" />
      </div>
      <p class="cv-phase">{{ phase }}</p>
      <p v-if="detail" class="cv-detail">{{ detail }}</p>
      <button
        type="button"
        class="cv-cancel"
        :disabled="cancelling"
        @click="emit('cancel')"
      >
        {{ t("convertCancel") }}
      </button>
      <p class="cv-hint">{{ t("convertBlockedHint") }}</p>
    </div>
  </div>
</template>

<style scoped>
.cv-lock {
  position: fixed;
  inset: 0;
  z-index: 10000;
  background: rgba(12, 14, 18, 0.72);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: all;
  user-select: none;
}
.cv-panel {
  width: min(440px, calc(100vw - 32px));
  padding: 22px 24px 18px;
  border-radius: 10px;
  background: #1a1d24;
  color: #e8eaed;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  border: 1px solid #2c313a;
}
.cv-title {
  margin: 0 0 8px;
  font-size: var(--text-title);
  font-weight: var(--fw-semibold);
  line-height: var(--lh-tight);
}
.cv-sub,
.cv-out,
.cv-phase,
.cv-detail,
.cv-hint {
  margin: 0 0 6px;
  font-size: var(--text-sm);
  line-height: var(--lh-body);
  color: #9aa0a6;
  word-break: break-all;
}
.cv-out {
  margin-bottom: 14px;
}
.cv-bar {
  height: 6px;
  border-radius: 999px;
  background: #2c313a;
  overflow: hidden;
  margin-bottom: 12px;
}
.cv-bar-ind {
  height: 100%;
  width: 40%;
  border-radius: inherit;
  background: #5b9cff;
  animation: cv-slide 1.1s ease-in-out infinite;
}
@keyframes cv-slide {
  0% {
    transform: translateX(-120%);
  }
  100% {
    transform: translateX(320%);
  }
}
.cv-cancel {
  margin-top: 10px;
  width: 100%;
  padding: 8px 12px;
  border-radius: 6px;
  border: 1px solid #5a3a3a;
  background: #2a1c1c;
  color: #f0c0c0;
  cursor: pointer;
  font-size: var(--text-sm);
  font-weight: var(--fw-medium);
}
.cv-cancel:disabled {
  opacity: 0.5;
  cursor: default;
}
.cv-hint {
  margin-top: 10px;
  margin-bottom: 0;
  font-size: var(--text-xs);
  text-align: center;
}
</style>
