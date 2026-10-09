# Monolith v0.3.1 — 发布说明

日期：2026-10-09

在 **v0.2.2** 之上：内嵌 **Pi Agent**（随应用打包）、多 LLM / RAG / MCP / Skill 设置、资产树与 Agent 工具加固。聊天走本机 Bridge sidecar（需系统 **Node ≥ 22.19**）。

## 下载

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | [`Monolith_0.3.1_aarch64.dmg`](https://github.com/reallyMu/monolith/releases/download/v0.3.1/Monolith_0.3.1_aarch64.dmg) |
| Windows (x64) | [`Monolith_0.3.1_x64-setup.exe`](https://github.com/reallyMu/monolith/releases/download/v0.3.1/Monolith_0.3.1_x64-setup.exe) |

发布页：https://github.com/reallyMu/monolith/releases/tag/v0.3.1

---

## 本版亮点

### Pi Agent（随安装包交付）

- 生产 Bridge + `@earendil-works/pi-coding-agent` 打进 App Resources（`third-party/pi-agent/`）
- 运行期配置：`~/Library/Application Support/com.muqiang.monolith/pi-agent/`（Windows：对应 AppData）
- 设置中的 LLM / MCP / Skill 启动前同步为 Pi `models.json` / `auth.json` / `settings.json` / `mcp.json`
- 悬浮窗走 Bridge SSE；RAG / 资产名检索作为宿主上下文前缀
- **不**依赖 healix-cdh；需本机 Node ≥ 22.19（设置 → LLM 可见 Bridge 状态）

### 设置：LLM / RAG / MCP / Skill

- 多条 OpenAI 兼容 LLM（oMLX / Ollama / 云端）
- 多向量库连接（HTTP Search、pgvector、Qdrant、Milvus、Chroma 等）+ 连接测试
- MCP / Skill 可无限增删；启用项注入 Agent

### 资产与体验

- 资产树拖动为**移动**挂载（非镜像）；默认折叠
- 可配置 `assetsDir`；Agent/MCP 新建文件默认落盘目录

### 编码约定

- 去掉伪造默认 LLM / API Key、双路径 MCP 兜底、静默填 RAG 缺省；失败显式报错
- 单元测试：`npm run test:agent`（Bridge 配置 + SSE + tools + RAG）

---

## 校验（发布前）

- `vue-tsc --noEmit`
- `npm run test:agent`
- `cargo test --manifest-path src-tauri/Cargo.toml agent_bridge`
- macOS：`npm run tauri build` → DMG
- Windows：GitHub Actions `Build Windows`（tag `v*`）→ NSIS
