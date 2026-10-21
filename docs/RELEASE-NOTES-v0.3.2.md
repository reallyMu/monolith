# Monolith v0.3.2

在 **v0.3.1** 之上：Pi Agent **开箱即用**（内置 Node）+ Bridge 端口可配置/自避让。

## 变更

- **内置 Node v22.19.0**：随安装包交付（`Resources/third-party/node`），正式版不再要求用户本机安装 Node；开发机仍可用系统 Node 兜底。
- **Bridge 首选端口**：设置 → LLM 可编辑；占用时自动顺延空闲端口并写回设置（避免与 Atlas/WebAPI 等冲突）。
- **CORS**：Bridge 响应允许 WebView 跨域，修复悬浮 Agent「Load failed」。
- **设置 UI**：Bridge 卡片精简；状态显示实际端口与 Node 来源（`bundled` / `system`）。

## 体积（aarch64）

- 安装态 `.app` 约 **326 MB**（含内置 Node ~106 MB + pi-agent）
- 需本机 oMLX / 已配置 LLM 才能对话

## 构建

```bash
npm run pi-agent:install
npm run pi-node:install
npx tauri build
```
