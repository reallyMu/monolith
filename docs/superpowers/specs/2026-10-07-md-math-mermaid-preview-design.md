# Monolith — MD 预览：公式 + Mermaid

> 状态：已批准（用户确认右侧渲染用 TipTap）  
> 日期：2026-10-07

## 决定

1. **MD 源码编辑** = Monaco（左侧 / 仅编辑）；**MD 渲染** = TipTap（右侧 / 仅预览 / 对照右半）。不在 Monaco 内画公式或图。
2. **公式**：行内 `$…$`、块级 `$$…$$` → KaTeX 渲染；源码往返仍为 TeX 定界符，不落成 HTML。
3. **Mermaid**：` ```mermaid ` 围栏 → Mermaid SVG；失败时显示错误文案，源码保留。
4. **编辑**：改公式/图源优先在 Monaco。公式可视化结构编辑器仍不做；工具栏公式面板见 [2026-10-08-md-formula-editor-design.md](./2026-10-08-md-formula-editor-design.md)。
5. **对照同步**：math / mermaid 各占一个顶层块，纳入现有 `sourceSpansFromMarkdown` 块序。
6. **Mermaid 交互（收束）**：缩放用工具栏 / ⌘+滚轮；平移用视口原生滚动。不与 TipTap 抢 pointer / 不挂 `window` 级拖动手势。

## 非目标

- Monaco 内嵌渲染插件
- 设置项切换渲染引擎
- 全量 MathJax / 任意 diagram 语言
- Mermaid 自定义抓取拖动（与编辑器事件冲突，已放弃）

## 验收

1. 预览可见高斯积分等 `$$` 公式。
2. 预览可见 `graph TD` mermaid 图；可缩放、可滚动查看。
3. 保存/往返后源码仍为 `$$` / ` ```mermaid `，不丢内容。
