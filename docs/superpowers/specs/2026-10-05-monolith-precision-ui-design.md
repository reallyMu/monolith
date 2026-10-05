# Monolith 精密仪器 UI + 资产查找 + 快捷键 + MCP search

> 日期：2026-10-05  
> 状态：已定稿（待实现计划）  
> 范围：视觉语言、工具条收束、编辑器同调、资产树名称搜索（全文占位）、快捷键绑定与一览、MCP `asset_search`  
> 前置：[`2026-10-05-monolith-mcp-assets-design.md`](./2026-10-05-monolith-mcp-assets-design.md)

## 1. 目标

把 Monolith 从「默认深色工具壳」提升为 **精密仪器** 气质：冷、准、少装饰；同时补齐资产树查找（v1 名称过滤）、MCP `asset_search`（与 UI 同语义）、与常用快捷键可发现性。全文检索只留 UI/MCP 模式钩子，本轮不实现索引。

## 2. 决策摘要

| 项 | 选择 |
|----|------|
| 气质 | A — 精密仪器（冰蓝强调，非终端荧光 / 非柔和笔记风） |
| 落地方式 | 方案 2 — Token + 工具条收束 + 编辑器同调 + 树搜索（不重做三栏工作台） |
| 资产查找（UI） | C — 名称过滤可用；「全文」模式占位 |
| 资产查找（MCP） | C — `asset_search`：`mode=name` 可用；`mode=fulltext` 明确未实现 |
| 快捷键 | C — 补齐绑定 + 设置内一览 |
| ⌘U 下划线 | 一览标注「未支持」（Markdown 无标准下划线，避免污染源码） |
| ⌘K 显示目录 | 聚焦资产树搜索框 |
| ⌘E 切换源码模式 | `edit` ↔ `preview` 切换（对照模式不强制退出） |

## 3. 视觉语言

### 3.1 Token（`src/styles.css` `:root`）

| 角色 | 含义 |
|------|------|
| `--bg-0` / `--bg-1` / `--bg-2` | 窗底 / 面板 / 抬起面（工具条、输入） |
| `--hairline` | 低对比细边框 |
| `--text-1` / `--text-2` / `--text-3` | 正文 / 次要 / 元信息 |
| `--accent` | 冰蓝：焦点、选中、主按钮、树高亮 |
| `--danger` / `--ok` | 仅状态徽章 |
| 圆角 | 4–6px |
| 字体 | UI 系统字体；路径 / 快捷键 / 元数据等宽 |

### 3.2 刻意不做

大渐变、玻璃模糊、强发光阴影、大胶囊药丸堆、喧宾夺主的插画空状态。

### 3.3 编辑器同调

Monaco 与预览区背景对齐 `--bg-0` / `--bg-1`；选区 / 行高亮用同一冰蓝低透明度。不引入第三方 Monaco 主题包。

## 4. 布局与工具条

### 4.1 骨架（保持）

左资产树（可折叠）+ 中标签与编辑/预览/对照。不改为固定三栏工作台。

### 4.2 文件条（始终可见，单行）

分组：`文件`（新建 / 打开 / Inbox / 转换 / 保存）｜`资产`（登记 / 新版本）｜`视图`（编辑 / 预览 / 对照 / 源）｜`系统`（设置）。

- 编辑区/预览区背景色选择器收入设置或视图次级，不占主条。
- 图标间距加大；组分隔用 hairline。

### 4.3 格式条（仅 MD / 富文本相关时显示）

- 默认一行：标题、字号、B/I、链接、代码。
- 「更多格式」展开或溢出：删除线、表格、任务列表等。
- 按钮 `title` 带 ⌘ 提示，与快捷键表一致。

### 4.4 密度

- 工具条高度统一（约 36px），消除多行图标墙。
- 窄窗：格式条优先折叠；文件条以溢出菜单保单行。

### 4.5 设置窗

保持侧栏分页结构；换用 token 配色与间距。新增「快捷键」分页（见 §6）。

## 5. 资产树查找

### 5.1 UI

资产树标题下方：搜索框 + 模式切换（segmented 或下拉）。

| 模式 | v1 行为 |
|------|---------|
| 名称（默认） | 对 `displayName` 与 path basename 做不区分大小写包含匹配；命中资产显示，祖先夹展开；无匹配空态 |
| 全文 | 可选中；输入/回车提示「全文检索尚未实现」；不调用假后端 |

### 5.2 交互

- 输入 debounce ~150ms。
- Esc 清空并恢复全树。
- 点击结果走现有打开/选中。
- ⌘K 聚焦该搜索框。

### 5.3 实现边界

- v1 纯前端过滤已加载的 `assetListTree`，不建 FTS。
- 预留 `searchMode: 'name' | 'fulltext'`；全文另开设计。

### 5.4 非目标（查找）

跨库语义搜索、拼音分词、正文倒排索引实现。

## 5b. MCP `asset_search`

与 UI 查找同语义，走 Domain（禁止 MCP 旁路拼 SQL）。

### 5b.1 Tool 契约（v1）

