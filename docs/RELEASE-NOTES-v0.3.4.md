# Monolith v0.3.4 — 发布说明

日期：2026-10-21

在 **v0.3.1** 之上：内置 Node 的 Pi Agent 开箱即用、Bridge 端口可配、⌘F 正文查找修复；**不打包 OCR / JRE / Python**，安装包显著变瘦。

## 下载

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | [`Monolith_0.3.4_aarch64.dmg`](https://github.com/reallyMu/monolith/releases/download/v0.3.4/Monolith_0.3.4_aarch64.dmg) |
| Windows (x64) | [`Monolith_0.3.4_x64-setup.exe`](https://github.com/reallyMu/monolith/releases/download/v0.3.4/Monolith_0.3.4_x64-setup.exe) |

发布页：https://github.com/reallyMu/monolith/releases/tag/v0.3.4

---

## 本版亮点

### 体积（相对试装 OCR 的胖包）

- macOS `.app` 约 **337 MB**（pi-agent + 内置 Node + downmark）
- **不包含** RapidOCR / 内嵌 Python / Temurin JRE / OpenDataLoader PDF
- PDF / Office → Markdown 仍走 **downmark**；图片 OCR 本版不做

### Pi Agent

- **内置 Node v22.19**：正式版不再要求用户本机安装 Node
- Bridge 首选端口可在设置里改；占用时自动顺延并写回
- CORS 修复 WebView「Load failed」
- `pi-agent:install`：已有 Bridge 时可跳过重新 vendor（`PI_FORCE=1` 强制刷新）

### 编辑器

- **⌘F 正文查找**：预览模式下先切回编辑/分栏再开 Monaco 查找；查找框聚焦时不再被 ⌘B/⌘K 抢走

### Windows

- CI 打 NSIS：`pi-node:install`（win-x64）+ downmark + `tauri build --bundles nsis`
- 不再拉取 JRE / Python / OCR

---

## 构建

```bash
npm run pi-node:install
npm run pi-agent:install
npx tauri build
```

Windows：推送 `v*` tag 触发 `.github/workflows/build-windows.yml`。
