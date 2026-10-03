# Monolith 需求整理（验收基线）

小工具：本地 Markdown / 文本编辑器（Tauri + Monaco）。

## 功能

1. **文件**：新建 / 打开 / 保存 / 另存为 / 最近文件；扩展名走 Monaco 语言映射表。
2. **视图**：编辑 / 预览 / 对照；背景色左右可调，控件在工具栏右侧。
3. **Markdown 编辑**
   - 左侧：Monaco 源码。
   - **对照 / 预览右侧**：TipTap 所见即所得，可直接改并写回源码。
   - **对照同步**：源码行 ↔ TipTap 顶层块（列表整块，非逐项），滚动/高亮/点击互跳。
4. **Markdown 工具**：标题、加粗/斜体等、有序/无序/任务列表（有序自动续号）、表格、符号；对照时按焦点落在源码或 TipTap。
5. **SQL**：`.sql` 支持格式化（`sql-formatter`，PostgreSQL 方言）。
6. **界面语言**：跟随系统（`zh*` → 中文，否则英文）。

## 对照同步（硬性）

- 禁止用「全文行进度百分比」对齐。
- 映射权威：`sourceSpansFromMarkdown`（与 TipTap 顶层块同序，`src/utils/mdSourceMap.ts`）。
- 滚动时：当前驱动侧带动另一侧；程序化滚动不得回灌。
- 光标/点击：两侧高亮同一逻辑块，视口 Y 误差 ≤ 6px。
- 找不到对应块 = 不对齐，不发明假映射。

## 工程约束

- 小工具：少抽象、无只读预览与 WYSIWYG 双轨。

## 自测（交付前）

```bash
node --experimental-strip-types /Users/muqiang/.openclaw/workspace/monolith/scripts/acceptance.mjs
node --experimental-strip-types /Users/muqiang/.openclaw/workspace/monolith/scripts/test-md-sync.mjs
npx vue-tsc --noEmit
```

### 实跑验收（对照 + 便捷）

| # | 项 | 判据 |
|---|---|---|
| A | 右侧可编辑 | split/preview 均为 TipTap `contenteditable=true` |
| B | 右改→左 | `insertText` 写回 `tab.content` |
| C | 左→右高亮 | Decoration `.is-active` 落在对应块 |
| D | 视口 Y | 对齐误差 ≤ 6px |
| E | 右点/右滚→左 | 行号与锚点一致 |
| F | OL 续号 / B I 表 引用 任务 SQL | 工具栏与 API 行为正确 |
| G | 视图切换 | 编辑/预览/对照 UI 按钮可用 |
| H | i18n | 系统中文 →「预览」「编辑」 |
