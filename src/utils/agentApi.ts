import { invoke } from "@tauri-apps/api/core";
import type { LlmEndpoint } from "./settingsApi";

export interface AgentSkillContent {
  id: string;
  name: string;
  path: string;
  body: string;
}

export interface RagHit {
  text: string;
  score?: number | null;
  meta?: unknown;
}

export function agentSkillsLoad(): Promise<AgentSkillContent[]> {
  return invoke("agent_skills_load");
}

/** In-process monolith MCP tool (same handlers as monolith-mcp stdio). */
export function agentToolCall(
  name: string,
  args: Record<string, unknown> = {},
): Promise<unknown> {
  return invoke("agent_tool_call", { req: { name, args } });
}

export function agentRuntimeStatus(): Promise<{
  skillsLoaded: { id: string; name: string; path: string; bytes: number }[];
  skillsConfigured: number;
  mcpServers: { id: string; name: string; enabled: boolean; command: string }[];
  bridgePort?: number;
  piAgentDir?: string;
}> {
  return invoke("agent_runtime_status");
}

export interface BridgeStatus {
  up: boolean;
  port: number;
  bridgeUrl: string;
  healthUrl: string;
  pid?: number | null;
  nodeOk: boolean;
  nodeVersion?: string | null;
  nodeError?: string | null;
  agentDir: string;
  workdir: string;
  bridgeScript: string;
  health?: unknown;
  hint?: string | null;
}

export function agentBridgeStatus(): Promise<BridgeStatus> {
  return invoke("agent_bridge_status");
}

export function agentBridgeStart(): Promise<BridgeStatus> {
  return invoke("agent_bridge_start");
}

export function agentBridgeStop(): Promise<BridgeStatus> {
  return invoke("agent_bridge_stop");
}

export type StreamPiDoneMeta = { aborted?: boolean };

export type PiSseHandlers = {
  onDelta: (delta: string) => void;
  onDone: (text: string, meta?: StreamPiDoneMeta) => void;
  onError: (error: string) => void;
};

/** Parse one SSE event block from Pi Bridge. Exported for unit tests. */
export function dispatchPiSseBlock(
  block: string,
  streamedText: string,
  handlers: PiSseHandlers,
): { streamedText: string; terminal: "done" | "error" | null } {
  const lines = block.split("\n");
  let event = "message";
  let data = "";
  for (const line of lines) {
    if (line.startsWith("event:")) event = line.slice(6).trim();
    else if (line.startsWith("data:")) data += line.slice(5).trim();
  }
  if (!data) return { streamedText, terminal: null };
  let payload: {
    delta?: string;
    text?: string;
    error?: string;
    aborted?: boolean;
  };
  try {
    payload = JSON.parse(data) as typeof payload;
  } catch {
    handlers.onError("SSE parse failed");
    return { streamedText, terminal: "error" };
  }
  if (event === "delta" && payload.delta) {
    const next = streamedText + payload.delta;
    handlers.onDelta(payload.delta);
    return { streamedText: next, terminal: null };
  }
  if (event === "done") {
    const text = payload.text !== undefined ? payload.text : streamedText;
    handlers.onDone(text, payload.aborted ? { aborted: true } : undefined);
    return { streamedText: text, terminal: "done" };
  }
  if (event === "error") {
    if (!payload.error) {
      handlers.onError("bridge error event missing error field");
      return { streamedText, terminal: "error" };
    }
    handlers.onError(payload.error);
    return { streamedText, terminal: "error" };
  }
  return { streamedText, terminal: null };
}

export function requireBridgeBaseUrl(bridgeUrl: string): string {
  const base = bridgeUrl.trim().replace(/\/$/, "");
  if (!base) throw new Error("bridge URL empty");
  return base;
}

/** POST /api/prompt on the local Pi Bridge and consume SSE (delta / done / error). */
export async function streamPiBridgePrompt(
  bridgeUrl: string,
  message: string,
  onDelta: (delta: string) => void,
  onDone: (text: string, meta?: StreamPiDoneMeta) => void,
  onError: (error: string) => void,
  options?: { signal?: AbortSignal },
): Promise<void> {
  let terminal = false;
  let streamedText = "";
  const finishDone = (text: string, meta?: StreamPiDoneMeta) => {
    if (terminal) return;
    terminal = true;
    onDone(text, meta);
  };
  const finishError = (arg: string) => {
    if (terminal) return;
    terminal = true;
    onError(arg);
  };

  let base: string;
  try {
    base = requireBridgeBaseUrl(bridgeUrl);
  } catch (e) {
    finishError(e instanceof Error ? e.message : String(e));
    return;
  }
  const signal = options?.signal;
  const handlers: PiSseHandlers = {
    onDelta,
    onDone: finishDone,
    onError: finishError,
  };

  try {
    const res = await fetch(`${base}/api/prompt`, {
      method: "POST",
      credentials: "omit",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message }),
      signal,
    });

    if (!res.ok || !res.body) {
      const t = await res.text().catch(() => "");
      finishError(`Bridge ${res.status}: ${t || res.statusText}`);
      return;
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      if (signal?.aborted) {
        await reader.cancel().catch(() => undefined);
        finishDone(streamedText, { aborted: true });
        return;
      }
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let idx;
      while ((idx = buffer.indexOf("\n\n")) !== -1) {
        const block = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 2);
        const r = dispatchPiSseBlock(block, streamedText, handlers);
        streamedText = r.streamedText;
        if (r.terminal) return;
      }
    }
    if (buffer.trim()) {
      const r = dispatchPiSseBlock(buffer, streamedText, handlers);
      streamedText = r.streamedText;
      if (r.terminal) return;
    }
    if (!terminal) finishDone(streamedText);
  } catch (e) {
    if (e instanceof Error && e.name === "AbortError") {
      finishDone(streamedText, { aborted: true });
      return;
    }
    finishError(e instanceof Error ? e.message : String(e));
  }
}

