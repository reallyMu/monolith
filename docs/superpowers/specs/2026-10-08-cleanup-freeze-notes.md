# 冻结收束纪要（2026-10-08）

> 状态：已落地 — `vue-tsc` / `cargo check` 过；已 `tauri build` 并装到 `/Applications/Monolith.app`

## 已做

1. **Mermaid**：删 `window` 拖动、`handleDOMEvents` 拦截、`isMermaidBlockEvent`；改为 SVG 宽高缩放 + 视口 `overflow: auto` 滚动平移；`stopEvent: true` 仅挡 ProseMirror。
2. **红 ❌**：只保留 `on_window_event` → `prevent_close` + 下一拍 `hide`；`RunEvent` 不再重复 `prevent_close`；`ExitRequested` 仅 `prevent_exit`，不再 `bring_to_front`。

## 建议后续提交切割（尚未执行）

| 主题 | 大致路径 |
|---|---|
| menu-i18n | `src-tauri/src/i18n.rs`, `lib.rs` menu, `src/i18n` |
| md-math-mermaid | `src/extensions/*`, `MarkdownWysiwyg.vue`, `mdSourceMap.ts`, katex/mermaid deps |
| close-hide | `lib.rs` window/run events |
| toolbar | `App.vue` format toolbar |
| import/convert/clipper | `assets.rs`, `convert.rs`, `folder_import.rs`, clipper… |

## 仍冻结

- 新功能、新依赖（除修 bug 必需）
- 再叠一层 Mermaid 自定义拖动
