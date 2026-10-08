# Monolith — 导入目录合一

> 状态：已批准 / 已实现  
> 日期：2026-10-06  
> 取代：`2026-10-06-convert-default-output-dir-design.md` 中独立「默认输出目录」；`2026-10-06-import-resource-browser-design.md` 双 Tab。

## 决定

网页剪藏与文件转 Markdown **共用一个导入目录**（`inboxDir`，UI：「导入目录」，默认 `~/Downloads/MonolithInbox`）。

- 转换保存对话框预填 `{inboxDir}/{stem}.md`，仍可改路径。
- 导入资源浏览器单列表，已/未资产化逻辑不变。
- 删除 `defaultConvertOutputDir` 与 `clipper_list_convert_output`。
- 默认打开 / 默认另存仍独立（对话框起始位置）。
