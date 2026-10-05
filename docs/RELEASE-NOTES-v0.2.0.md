# Monolith v0.2.0 — 发布说明

日期：2026-10-05

上一版（v0.1）已能打开、编辑本机文本文件。本版核心升级是：**在本机建立文档资产库**，把散落的 Word / Excel / PDF / PPT、网页与 Markdown 统一编目、可版本管理，并让 AI Agent 能通过 MCP **准确找到并阅读**这些文档——文件仍留在原位置，不搬家。

## 下载

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | [`Monolith_0.2.0_aarch64.dmg`](https://github.com/reallyMu/monolith/releases/download/v0.2.0/Monolith_0.2.0_aarch64.dmg) |
| Windows (x64) | [`Monolith_0.2.0_x64-setup.exe`](https://github.com/reallyMu/monolith/releases/download/v0.2.0/Monolith_0.2.0_x64-setup.exe) |

发布页：https://github.com/reallyMu/monolith/releases/tag/v0.2.0

macOS 为 ad-hoc 签名（未公证）。若首次打开被拦截：系统设置 → 隐私与安全性 → 仍要打开。Windows 需已安装 [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)（Win10/11 一般已自带）。

---

## 本版解决什么问题

### 0. 本机文档资产库，并与原文档保持同步

在 Monolith 中登记文档后，会形成**本机文档资产库**（浏览树 + 台账）。  
资产指向磁盘上的原文件路径：**不复制、不迁移**文件；编辑保存仍写回原路径。索引失效或源文件变更时可感知并处理，使资产目录与真实文档保持一致。

### 1. 不仅读文本：Word / Excel / PDF / PPT → Markdown

v0.1 主要面向文本与 Markdown。  
v0.2 可将 **doc/docx、xls/xlsx、pdf、ppt/pptx** 等转为 Markdown，再纳入资产库统一阅读与管理（依赖内置转换引擎；无文字层的扫描件会明确失败，而非 silently 糊弄）。

### 2. 不搬家：全局文档索引 + 版本管理

- **全局索引**：跨文件夹登记文档，在左侧资产树中浏览、搜索（按名称）、整理。  
- **版本管理**：日常保存覆盖当前文件；「保存新版本」在旁生成带时间戳的新文件，并在资产中留下版本历史。  
- **删除资产只删登记**，不会删除磁盘上的原文件。

### 3. Chrome 插件：网页一键存为 Markdown

配套 **Monolith Web Clipper**（Chrome 加载已解压扩展）。  
网页剪藏为 Markdown 落入 Inbox，在应用内打开并登记为资产。安装入口在应用 **设置**。

### 4. MCP：让 AI Agent 读得准、找得到

提供 **`monolith-mcp`** 服务，与应用共用同一套资产库。  
Agent 可通过 MCP 列出树、按名称搜索、读取正文、登记与修订——**解决「Agent 不知道你本机文档在哪、读不准」的问题**。  
设置中可发现本机 Agent、一键安装 MCP，并编辑 / 重载 Skill 说明。

---

## 其它体验改进

- 精密仪器风格界面（冰蓝强调、工具条收束）
- 资产树名称搜索；⌘K 聚焦搜索；⌘E 在「编辑 / 预览 / 对照」间循环
- 设置内快捷键一览

## 已知限制

- 资产「全文检索」（搜正文）尚未实现；当前搜索按名称 / 文件名
- 转换依赖各平台 downmark 二进制；扫描版 PDF/图片无 OCR
- macOS 安装包未公证

## 相关设计文档（仓库内）

- `docs/superpowers/specs/2026-10-04-local-document-asset-design.md`
- `docs/superpowers/specs/2026-10-04-nontext-md-conversion-design.md`
- `docs/superpowers/specs/2026-10-05-monolith-web-clipper-design.md`
- `docs/superpowers/specs/2026-10-05-monolith-mcp-assets-design.md`
- `docs/superpowers/specs/2026-10-05-monolith-precision-ui-design.md`
