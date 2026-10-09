<script setup lang="ts">
/**
 * Shell/ops cloned from healix-cdh PiAgentFloat:
 * FAB drag, dialog drag, Esc minimize, backdrop minimize, × clear,
 * tech chrome + avatar. Chat goes through bundled Pi Bridge (SSE).
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import piAvatar from "../assets/pi-agent-avatar.png";
import MarkdownWysiwyg from "./MarkdownWysiwyg.vue";
import {
  abortPiBridgePrompt,
  agentBridgeStart,
  agentBridgeStatus,
  agentRagRetrieve,
  streamPiBridgePrompt,
  type RagHit,
} from "../utils/agentApi";
import { guessAssetNameQuery, runAssetSearchTool } from "../utils/agentTools";
import { settingsGet } from "../utils/settingsApi";
import { useI18n } from "../i18n";

const { t } = useI18n();

const FAB_POS_KEY = "monolith-pi-float-pos";
const DIALOG_POS_KEY = "monolith-pi-dialog-pos";
const FAB_SIZE = 56;
const DRAG_THRESHOLD = 6;
const FAB_MARGIN = 24;

type Line = { role: "user" | "assistant" | "error"; text: string };

const dialogRef = ref<HTMLElement | null>(null);
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const dialogVisible = ref(false);
const fabPos = ref({ x: 0, y: 0 });
const dialogPos = ref({ x: 0, y: 0 });

const busy = ref(false);
const input = ref("");
const lines = ref<Line[]>([]);
const err = ref("");
const statusMsg = ref("");
const llmReady = ref<boolean | null>(null);
const bridgeUrl = ref("");
let promptAbort: AbortController | null = null;

const hasSession = computed(
  () => lines.value.length > 0 || !!input.value.trim(),
);

const fabDragging = ref(false);
let fabPointerId: number | null = null;
let fabStartClient = { x: 0, y: 0 };
let fabStartPos = { x: 0, y: 0 };
let fabMoved = false;

const titleDragging = ref(false);
let titlePointerId: number | null = null;
let titleStartClient = { x: 0, y: 0 };
let titleStartPos = { x: 0, y: 0 };

function dialogSize() {
  // 70% of previous 780px CDH-aligned width
  const w = Math.min(546, window.innerWidth - 40);
  const h = Math.min(window.innerHeight * 0.9, 960);
  return { w, h };
}

function clampFab(x: number, y: number) {
  const maxX = Math.max(8, window.innerWidth - FAB_SIZE - 8);
  const maxY = Math.max(8, window.innerHeight - FAB_SIZE - 8);
  return { x: Math.min(Math.max(8, x), maxX), y: Math.min(Math.max(8, y), maxY) };
}

function clampDialog(x: number, y: number) {
  const { w, h } = dialogSize();
  return {
    x: Math.min(Math.max(8, x), Math.max(8, window.innerWidth - w - 8)),
    y: Math.min(Math.max(8, y), Math.max(8, window.innerHeight - h - 8)),
  };
}

function defaultDialogPos() {
  const { w, h } = dialogSize();
  return clampDialog(window.innerWidth - w - 20, window.innerHeight - h - 20);
}

function loadPositions() {
  try {
    const fabRaw = localStorage.getItem(FAB_POS_KEY);
    if (fabRaw) {
      const p = JSON.parse(fabRaw) as { x?: number; y?: number };
      if (typeof p.x === "number" && typeof p.y === "number") {
        fabPos.value = clampFab(p.x, p.y);
      }
    } else {
      fabPos.value = clampFab(
        window.innerWidth - FAB_SIZE - FAB_MARGIN,
        window.innerHeight - FAB_SIZE - FAB_MARGIN - 72,
      );
    }
  } catch {
    fabPos.value = clampFab(
      window.innerWidth - FAB_SIZE - FAB_MARGIN,
      window.innerHeight - FAB_SIZE - 80,
    );
  }
  try {
    const dlgRaw = localStorage.getItem(DIALOG_POS_KEY);
    if (dlgRaw) {
      const p = JSON.parse(dlgRaw) as { x?: number; y?: number };
      if (typeof p.x === "number" && typeof p.y === "number") {
        dialogPos.value = clampDialog(p.x, p.y);
        return;
      }
    }
  } catch {
    /* ignore */
  }
  dialogPos.value = defaultDialogPos();
}

