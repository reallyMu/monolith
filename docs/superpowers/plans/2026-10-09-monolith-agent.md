# Monolith Agent P0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship P0 — multi-LLM settings, Pi Bridge sidecar, chat float, MCP + local/builtin skills, HTTP RAG retrieve-only.

**Architecture:** Extend `settings.json` with `llms` / `ragStores` / `mcpServers` / `skills`. Rust spawns/stops Node bridge; Vue float talks to bridge HTTP. RAG HTTP client in Rust or bridge; asset tools via existing APIs.

**Tech Stack:** Tauri 2, Vue 3, existing settings APIs, Pi bridge pattern from healix-cdh `third-party/pi/bridge`.

## File map

| Area | Files |
|---|---|
| Settings model | `src-tauri/src/settings.rs`, frontend settings UI in `App.vue` |
| Agent bridge | `src-tauri/src/agent_bridge.rs` (new), `lib.rs` commands |
| Chat UI | `src/components/AgentFloat.vue` (new) |
| RAG HTTP | `src-tauri/src/rag_http.rs` (new) |
| Skills | reuse/extend `mcp_install` skill paths + settings enable list |
| Spec | `docs/superpowers/specs/2026-10-09-monolith-agent-design.md` |

## Tasks

1. Extend `AppSettings` + load/save + `settingsGet` view for llms/rag/mcp/skills (defaults: oMLX chat preset).
2. Settings UI tabs/sections for the four areas (minimal, not over-designed).
3. `agent_bridge` start/stop/status using system `node`; bundle or path-config bridge script.
4. `AgentFloat.vue` chat → bridge; wire into `App.vue`.
5. HTTP RAG retrieve helper + inject into agent context when enabled.
6. Skills: list builtin + App Support dir; enable flags in settings.
7. Smoke: vue-tsc / cargo check; manual chat if oMLX up.
