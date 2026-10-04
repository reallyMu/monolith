# 本机文档资产管理（Monolith）— 设计

> 日期：2026-10-04 · 状态：**已批准 / 开工**  
> 参考：Healix CDH `asset_browse_term` / `asset_instance` / `asset_version`（**不上** `asset_relationship`；溯源用实例上可选 `source_path`）；资产树交互参考 `AttributeAssetTree`（可拖夹、可拖资产）  
> 栈：Tauri 2 + SQLite（嵌入）+ Vue 3 + Monaco

---

## 0. 一句话

**物理文件与摘要都不搬家；SQLite 只做逻辑台账与路径索引；仅 Monaco 可读文本可入资产；人工「生成资产」后进浏览树 root，再拖拽整理；删除只删登记不删磁盘文件。**

---

## 1. 目标与非目标

### 1.1 目标

1. 解决本机文档分散、难整理、难查找：逻辑分类 + 路径索引，不强制整理文件系统。
2. 左树右编：浏览夹 / 资产树（可拖拽夹与资产）+ Monaco 打开当前版本。
3. 人工「生成资产」入库；人工「保存为新版本」才产生新副本路径。
4. 可选溯源：`asset_instance.source_path`（转换前来源绝对路径；可空）。**无 relationship 表。**
5. 文件缺失：提示并保留登记，可重定位或删登记。

### 1.2 非目标

- Obsidian 式迁库 / 指定摘要目录。
- 非文本直接作为资产（须先转成 Monaco 可读文本）。
- Ctrl+S 自动留历史。
- 插件协议完整生态（本期可后置；转换结果路径由人/插件决定）。

---

## 2. 铁律

| # | 铁律 |
|---|------|
| L1 | **不搬文件**：台账只记绝对路径。 |
| L2 | **准入** = `file-types.json` 中 Monaco 可读扩展名（或特殊基名）；`file_type` 用真实扩展名，不发明 kind 名。 |
| L3 | **入库人工**：打开文本后点「生成资产」；默认挂 `browse_term` **root**。 |
| L4 | **路径唯一**：`asset_version.absolute_path` 全局 UNIQUE；重复生成 → 提示已存在并定位。 |
| L5 | **Ctrl+S** = 覆盖当前版本文件，不建 `asset_version`。 |
| L6 | **新版本人工**：同目录 `基名_YYYYMMDD_HHMMSS.ext`（冲突加 `_2`…），新版本行并切当前。 |
| L7 | **删除资产** = 只删 SQLite 登记（及版本行），**不删**物理文件。 |
| L8 | **实例扁平，树靠 term**：夹不是资产；UI 参考 CDH AttributeAssetTree 拖拽。 |
| L9 | **缺失保留**：打不开时提示缺失，登记仍在；可重定位或删登记。 |

---

## 3. 数据模型（SQLite）

库文件：应用数据目录下 `assets.sqlite`（如 macOS `~/Library/Application Support/com.muqiang.monolith/assets.sqlite`）。

### 3.1 `asset_browse_term`

| 列 | 说明 |
|----|------|
| `id` | PK |
| `parent_id` | 父夹；root 的 parent 为 NULL |
| `code` | 稳定码；`root` 预置 |
| `display_name` | 可改展示名 |
| `sort_order` | 同级排序 |
| `status` | `ACTIVE` / `ARCHIVED` |
| `created_at` / `updated_at` | ISO-8601 |

- 种子：插入 `code='root'` 的根夹。  
- 新建资产：`browse_term_id = root.id`。  
- 软上限深度可对齐 CDH（建议 ≤5）；拖拽时禁止成环。

### 3.2 `asset_instance`

| 列 | 说明 |
|----|------|
| `id` | PK |
| `display_name` | 展示名（默认文件名） |
| `file_type` | 扩展名，如 `md` / `json` |
| `browse_term_id` | → term |
| `current_version_id` | → 当前 `asset_version` |
| `created_at` | 「生成资产」时间 |
| `source_path` | 可选；转换前来源绝对路径（溯源） |

### 3.3 `asset_version`

| 列 | 说明 |
|----|------|
| `id` | PK |
| `asset_id` | → instance |
| `absolute_path` | **UNIQUE**，含文件名的绝对路径 |
| `created_at` | 该版本登记时间 |

**不上 `asset_relationship`。** 溯源只靠 `source_path`。

---

## 4. 关键行为

### 4.1 生成资产

前置：当前 tab 有路径且 `isSupportedPath`。  
若 `absolute_path` 已在 `asset_version` → 报错/提示并选中该资产。  
否则：插 `instance`（browse=root，可选 `source_path`）+ 首条 `version`。

### 4.2 保存

- **普通保存 / Ctrl+S**：`write` 当前版本路径；不插版本。  
- **保存为新版本**：写同目录新文件名 → 插 version → 更新 `current_version_id`；tab 路径切到新文件。

命名：`{stem}_{YYYYMMDD}_{HHMMSS}{ext}`；若存在则 `{stem}_{YYYYMMDD}_{HHMMSS}_2{ext}`…

### 4.3 树拖拽（对齐 CDH AttributeAssetTree）

- 资产拖到夹 → 更新 `browse_term_id`。  
- 夹拖到夹 → 更新 `parent_id`（校验非自身/非子孙、深度）。  
- 删除夹：仅空夹（无子夹、无挂载资产）。  
- 删除资产：级联删 versions/relationships；**不** `fs::remove_file`。

### 4.4 打开资产

解析 `current_version.absolute_path`；若文件不存在 → 状态栏/对话框提示缺失，保留树节点；提供「重新定位」（选新路径，UNIQUE 校验后更新 version 路径）。

---

## 5. UI

- 可收起左侧栏：term 树 + 资产叶（展示名；可显示 file_type）。  
- 工具栏：「生成资产」「保存为新版本」；删除在树上下文菜单（文案注明仅移除登记）。  
- 编辑区仍为现有 Monaco / MD 预览。

---

## 6. 与插件转换的边界

本期：转换可由外部工具完成；产物路径打开后「生成资产」，若知来源则填 `to_external_path`。  
下期：开放插件协议（stdio/JSON-RPC）注册转换器，主程序仍不解析非文本。

---

## 7. DoD（v1）

1. SQLite 初始化含 root；生成资产 / 重复路径提示。  
2. 左树：建夹、改名、删空夹、拖资产、拖夹。  
3. 删资产不删文件。  
4. Ctrl+S 覆盖；保存为新版本出时间戳副本并切当前。  
5. 缺失文件提示 + 可重定位。  
6. 可选写入 `source_path`（无 relationship 表）。