function saveFabPos() {
  localStorage.setItem(FAB_POS_KEY, JSON.stringify(fabPos.value));
}
function saveDialogPos() {
  localStorage.setItem(DIALOG_POS_KEY, JSON.stringify(dialogPos.value));
}

async function probeLlm() {
  try {
    const view = await settingsGet();
    llmReady.value = view.settings.llms.some(
      (l) => l.enabled && (l.role === "chat" || l.role === "both"),
    );
  } catch {
    llmReady.value = false;
  }
}

async function openDialog() {
  dialogVisible.value = true;
  void probeLlm();
  void ensureBridge().catch((e) => {
    err.value = String(e);
    llmReady.value = false;
  });
  await nextTick();
  dialogRef.value?.focus();
  textareaRef.value?.focus();
}

function abortActivePrompt() {
  if (!busy.value || !promptAbort) return;
  promptAbort.abort();
  if (bridgeUrl.value.trim()) {
    void abortPiBridgePrompt(bridgeUrl.value).catch(() => {
      /* abort is optional once the client already aborted the SSE stream */
    });
  }
}

function minimizeDialog() {
  abortActivePrompt();
  dialogVisible.value = false;
}

function dismissAndClear() {
  abortActivePrompt();
  lines.value = [];
  input.value = "";
  err.value = "";
  statusMsg.value = "";
  dialogVisible.value = false;
}

function onFabPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  fabDragging.value = true;
  fabMoved = false;
  fabPointerId = e.pointerId;
  fabStartClient = { x: e.clientX, y: e.clientY };
  fabStartPos = { ...fabPos.value };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onFabPointerMove(e: PointerEvent) {
  if (!fabDragging.value || fabPointerId !== e.pointerId) return;
  const dx = e.clientX - fabStartClient.x;
  const dy = e.clientY - fabStartClient.y;
  if (Math.abs(dx) > DRAG_THRESHOLD || Math.abs(dy) > DRAG_THRESHOLD) fabMoved = true;
  if (fabMoved) fabPos.value = clampFab(fabStartPos.x + dx, fabStartPos.y + dy);
}

function onFabPointerUp(e: PointerEvent) {
  if (fabPointerId !== e.pointerId) return;
  fabDragging.value = false;
  fabPointerId = null;
  if (fabMoved) saveFabPos();
  else void openDialog();
}

function onTitlePointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  if ((e.target as HTMLElement).closest("button")) return;
  titleDragging.value = true;
  titlePointerId = e.pointerId;
  titleStartClient = { x: e.clientX, y: e.clientY };
  titleStartPos = { ...dialogPos.value };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onTitlePointerMove(e: PointerEvent) {
  if (!titleDragging.value || titlePointerId !== e.pointerId) return;
  const dx = e.clientX - titleStartClient.x;
  const dy = e.clientY - titleStartClient.y;
  dialogPos.value = clampDialog(titleStartPos.x + dx, titleStartPos.y + dy);
}

function onTitlePointerUp(e: PointerEvent) {
  if (titlePointerId !== e.pointerId) return;
  titleDragging.value = false;
  titlePointerId = null;
  saveDialogPos();
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape" || !dialogVisible.value) return;
  e.preventDefault();
  e.stopPropagation();
  minimizeDialog();
}

function onResize() {
  fabPos.value = clampFab(fabPos.value.x, fabPos.value.y);
  dialogPos.value = clampDialog(dialogPos.value.x, dialogPos.value.y);
}

async function ensureBridge(): Promise<string> {
  statusMsg.value = t("agentBridgeStarting");
  let st = await agentBridgeStatus();
  if (!st.nodeOk) {
    throw new Error(st.nodeError || t("agentBridgeNeedNode"));
  }
  if (!st.up) {
    st = await agentBridgeStart();
  }
  if (!st.up) {
    throw new Error(st.hint || t("agentBridgeDown"));
  }
  if (!st.bridgeUrl?.trim()) {
    throw new Error(t("agentBridgeDown"));
  }
  bridgeUrl.value = st.bridgeUrl;
  llmReady.value = true;
  return bridgeUrl.value;
}

