# Monolith — 转换默认输出目录

> 状态：已取代（导入目录合一，见 `2026-10-06-import-dir-unify-design.md`）  
> 日期：2026-10-06  
> 相关：`2026-10-04-nontext-md-conversion-design.md`、系统设置（`settings.json`）

## 1. 目标

文件转 Markdown 时，默认落盘路径可指向**独立输出目录**，避免在源文件所在目录新增 `.md`、打乱源目录结构与文件数量。

## 2. 冻结决定

1. **新增设置** `defaultConvertOutputDir`（UI：「默认输出目录」），与「默认打开目录」「默认另存目录」并列；**可空**。
2. **交互选 A**：始终弹出「选择 Markdown 保存位置」；仅改变对话框的 `defaultPath` 预填，用户可改到别处。
3. **默认路径规则**：
   - `defaultConvertOutputDir` 非空 → `{defaultConvertOutputDir}/{stem}.md`
   - 为空 → 维持现状：`{源文件父目录}/{stem}.md`
4. **不复用**「默认另存目录」：另存 = 编辑中文件；输出目录 = 转换落盘，语义分离。
5. **不跳过**保存对话框；不做静默直写。

## 3. 设置面

| 字段（camelCase） | UI 文案（zh） | 作用 |
|---|---|---|
| `inboxDir` | 剪藏 Inbox | 剪藏落盘（不变） |
| `defaultOpenDir` | 默认打开目录 | 打开 / 转换**选源**对话框起始（不变） |
| `defaultSaveDir` | 默认另存目录 | 另存为对话框起始（不变） |
| `defaultConvertOutputDir` | 默认输出目录 | 转换 MD 保存对话框预填目录（新增） |

- 选目录：沿用现有 `pickSettingsDir`。
- 持久化：`~/Library/Application Support/com.muqiang.monolith/settings.json`。
- i18n：中/英 hint 说明「转换 Markdown 保存对话框的默认目录（可空）；空则与源文件同目录」。

## 4. 后端

- `AppSettings` 增加 `default_convert_output_dir: String`（serde camelCase → `defaultConvertOutputDir`）；`Default` 为空串。
- `convert_default_output` 命令内部 `load_settings()`，再算路径：
  - 若 `default_convert_output_dir` trim 非空 → `Path::new(dir).join(format!("{stem}.md"))`；
  - 否则保持现有同目录逻辑（`default_output_path` 现有行为）。
- 可把「带设置的路径解析」提成纯函数便于单测：有设置 → 输出在设置目录；无设置 → 仍为源同目录。
- 目录不存在：算路径阶段不创建；真正写入时由现有 convert 写文件逻辑创建父目录。
- 与 `2026-10-04-nontext-md-conversion-design.md` §1.1.3「默认源同目录」关系：该句在**未设置**本字段时仍成立；已设置时默认改为输出目录（对话框预填）。

## 5. 前端

- `AppSettings` / `settingsDraft` 增加字段；设置面板在「默认另存目录」旁增加一行「默认输出目录」。
- `beginConvert`：仍 `convertDefaultOutput(inputPath)` → `save({ defaultPath })`；无需前端拼接路径（权威在 Rust）。

## 6. 范围外

- 跳过保存对话框 / 静默直写
- 批量转换策略、重名自动编号策略变更
- 自动登记资产、改 Inbox / 打开 / 另存语义
- Windows 路径特例（跟随现有 Path / dialog 行为）

## 7. 验收

1. 未设置「默认输出目录」：转换预填仍为源文件旁 `{stem}.md`。
2. 已设置：预填为该目录下 `{stem}.md`；取消对话框则不写文件。
3. 在对话框改到其它路径后，实际写入用户所选路径。
4. 设置可清空并保存；清空后行为回到验收 1。
