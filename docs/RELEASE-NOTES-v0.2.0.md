# Monolith v0.2.0 — 发布说明

日期：2026-10-05

本版在 v0.1.0 本地编辑器基础上，补齐**本机文档资产**、**剪藏 / 转换**、**MCP 资产工具**，以及精密仪器风格 UI 与快捷键。

## 下载

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | `Monolith_0.2.0_aarch64.dmg` |
| Windows (x64) | `Monolith_0.2.0_x64-setup.exe`（NSIS，GitHub Actions 构建） |

发布页：https://github.com/reallyMu/monolith/releases/tag/v0.2.0

macOS 为 ad-hoc 签名（未公证）。首次打开若被拦截：系统设置 → 隐私与安全性 → 仍要打开。

## 新增功能

### 本机文档资产

- SQLite 台账（`assets.sqlite`）：浏览夹 + 路径索引；**文件不搬家**
- 生成资产 / 新版本 / 删除登记（不删磁盘文件）
- 左树可折叠、可拖拽整理；可选转换溯源 `source_path`

### Web 剪藏与 Inbox

- Monolith Web Clipper（基于 Obsidian Web Clipper 换皮）
- 剪藏落入 Inbox；应用内浏览、打开、登记为资产
- 扩展安装入口在**设置**（不再占工具条）

### 非文本 → Markdown 转换

- 内置 downmark 转换入口与转换日志
- 可登记为资产并保留来源路径

### MCP 资产读写

- 独立 `monolith-mcp`（stdio），与 App 共用 Domain / 数据库
- 工具含树浏览、登记、读写、修订、整理、清除登记，以及 `asset_search`
- 设置内可发现 Agent、安装 MCP、编辑 / 重载 Skill

### 精密仪器 UI

- 冰蓝 token、工具条收束（文件条全宽；格式条在编辑区上方）
- 资产树名称搜索（全文模式明确「尚未实现」）
- 收起态恢复：左上角竖排「资产」+ 展开箭头

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| Cmd+K | 聚焦资产树搜索 |
| Cmd+E | 编辑 → 预览 → 渲染对照（循环） |
| Cmd+B / Cmd+I / Cmd+D / Cmd+L | 加粗 / 斜体 / 删除线 / 链接 |
| Shift+Cmd+K / Shift+Cmd+T | 代码块 / 表格 |
| Cmd+U | 未支持（设置一览中标注） |

设置 →「快捷键」可查看完整一览。

## 设计与计划文档（本仓库）

- `docs/superpowers/specs/2026-10-05-monolith-precision-ui-design.md`
- `docs/superpowers/specs/2026-10-05-monolith-mcp-assets-design.md`
- `docs/superpowers/specs/2026-10-05-monolith-web-clipper-design.md`
- `docs/superpowers/specs/2026-10-04-nontext-md-conversion-design.md`
- `docs/superpowers/specs/2026-10-04-local-document-asset-design.md`

## 已知限制

- 资产「全文检索」尚未实现（UI / MCP 均明确返回未实现，不会静默当名称搜索）
- macOS 安装包未公证；Windows 需 WebView2
- 非文本转换依赖平台对应的 downmark 二进制（构建时由 `scripts/install-converters.sh` 拉取）

## 开发者速查

```bash
npm install
npm run tauri dev          # 开发
npm run tauri build        # macOS .app + .dmg
npm run test:asset-search  # 名称匹配契约自检
```

Windows NSIS：推送 `v*` 标签或手动跑 `.github/workflows/build-windows.yml`。