export async function abortPiBridgePrompt(bridgeUrl: string): Promise<void> {
  const base = bridgeUrl.trim().replace(/\/$/, "");
  if (!base) return;
  const res = await fetch(`${base}/api/abort`, { method: "POST", credentials: "omit" });
  if (!res.ok) {
    throw new Error(`Bridge abort ${res.status}`);
  }
}

/** Tools from every enabled MCP server (monolith + external stdio). */
export function agentMcpTools(): Promise<{
  servers: {
    id: string;
    name: string;
    kind: string;
    error?: string;
    tools: { name: string; description: string }[];
  }[];
}> {
  return invoke("agent_mcp_tools");
}

export function agentRagRetrieve(query: string, storeId = ""): Promise<RagHit[]> {
  return invoke("agent_rag_retrieve", { req: { query, storeId } });
}

export function agentActiveChatLlm(): Promise<LlmEndpoint | null> {
  return invoke("agent_active_chat_llm");
}

/** Exported for unit tests. */
export function llmHeaders(llm: LlmEndpoint): Record<string, string> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const key = llm.apiKey.trim();
  if (key) headers.Authorization = `Bearer ${key}`;
  return headers;
}

/** Exported for unit tests. */
export function llmBase(llm: LlmEndpoint): string {
  const base = llm.baseUrl.trim().replace(/\/$/, "");
  if (!base) throw new Error("base URL empty");
  return base;
}

/** Exported for unit tests. */
export function requireChatModel(llm: LlmEndpoint): string {
  const model = llm.model.trim();
  if (!model) throw new Error(`LLM «${llm.name || llm.id}» has empty model`);
  return model;
}

/** OpenAI-compatible chat (settings “Test” path). */
export async function chatCompletions(
  llm: LlmEndpoint,
  messages: { role: string; content: string }[],
): Promise<string> {
  const model = requireChatModel(llm);
  const res = await fetch(`${llmBase(llm)}/chat/completions`, {
    method: "POST",
    headers: llmHeaders(llm),
    body: JSON.stringify({ model, messages, temperature: 0.3 }),
  });
  if (!res.ok) {
    const t = await res.text().catch(() => "");
    throw new Error(`LLM ${res.status} (${llm.name || llm.id}): ${t || res.statusText}`);
  }
  const data = (await res.json()) as {
    choices?: { message?: { content?: string } }[];
  };
  const text = data.choices?.[0]?.message?.content?.trim();
  if (!text) throw new Error("Empty LLM response");
  return text;
}

/**
 * Probe an OpenAI-compatible endpoint.
 * Uses GET /models when available; otherwise a single chat completion (requires model).
 */
export async function testLlmConnection(llm: LlmEndpoint): Promise<string> {
  const base = llmBase(llm);
  const headers = llmHeaders(llm);
  const modelsRes = await fetch(`${base}/models`, { method: "GET", headers });
  if (modelsRes.ok) {
    const data = (await modelsRes.json().catch(() => null)) as {
      data?: { id?: string }[];
    } | null;
    const ids = (data?.data ?? []).map((m) => m.id).filter(Boolean) as string[];
    if (ids.length) {
      const configured = llm.model.trim();
      if (configured && !ids.includes(configured)) {
        return `ok · ${ids.length} models (configured «${configured}» not listed)`;
      }
      if (configured) return `model ${configured} ok`;
      return `ok · ${ids.length} models (e.g. ${ids.slice(0, 3).join(", ")})`;
    }
    return "ok · /models reachable";
  }
  const reply = await chatCompletions(llm, [
    { role: "user", content: "Reply with exactly: pong" },
  ]);
  return `ok · chat (${reply.slice(0, 48)})`;
}

/** Smoke-test one RAG store (HTTP retrieve or pgvector extension probe). */
export function testRagConnection(storeId: string): Promise<string> {
  return invoke("agent_rag_test", { storeId });
}