| 字段 | 类型 | 说明 |
|------|------|------|
| `query` | string | 必填；trim 后空则返回空列表或错误「query required」 |
| `mode` | `"name" \| "fulltext"` | 默认 `"name"` |
| `folder` | string? | 可选；有则仅在该夹（及实现时约定是否含子孙，**v1：默认不递归**，与 `asset_list_folder` 的 `recursive` 默认一致；需要子孙时传 `recursive: true`） |
| `browse_term_id` | number? | 可选；与 `folder` 二选一解析 |
| `recursive` | boolean? | 默认 `false`；仅当带 folder 作用域时有效 |

**`mode=name`**：匹配 `displayName` 与当前版本 path basename（不区分大小写包含）；返回资产摘要列表（id、displayName、path、browseTermId 等，与 `asset_list_folder` 条目同形或为其子集）。

**`mode=fulltext`**：不执行检索；返回结构化错误或结果信封标明 `implemented: false` / message「全文检索尚未实现」，**禁止**静默当名称搜索。

### 5b.2 Skill

更新 `monolith-assets` SKILL：说明何时用 `asset_search` vs `asset_list_folder`；全文未实现勿假装已搜正文。同步 App Support 可编辑副本需用户「同步到 Agent」。

### 5b.3 与 UI 一致

同一套匹配规则（可抽共享过滤函数：前端 TS 一份逻辑描述；Rust Domain 为权威实现，UI 可继续前端滤树或后续改调 Domain——**v1 允许 UI 前端滤、MCP 走 Domain 名称匹配**，规则文档对齐即可；若易漂移则 P1 末尾抽共享）。

## 6. 快捷键

### 6.1 平台

macOS 以 ⌘ 为准（实现用 `CmdOrCtrl`）；一览文案随系统 locale。

### 6.2 绑定

| 快捷键 | 功能 | 备注 |
|--------|------|------|
| ⌘S | 保存 | 已有 |
| ⌘O / ⌘N | 打开 / 新建 | 已有 |
| ⌘F | 查找 | 已有（编辑区） |
| ⌘Z / ⇧⌘Z | 撤销 / 重做 | 已有 |
| ⌘B / ⌘I | 加粗 / 斜体 | WYSIWYG 补齐；源码侧插入对应 MD 或等价操作 |
| ⌘D | 删除线 | WYSIWYG `toggleStrike`；仅 MD 上下文，避免与 Monaco「复制行」冲突 |
| ⌘L | 插入链接 | WYSIWYG `setLink` |
| ⇧⌘K | 代码块 | WYSIWYG `toggleCodeBlock` |
| ⇧⌘T | 插入表格 | 打开现有表格选择器 |
| ⌘E | 切换源码模式 | `edit` ↔ `preview` |
| ⌘K | 显示目录 | 聚焦资产树搜索 |
| ⌘U | 下划线 | **未支持**（一览标注）；本轮不加 underline mark |

### 6.3 一览面板

- 设置 →「快捷键」分页（可选格式条旁 `?` 深链到此页）。
- 只读表格；未支持项灰色 + 备注。
- v1 不提供改键。

### 6.4 冲突

- App 内捕获并 `preventDefault`（焦点在本窗且非重命名/纯文本输入冲突时）。
- 资产树 draft 输入框内不抢格式快捷键。

## 7. 分期

| 阶段 | 内容 |
|------|------|
| P0 | Token + 工具条/设置换皮；编辑器背景同调 |
| P1 | 资产树名称搜索 + 全文占位；⌘K；MCP `asset_search`（name + fulltext 占位）+ Skill 更新 |
| P2 | 快捷键补齐 + 设置一览；⌘E |
| 以后 | 全文检索（UI + MCP 同开设计） |

## 8. 验收

1. 主界面呈冰蓝精密仪器风，而非默认灰 IDE。
2. 文件条单行；格式条默认一行（更多可展开）。
3. 树顶可按名称过滤；「全文」模式有明确未实现提示。
4. MCP `tools/list` 含 `asset_search`；`mode=name` 可搜；`mode=fulltext` 明确未实现。
5. §6.2 已支持项可用；⌘U 在一览标未支持。
6. 设置 → 快捷键可查完整表。

## 9. 非目标（汇总）

- 全文检索实现与索引选型（UI/MCP 均仅占位）  
- 可自定义改键  
- 固定三栏工作台重排  
- 大型图标库 / 玻璃拟态 / 多主题商店  

## 10. 主要触达文件（实现时参考）

- `src/styles.css` — token  
- `src/App.vue` — 工具条、快捷键、编辑器背景  
- `src/components/AssetBrowser.vue` — 树搜索 UI 与过滤  
- `src/components/MarkdownWysiwyg.vue` — 格式动作  
- `src/i18n/messages.ts` — 文案  
- `src-tauri/src/lib.rs` — 菜单加速键与一览入口（若需要）  
- `src-tauri/src/mcp_server.rs` / Domain — `asset_search`  
- `skills/monolith-assets/SKILL.md` — Agent 用法  

