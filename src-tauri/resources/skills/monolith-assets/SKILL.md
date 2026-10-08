---
name: monolith-assets
description: >-
  通过 monolith MCP 管理 Monolith 本地文档库（登记、读写正文、修订、清除索引）。
  库的称呼等价：资产库、知识库、ML库、monolith库、Monolith、ML。
  用户提到以上任一称呼，或 MonolithInbox 剪藏，或要列出/登记/更新/删除本地 Markdown 资产时使用。
  夹优先：不提夹=全局检索；指定夹=定向查读改；登记未指夹→root。
  Manage the Monolith local document library via the monolith MCP server.
  Equivalent names: 资产库, 知识库, ML库, monolith库, Monolith, ML.
  Folder-first: no folder = global; with folder = scoped; register without folder → root.
---

# Monolith 资产 / Monolith Assets (MCP)

用户说的 **资产库 / 知识库 / ML库 / monolith库 / Monolith / ML** 均指同一库。  
**资产库 / 知识库 / ML库 / monolith库 / Monolith / ML** all mean this same library.

调用 **`monolith`** MCP server 的 tools。禁止自造 SQL，禁止自行删除磁盘文件。  
Call tools from the **`monolith`** MCP server. Do not invent SQL or delete disk files yourself.

## 夹语义 / Folder semantics（重要）

| 用户说法 | 行为 |
|---------|------|
| **不提夹** | **全局**检索 / 列表 / 按名定位后再读、改 |
| **指定夹**（名或 `root/子夹` 路径） | **定向**：只在该夹内查 / 读 / 写 / 改（`recursive` 默认 false；找文件名时可 true） |
| **登记且未指定夹** | 挂到 **root** |
| **改已有正文** | 仍在该资产当前夹，**不挪夹** |

夹名歧义 → 用路径（如 `root/临床/指南`）或 `browse_term_id`；仍歧义则列出候选问用户。  
Ambiguous folder/asset names → use path or id; else list candidates and ask.

全局列表默认只返回名单，**不要**把全库正文读进来。  
Global list = catalog only; do not bulk-read every body.

## 铁律 / Iron rules

| 规则 Rule | 含义 Meaning |
|------|---------|
| L1 / L7 | `asset_delete` **只删索引**。磁盘文件与 `conversion_log` 保留。 |
| L5 | `asset_write_content` = 覆盖当前文件（Ctrl+S），**不**新增版本行。 |
| L6 | 需要历史 → App「保存为新版本」。 |
| L2 | 仅 Monaco 可读文本可登记。 |
| 并发 | 写前带上 `asset_read_content` 的 `mtime` 作为 `expected_mtime`。 |

## 工具 / Tools

| Tool | 何时用 When |
|------|------|
| `asset_list_tree` | 全局总览：夹树 `terms` + 全部 `assets` |
| `asset_list_folder` | **主入口**：不传 `folder`=全局；传 `folder`/`browse_term_id`=该夹；`recursive` 默认 false |
| `asset_get` | `asset_id` / `path` / `name`（+可选 `folder`） |
| `asset_read_content` | 读正文；记下 `mtime` |
| `asset_register` | 登记盘上文件；可选 `folder`，省略→**root** |
| `asset_write_content` | 更新正文（不挪夹） |
| `asset_delete` | **仅取消登记** — 告知用户文件仍在 |

## 典型流程 / Typical flows

### 读某夹文档 / Read a folder

1. `asset_list_folder` `{ "folder": "临床" }` 或 `{ "folder": "root/临床" }`
2. 对需要的项 `asset_read_content`（`asset_id` 或 `name` + `folder`）
3. 不要一次读完全夹所有长文，除非用户明确要求

### 全局查找 / Global search

1. `asset_list_folder`（不传 folder）或 `asset_list_tree`
2. 按 `display_name` / 路径缩小后再读

### 登记 / Register

1. 用户指定夹 → `asset_register` 带 `folder`
2. 未指定夹 → `asset_register` **不传 folder**（进 root）
3. Inbox：`path` = `~/Downloads/MonolithInbox/….md`，可选 `source_path` URL

### 安全编辑 / Edit safely

1. `asset_read_content` → 保留 `mtime`
2. 修改文本
3. `asset_write_content` + `expected_mtime`
4. `MTIME_CONFLICT` → 重读合并，禁止盲写

### 清除登记 / Clear registration

1. 确认只要去索引、不删文件
2. `asset_delete`（`asset_id`）
3. 说明磁盘文件仍在

## 禁止 / Do not

- 把 `asset_delete` 当成 `rm`
- 经 MCP 写二进制/非文本
- 用 shell `sqlite3` 直连 `assets.sqlite`
- 未指定夹时对全库正文做批量盲读