async function send() {
  const text = input.value.trim();
  if (!text || busy.value) return;
  busy.value = true;
  err.value = "";
  statusMsg.value = "";
  input.value = "";
  lines.value.push({ role: "user", text });
  await nextTick();

  try {
    const view = await settingsGet();
    const hasLlm = view.settings.llms.some(
      (l) => l.enabled && (l.role === "chat" || l.role === "both"),
    );
    if (!hasLlm) throw new Error(t("agentNoLlm"));

    const preferMd = /md|markdown|文档/.test(text);
    statusMsg.value = t("agentRetrieving");
    let prefix = "";
    const guessed = guessAssetNameQuery(text);
    if (guessed) {
      try {
        const block = await runAssetSearchTool({ query: guessed, recursive: "true" }, { preferMd });
        if (block && !/Asset search error/.test(block)) {
          // Fast path: clear name lookup without waiting on the agent loop.
          lines.value.push({ role: "assistant", text: block });
          statusMsg.value = "";
          return;
        }
        if (block) prefix += `\n\nAsset search results:\n${block}`;
      } catch (e) {
        prefix += `\n\nAsset search error: ${String(e)}`;
      }
    }

    try {
      const hits: RagHit[] = await agentRagRetrieve(text);
      if (hits.length) {
        prefix +=
          "\n\nRetrieved context:\n" +
          hits.map((h, i) => `[${i + 1}] ${h.text}`).join("\n\n");
      }
    } catch (e) {
      statusMsg.value = String(e);
    }

    const url = await ensureBridge();
    statusMsg.value = t("agentThinking");
    const message = prefix
      ? `${text}\n\n---\nHost context (Monolith):\n${prefix.trim()}`
      : text;

    promptAbort?.abort();
    promptAbort = new AbortController();
    const assistantIdx = lines.value.length;
    lines.value.push({ role: "assistant", text: "" });

    let streamErr: string | null = null;
    await streamPiBridgePrompt(
      url,
      message,
      (delta) => {
        const row = lines.value[assistantIdx];
        if (row && row.role === "assistant") {
          row.text += delta;
        }
      },
      (finalText, meta) => {
        const row = lines.value[assistantIdx];
        if (row && row.role === "assistant") {
          row.text = (finalText || row.text || (meta?.aborted ? "（已中断）" : "")).trim();
          if (!row.text) row.text = "（空回复）";
        }
      },
      (error) => {
        streamErr = error;
      },
      { signal: promptAbort.signal },
    );
    if (streamErr) throw new Error(streamErr);
    statusMsg.value = "";
  } catch (e) {
    err.value = String(e);
    lines.value.push({ role: "error", text: String(e) });
    statusMsg.value = "";
    llmReady.value = false;
  } finally {
    busy.value = false;
    promptAbort = null;
  }
}

function onTextareaKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !busy.value) {
    e.preventDefault();
    void send();
  }
}

function exportAllMd() {
  if (!lines.value.length) return;
  const body = lines.value
    .map((l) => {
      const who = l.role === "user" ? "User" : l.role === "error" ? "Error" : "Assistant";
      return `### ${who}\n\n${l.text}\n`;
    })
    .join("\n");
  const md = `# Monolith Agent\n\n${body}`;
  const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `monolith-agent-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.md`;
  a.click();
  URL.revokeObjectURL(a.href);
}

onMounted(() => {
  loadPositions();
  window.addEventListener("keydown", onKeyDown, true);
  window.addEventListener("resize", onResize);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown, true);
  window.removeEventListener("resize", onResize);
  document.body.style.overflow = "";
});

watch(dialogVisible, (visible) => {
  document.body.style.overflow = visible ? "hidden" : "";
  if (visible) void nextTick(() => dialogRef.value?.focus());
});
</script>

