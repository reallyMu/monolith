# Monolith v0.2.2 — 发布说明

日期：2026-10-08

在 **v0.2.1** 之上的功能与体验更新：Markdown 公式/图表预览、工具栏公式编辑器、原生菜单随系统语言、红 ❌ 隐藏到 Dock，以及导入/转换/剪藏相关加固。

## 下载

| 平台 | 安装包 |
|------|--------|
| macOS (Apple Silicon) | [`Monolith_0.2.2_aarch64.dmg`](https://github.com/reallyMu/monolith/releases/download/v0.2.2/Monolith_0.2.2_aarch64.dmg) |
| Windows (x64) | [`Monolith_0.2.2_x64-setup.exe`](https://github.com/reallyMu/monolith/releases/download/v0.2.2/Monolith_0.2.2_x64-setup.exe) |

发布页：https://github.com/reallyMu/monolith/releases/tag/v0.2.2

---

## 本版亮点

### Markdown：公式 + Mermaid 预览

- 右侧 TipTap 渲染 `$…$` / `$$…$$`（KaTeX）与 ` ```mermaid ` 图
- 源码仍在 Monaco；往返不丢 TeX / 围栏源码
- Mermaid：工具栏 / ⌘+滚轮缩放，视口滚动平移

### 工具栏公式编辑器

- Σ 打开面板：模板条 · LaTeX · KaTeX 实时预览 · 块级/行内切换
- 光标在公式内 → 编辑并替换；预览双击公式同路径回填
- 切换块级/行内时预览区固定高度，避免对话框抖动

### 原生菜单随系统语言

- 菜单与界面文案跟随 macOS / 系统 locale（中/英）

### 红 ❌ = 隐藏到 Dock

- 关闭窗口不退出进程；Dock / 再打开可恢复
- 全屏关闭时先退出全屏再 hide，减轻黑屏 Space 问题

### 导入 / 转换 / 剪藏

- 文件夹导入、转换默认输出目录、Word altChunk 等路径加固
- 剪藏 inbox 命名与资源侧同步更新

---

## 校验（发布前）

- `vue-tsc --noEmit`
- `npm run test:md-math-range` / `test:md-math-mermaid` / `test:asset-search`
- `cargo check`
