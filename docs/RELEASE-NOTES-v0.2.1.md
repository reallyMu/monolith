# Monolith v0.2.1 — 发布说明

日期：2026-10-05

在 **v0.2.0**（本机文档资产库 / 转换 / 剪藏 / MCP）之上的补丁发布，重点修复 Agent 侧 MCP 连接。

## 下载

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | [`Monolith_0.2.1_aarch64.dmg`](https://github.com/reallyMu/monolith/releases/download/v0.2.1/Monolith_0.2.1_aarch64.dmg) |
| Windows (x64) | [`Monolith_0.2.1_x64-setup.exe`](https://github.com/reallyMu/monolith/releases/download/v0.2.1/Monolith_0.2.1_x64-setup.exe) |

发布页：https://github.com/reallyMu/monolith/releases/tag/v0.2.1

完整功能说明见 [v0.2.0 发布说明](./RELEASE-NOTES-v0.2.0.md)。

---

## 本版修复

### MCP：Cursor 等宿主 discovery 失败

部分 Agent（尤其 Cursor）在 `mcp.json` 的 `command` 指向  
`…/Application Support/…/monolith-mcp`（路径含空格）时，会出现 **live tool discovery failed**，无法调用资产工具。

**修复**：设置 → 安装 MCP 时，除释放 App Support 二进制外，再创建无空格软链：

`~/.local/bin/monolith-mcp` → App Support 中的真实二进制  

并把它写入各 Agent 的 MCP 配置。

升级后请在应用内重新「安装 MCP」，或确认 `~/.cursor/mcp.json` 已指向上述软链，然后在 Cursor 中 **Reload MCP**。

---

## 相对 v0.2.0 未改动的能力

- 本机文档资产库（不搬家、与原路径同步）
- Word / Excel / PDF / PPT → Markdown
- 全局索引与版本管理
- Chrome 网页剪藏为 MD
- MCP 读写资产，供 AI Agent 准确找文档