<template>
  <Teleport to="body">
    <Transition name="pi-float-fade">
      <div
        v-if="dialogVisible"
        class="pi-float-backdrop"
        aria-hidden="true"
        @click="minimizeDialog"
      />
    </Transition>

    <Transition name="pi-float-slide">
      <aside
        v-if="dialogVisible"
        ref="dialogRef"
        class="pi-float-dialog"
        role="dialog"
        aria-modal="true"
        :aria-label="t('agentTitle')"
        tabindex="-1"
        :style="{
          left: `${dialogPos.x}px`,
          top: `${dialogPos.y}px`,
          width: `${dialogSize().w}px`,
          height: `${dialogSize().h}px`,
        }"
        @keydown.esc.stop.prevent="minimizeDialog"
      >
        <div class="pi-float-dialog-glow" aria-hidden="true" />
        <div class="pi-float-dialog-grid" aria-hidden="true" />

        <header
          class="pi-float-dialog-head"
          :class="{ 'pi-float-dialog-head--dragging': titleDragging }"
          @pointerdown="onTitlePointerDown"
          @pointermove="onTitlePointerMove"
          @pointerup="onTitlePointerUp"
          @pointercancel="onTitlePointerUp"
        >
          <div class="pi-float-dialog-title">
            <span class="pi-float-dialog-avatar-wrap">
              <img :src="piAvatar" alt="" class="pi-float-dialog-avatar" width="32" height="32" />
              <span class="pi-float-dialog-pulse" />
            </span>
            <div class="pi-float-dialog-title-text">
              <span class="pi-float-dialog-name">{{ t("agentFloatName") }}</span>
              <span class="pi-float-dialog-sub">{{ t("agentFloatSub") }}</span>
            </div>
          </div>
          <div class="pi-float-dialog-head-actions">
            <span class="pi-float-hint" :title="t('agentEscHint')">{{ t("agentEscHint") }}</span>
            <button
              type="button"
              class="pi-float-close"
              :aria-label="t('agentDismissClear')"
              :title="t('agentDismissClear')"
              @click="dismissAndClear"
            >
              ×
            </button>
          </div>
        </header>

        <div class="pi-chat-panel pi-chat-panel--tech">
          <div v-if="err" class="pi-chat-banner pi-chat-banner--err">{{ err }}</div>
          <div v-else-if="llmReady === false" class="pi-chat-banner pi-chat-banner--warn">
            {{ t("agentNoLlm") }}
          </div>
          <div v-else-if="llmReady" class="pi-chat-banner pi-chat-banner--ok">
            {{ t("agentLlmReady") }}
          </div>
          <div v-if="statusMsg" class="pi-chat-banner pi-chat-banner--ok">{{ statusMsg }}</div>

          <div class="pi-chat-log" role="log" aria-live="polite">
            <p v-if="!lines.length" class="pi-chat-empty">{{ t("agentEmpty") }}</p>
            <div
              v-for="(line, i) in lines"
              :key="i"
              class="pi-chat-line"
              :class="line.role"
            >
              <span class="pi-chat-role">
                <span class="pi-chat-role-dot" :data-role="line.role" />
                {{
                  line.role === "user"
                    ? t("agentYou")
                    : line.role === "error"
                      ? t("agentError")
                      : t("agentBot")
                }}
              </span>
              <!-- Assistant: same TipTap MD preview as the workbench (Monaco is source-only). -->
              <div v-if="line.role === 'assistant'" class="pi-chat-md">
                <MarkdownWysiwyg
                  :content="line.text"
                  :editable="false"
                  background="transparent"
                  foreground="#e2e8f0"
                  muted="#94a3b8"
                  border="rgba(56, 189, 248, 0.18)"
                />
              </div>
              <pre v-else class="pi-chat-text">{{ line.text }}</pre>
            </div>
          </div>

          <footer class="pi-chat-input">
            <div class="pi-chat-input-row">
              <div class="pi-chat-textarea-wrap">
                <textarea
                  ref="textareaRef"
                  v-model="input"
                  class="pi-chat-textarea"
                  rows="2"
                  :placeholder="busy ? t('agentThinking') : t('agentPlaceholder')"
                  :disabled="busy"
                  @keydown="onTextareaKeydown"
                />
              </div>
            </div>
            <p class="pi-chat-input-hint">{{ t("agentInputHint") }}</p>
            <div class="pi-chat-actions">
              <button
                v-if="lines.length"
                type="button"
                class="pi-btn-ghost"
                :disabled="busy"
                @click="exportAllMd"
              >
                {{ t("agentExportAll") }}
              </button>
              <button type="button" class="pi-btn-ghost" :disabled="busy" @click="probeLlm">
                {{ t("agentRefresh") }}
              </button>
              <button
                type="button"
                class="pi-btn-primary"
                :disabled="busy || !input.trim()"
                @click="send"
              >
                {{ busy ? "…" : t("agentSend") }}
              </button>
            </div>
          </footer>
        </div>
      </aside>
    </Transition>

    <button
      v-show="!dialogVisible"
      type="button"
      class="pi-float-fab"
      :class="{
        'pi-float-fab--dragging': fabDragging,
        'pi-float-fab--session': hasSession,
      }"
      :style="{ left: `${fabPos.x}px`, top: `${fabPos.y}px` }"
      :aria-label="hasSession ? t('agentResume') : t('agentTitle')"
      @pointerdown="onFabPointerDown"
      @pointermove="onFabPointerMove"
      @pointerup="onFabPointerUp"
      @pointercancel="onFabPointerUp"
    >
      <span class="pi-float-fab-ring" aria-hidden="true" />
      <img :src="piAvatar" alt="Agent" class="pi-float-fab-img" draggable="false" />
      <span v-if="hasSession" class="pi-float-fab-badge" aria-hidden="true" />
    </button>
  </Teleport>
