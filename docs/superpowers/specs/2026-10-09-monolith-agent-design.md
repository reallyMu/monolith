# Monolith — 内嵌 Pi Agent（LLM / MCP / Skill / RAG）

> 状态：已批准（对话定稿后用户「开工」）  
> 日期：2026-10-09  
> 版本目标：v0.3.x 系列

## 决定

1. **集成 Pi Agent**：以 Bridge sidecar 运行（`@earendil-works/pi-coding-agent` 同源能力）；优先依赖**系统 Node**，控制安装包体积（相对现网 ~40MB App，增量目标约 +40～80MB，不强制自带 Node runtime）。
2. **多 LLM 配置**：OpenAI 兼容多条；字段含名称、`baseUrl`、`apiKey`、`model`、角色（`chat` / `embed` / 两者）。**oMLX** 与 **Ollama** 均可配置；默认预设偏向本机 oMLX（`http://127.0.0.1:8000/v1`），不绑死、不要求本机已装某一家。
3. **RAG**：外部库负责向量**存储**；ML 只做向量**检索**并拼进 Agent 上下文。P0 = 通用 HTTP 适配器；每条库可选 `queryMode`: `text` | `vector`。P1 = pgvector 直连。
4. **MCP**：内嵌 Agent 可配置启用的 MCP server 列表（含本机 `monolith-mcp`）；对外安装给 Cursor 等的能力保留。
5. **Skill（全开 C）**：内置（至少 `monolith-assets`）+ 本地目录（App Support `skills/` 与用户追加路径）+ URL/git 拉取后加载。P0 先做内置+本地；URL/git 进 P1 亦可提前若工期允许。
6. **资产工具**：进程内优先走现有 assets API（与 MCP 工具语义对齐）；写/删需确认（P1）。
7. **UI**：设置分区（LLM / RAG / MCP / Skill）+ 悬浮 Agent 聊天窗。

## 打包（Pi Agent 一等模块）

- 运行时位于仓库 `third-party/pi-agent/`（Bridge + npm `@earendil-works/pi-coding-agent`），经 Tauri `bundle.resources` **随安装包交付**。
- 运行期配置与会话写入 `~/Library/Application Support/com.muqiang.monolith/pi-agent/`；Monolith 设置中的 LLM / MCP / Skill 在 Bridge 启动前同步为 `.pi-agent` 的 models/auth/settings/mcp.json。
- 依赖**系统 Node ≥ 22.19**（不强制自带 Node runtime；P2 可选）。

## 非目标

- ML 内建向量库 / 切块入库  
- 将 healix-cdh 整棵 `third-party/pi` **开发树 / source monorepo** 打进安装包（用 npm 生产依赖即可）  
- 默认唯一依赖 Ollama

## 分期

| 期 | 内容 |
|---|---|
| **P0** | 多 LLM 设置与持久化；Pi Bridge 启停；悬浮聊天；MCP 配置；Skill 内置+本地启用；HTTP RAG（text/vector）只读检索进上下文；资产只读工具 |
| **P1** | Skill URL/git；写工具确认；pgvector 适配器；当前打开文档上下文 |
| **P2** | 会话历史增强、更多 HTTP 预设、可选自带 Node runtime |

## 验收（P0）

1. 设置可增删多条 LLM，切换后聊天走对应 `baseUrl`/`model`。  
2. Bridge 随需要启动；聊天窗可对话（本机 oMLX 或已配置的兼容端点可用时）。  
3. 可启用 `monolith-mcp` 与内置/本地 skill。  
4. 配置一条 HTTP RAG 后，提问可带上检索片段（库可用时）。  
5. 不在 ML 内写入任何向量存储。
