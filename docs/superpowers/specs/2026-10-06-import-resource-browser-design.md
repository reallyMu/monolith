# Monolith — 导入资源浏览器

> 状态：已取代（单列表 + 导入目录合一，见 `2026-10-06-import-dir-unify-design.md`）  
> 日期：2026-10-06  
> 相关：剪藏 Inbox 浏览、`defaultConvertOutputDir`

## 1. 目标

将「浏览 Clip Inbox」升级为 **导入资源浏览器**（EN: **Import Resource Browser**）：双 Tab 浏览剪藏 Inbox 与转换输出目录中的 Markdown，并标注已/未资产化。

## 2. 冻结决定

1. 入口改名：导入资源浏览器 / Import Resource Browser（工具栏、菜单、模态标题）。
2. 双 Tab：剪藏 Inbox / Clip Inbox；转换输出 / Convert Output。
3. 列表能力与现 Inbox 一致：仅 `.md`、已资产化徽章、点击打开。
4. 未设置 `defaultConvertOutputDir`：**仍显示**转换输出 Tab，空列表 + 提示去设置（选项 A）。
5. 不递归子目录；不自动登记；不回退默认转换路径。

## 3. 后端

- 抽取 `list_md_entries(dir, conn)`（登记判定复用 `asset_version.absolute_path`）。
- `clipper_list_inbox` 继续返回 `Vec`（内部走抽取函数）。
- 新增 `clipper_list_convert_output` → `{ configured, exists, dir, entries }`。

## 4. 前端

- 模态：标题 + 双 Tab + 当前目录 hint + 刷新 / 关闭 + 列表。
- 切换 Tab 时加载对应列表；刷新只刷当前 Tab。