</template>

<style scoped>
.pi-float-fab {
  position: fixed;
  z-index: 10050;
  width: 56px;
  height: 56px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: linear-gradient(145deg, #0f172a, #1e293b);
  box-shadow:
    0 0 0 1px rgba(56, 189, 248, 0.35),
    0 8px 28px rgba(0, 20, 60, 0.45),
    0 0 24px rgba(56, 189, 248, 0.15);
  cursor: grab;
  touch-action: none;
  transition: box-shadow 0.25s, transform 0.2s;
}
.pi-float-fab:hover {
  transform: scale(1.06);
  box-shadow:
    0 0 0 2px rgba(56, 189, 248, 0.55),
    0 10px 32px rgba(0, 30, 80, 0.5),
    0 0 32px rgba(99, 102, 241, 0.25);
}
.pi-float-fab--dragging {
  cursor: grabbing;
}
.pi-float-fab--session .pi-float-fab-ring {
  animation: pi-orbit 3s linear infinite;
  opacity: 1;
}
.pi-float-fab-ring {
  position: absolute;
  inset: -4px;
  border-radius: 50%;
  border: 2px solid transparent;
  border-top-color: #38bdf8;
  border-right-color: #818cf8;
  opacity: 0.5;
  pointer-events: none;
}
.pi-float-fab-badge {
  position: absolute;
  top: 2px;
  right: 2px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #22d3ee;
  box-shadow: 0 0 8px #22d3ee;
}
.pi-float-fab-img {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  pointer-events: none;
  user-select: none;
}
.pi-float-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10040;
  background: rgba(2, 6, 23, 0.55);
  backdrop-filter: blur(3px);
}
.pi-float-dialog {
  position: fixed;
  z-index: 10045;
  width: min(546px, calc(100vw - 40px));
  height: min(90vh, 960px);
  outline: none;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
  overflow: hidden;
  background: linear-gradient(165deg, rgba(15, 23, 42, 0.97) 0%, rgba(8, 12, 24, 0.98) 100%);
  border: 1px solid rgba(56, 189, 248, 0.22);
  box-shadow:
    0 0 0 1px rgba(99, 102, 241, 0.12),
    0 24px 64px rgba(0, 0, 0, 0.55),
    0 0 48px rgba(56, 189, 248, 0.08),
    inset 0 1px 0 rgba(255, 255, 255, 0.06);
}
.pi-float-dialog-glow {
  position: absolute;
  top: -40%;
  left: -20%;
  width: 70%;
  height: 50%;
  background: radial-gradient(ellipse, rgba(56, 189, 248, 0.18) 0%, transparent 70%);
  pointer-events: none;
}
.pi-float-dialog-grid {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(rgba(56, 189, 248, 0.04) 1px, transparent 1px),
    linear-gradient(90deg, rgba(56, 189, 248, 0.04) 1px, transparent 1px);
  background-size: 24px 24px;
  mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.5) 0%, transparent 45%);
  pointer-events: none;
}
.pi-float-dialog-head {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.75rem 0.85rem;
  border-bottom: 1px solid rgba(56, 189, 248, 0.15);
  background: linear-gradient(90deg, rgba(56, 189, 248, 0.08) 0%, rgba(99, 102, 241, 0.06) 100%);
  flex-shrink: 0;
  cursor: grab;
  touch-action: none;
  user-select: none;
}
.pi-float-dialog-head--dragging {
  cursor: grabbing;
}
.pi-float-dialog-title {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  min-width: 0;
}
.pi-float-dialog-avatar-wrap {
  position: relative;
  flex-shrink: 0;
}
.pi-float-dialog-avatar {
  border-radius: 50%;
  object-fit: cover;
  border: 2px solid rgba(56, 189, 248, 0.45);
}
.pi-float-dialog-pulse {
  position: absolute;
  inset: -3px;
  border-radius: 50%;
  border: 1px solid rgba(34, 211, 238, 0.5);
  animation: pi-pulse 2.4s ease-out infinite;
  pointer-events: none;
}
.pi-float-dialog-title-text {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-width: 0;
}
.pi-float-dialog-name {
  font-weight: 700;
  font-size: 0.95rem;
  letter-spacing: 0.02em;
  background: linear-gradient(90deg, #e0f2fe, #38bdf8, #a5b4fc);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}
.pi-float-dialog-sub {
  font-size: 0.68rem;
  color: rgba(148, 163, 184, 0.9);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.pi-float-dialog-head-actions {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  flex-shrink: 0;
}
.pi-float-hint {
  font-size: 0.65rem;
  color: rgba(148, 163, 184, 0.65);
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  border: 1px solid rgba(148, 163, 184, 0.15);
}
.pi-float-close {
  width: 2rem;
  height: 2rem;
  border: 1px solid rgba(248, 113, 113, 0.35);
  border-radius: 8px;
  background: rgba(248, 113, 113, 0.08);
  font-size: 1.25rem;
  line-height: 1;
  color: #fca5a5;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}
.pi-float-close:hover {
  background: rgba(248, 113, 113, 0.2);
  border-color: rgba(248, 113, 113, 0.55);
  color: #fecaca;
}

.pi-chat-panel {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}
.pi-chat-banner {
  padding: 0.45rem 0.75rem;
  font-size: 0.78rem;
  flex-shrink: 0;
  border-bottom: 1px solid rgba(56, 189, 248, 0.12);
}
.pi-chat-banner--err {
  color: #fecaca;
  background: rgba(248, 113, 113, 0.12);
}
.pi-chat-banner--warn {
  color: #fde68a;
  background: rgba(234, 179, 8, 0.12);
}
.pi-chat-banner--ok {
  color: #a5f3fc;
  background: rgba(34, 211, 238, 0.08);
}
.pi-chat-log {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0.75rem 1rem;
}
.pi-chat-empty {
  margin: 0;
  color: rgba(148, 163, 184, 0.85);
  font-size: 0.85rem;
}
.pi-chat-line {
  margin-bottom: 0.65rem;
}
.pi-chat-role {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.72rem;
  font-weight: 600;
  margin-bottom: 0.2rem;
  color: #94a3b8;
}
.pi-chat-role-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #64748b;
}
.pi-chat-role-dot[data-role="user"] {
  background: #38bdf8;
}
.pi-chat-role-dot[data-role="assistant"] {
  background: #a78bfa;
}
.pi-chat-role-dot[data-role="error"] {
  background: #f87171;
}
.pi-chat-md {
  border: 1px solid rgba(56, 189, 248, 0.15);
  border-radius: 8px;
  background: rgba(15, 23, 42, 0.65);
  overflow: hidden;
  max-height: min(52vh, 520px);
  overflow-y: auto;
}
.pi-chat-md :deep(.md-wysiwyg) {
  height: auto;
  min-height: 0;
  padding: 0.55rem 0.75rem;
  font-size: 0.85rem;
  line-height: 1.55;
}
.pi-chat-md :deep(.md-wysiwyg-prose) {
  min-height: 0;
}
.pi-chat-md :deep(.md-wysiwyg-prose > :first-child) {
  margin-top: 0;
}
.pi-chat-md :deep(.md-wysiwyg-prose > :last-child) {
  margin-bottom: 0;
}
.pi-chat-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.82rem;
  line-height: 1.45;
  background: rgba(15, 23, 42, 0.65);
  border: 1px solid rgba(56, 189, 248, 0.15);
  border-radius: 8px;
  padding: 0.55rem 0.7rem;
  color: #e2e8f0;
}
.pi-chat-line.user .pi-chat-text {
  background: rgba(56, 189, 248, 0.1);
  border-color: rgba(56, 189, 248, 0.28);
}
.pi-chat-line.error .pi-chat-text {
  color: #fecaca;
  border-color: rgba(248, 113, 113, 0.35);
}
.pi-chat-input {
  flex-shrink: 0;
  padding: 0.65rem 0.75rem 0.75rem;
  border-top: 1px solid rgba(56, 189, 248, 0.15);
  background: rgba(8, 12, 24, 0.6);
}
.pi-chat-input-row {
  display: flex;
  gap: 0.5rem;
}
.pi-chat-textarea-wrap {
  flex: 1;
  min-width: 0;
}
.pi-chat-textarea {
  width: 100%;
  box-sizing: border-box;
  resize: none;
  border-radius: 10px;
  border: 1px solid rgba(56, 189, 248, 0.22);
  background: rgba(15, 23, 42, 0.85);
  color: #e2e8f0;
  padding: 0.55rem 0.7rem;
  font: inherit;
  font-size: 0.88rem;
  line-height: 1.4;
}
.pi-chat-textarea:focus {
  outline: none;
  border-color: rgba(56, 189, 248, 0.55);
  box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.15);
}
.pi-chat-input-hint {
  margin: 0.35rem 0 0.45rem;
  font-size: 0.68rem;
  color: rgba(148, 163, 184, 0.75);
}
.pi-chat-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.pi-btn-primary,
.pi-btn-ghost {
  border-radius: 8px;
  font-size: 0.82rem;
  padding: 0.35rem 0.85rem;
  cursor: pointer;
}
.pi-btn-primary {
  border: 1px solid rgba(56, 189, 248, 0.45);
  background: linear-gradient(135deg, #0ea5e9, #6366f1);
  color: #fff;
}
.pi-btn-primary:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.pi-btn-ghost {
  border: 1px solid rgba(148, 163, 184, 0.25);
  background: transparent;
  color: #cbd5e1;
}
.pi-btn-ghost:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

@keyframes pi-pulse {
  0% {
    transform: scale(1);
    opacity: 0.7;
  }
  100% {
    transform: scale(1.35);
    opacity: 0;
  }
}
@keyframes pi-orbit {
  to {
    transform: rotate(360deg);
  }
}
.pi-float-fade-enter-active,
.pi-float-fade-leave-active {
  transition: opacity 0.22s ease;
}
.pi-float-fade-enter-from,
.pi-float-fade-leave-to {
  opacity: 0;
}
.pi-float-slide-enter-active,
.pi-float-slide-leave-active {
  transition:
    opacity 0.24s ease,
    transform 0.24s ease;
}
.pi-float-slide-enter-from,
.pi-float-slide-leave-to {
  opacity: 0;
  transform: translateY(16px) scale(0.96);
}
</style>
