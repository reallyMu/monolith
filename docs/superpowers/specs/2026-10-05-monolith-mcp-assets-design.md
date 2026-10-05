# Monolith MCP（资产读写）— 设计

> 日期：2026-10-05 · 状态：**已批准 / 开工**  
> 前置：[`2026-10-04-local-document-asset-design.md`](./2026-10-04-local-document-asset-design.md)（L1–L9）  
> 形态：**独立二进制 `monolith-mcp`（stdio）**；可与 Monolith.app **并发**访问同一 `assets.sqlite`（WAL + 乐观写校验）。

---

## 0. 一句话

**Agent 通过 MCP stdio 调用与 UI 同源的资产 Domain，完成资产的读 / 登记写 / 正文覆盖 / 新版本修订 / 树整理 / 清除登记；不删不搬磁盘文件。App 可向多种 Agent 主机注册 MCP；并提供 Skill 教 Agent 如何正确调用。**

---

## 1. 目标与非目标

### 1.1 目标（一期）

1. 提供独立可执行文件 `monolith-mcp`，以 **MCP over stdio** 暴露资产工具。
2. 工具覆盖：**读取、写入（登记 + 正文）、修订（新版本 / 改名 / 移动 / 重定位）、清除（删登记）**。
3. 与 Monolith.app **共用** `~/Library/Application Support/com.muqiang.monolith/assets.sqlite`（macOS；其它平台用等价 app data 路径）。
4. 允许 UI 与 MCP **同时打开同一库**：SQLite **WAL**；正文写入支持 `expected_mtime` 乐观并发。
5. 全部铁律与 UI 一致（L1–L9）；MCP **不得**旁路 Domain 直接拼业务 SQL。
6. **Skill 必交付**：仓库内 `monolith-assets` Skill，说明何时调哪个 tool、铁律与典型流程（否则 Agent 不知如何正确用 MCP）。
7. **App 多 Agent 注册**：设置/菜单「安装 Monolith MCP」支持**多选**目标 Agent 主机，写入各自配置并释放二进制 + Skill 落点。

### 1.2 非目标（一期）

- 在 Tauri App 内嵌 MCP 服务（形态 A）——不做。
- HTTP / SSE MCP 传输。
- 非文本转换（downmark）MCP 工具——二期可选。
- 网页剪藏扩展控制面。
- 远程鉴权、多租户、云同步。
- Agent 任意路径 `rm` / 搬家物理文件。
- 替用户实现各家 Agent 的完整插件市场上架（只做本机配置写入）。

### 1.3 已冻结决策

| # | 决策 |
|---|------|
| D1 | 形态 **B**：独立 `monolith-mcp` 二进制 |
| D2 | 传输：**stdio**（各支持 MCP 的 Agent 主机） |
| D3 | 与 UI 并发：**接受**；WAL + `expected_mtime`（写正文） |
| D4 | 代码：**共享 Domain**（Rust 库，App 与 mcp 二进制同链），禁止复制一套资产逻辑 |
| D5 | **Skill 一期必做**（MCP = 能力；Skill = 调用说明书） |
| D6 | App 注册 MCP 时 **可多选 Agent 主机**（非绑定单一产品） |

---

## 2. 铁律（MCP 侧）

| # | 铁律 |
|---|------|
| M1 | **薄适配**：tools 只调 Domain API；禁止 MCP 层手写业务 SQL / `fs::remove_file`。 |
| M2 | 遵守资产 **L1 / L7**：清除 = 删登记；不删磁盘文件；不删 `conversion_log`。 |
| M3 | 遵守 **L2**：仅 Monaco 可读类型可 `register` / `write_content`。 |
| M4 | 遵守 **L5 / L6**：`write_content` = 覆盖当前版本；`save_new_version` 才出时间戳副本。 |
| M5 | 库路径默认与 App 一致；可用环境变量 `MONOLITH_ASSETS_DB` 覆盖（测试/多实例）。 |
| M6 | 写正文若调用方提供 `expected_mtime` 且与盘上不符 → **冲突错误**，不覆盖。未提供则尽力写（Agent 自担）。 |
| M7 | Tool 描述必须对 Agent 写明「delete 不删文件」。 |

---

## 3. 架构

```text
┌──────────────────────────────────┐
│  AI Agent (Cursor / Claude / …)  │
└────────────────┬─────────────────┘
                 │ MCP JSON-RPC (stdio)
                 ▼
┌──────────────────────────────────┐
│  monolith-mcp                    │
│  · tool registry + JSON Schema   │
│  · path/db bootstrap             │
│  · error → MCP tool error        │
└────────────────┬─────────────────┘
                 │
                 ▼
┌──────────────────────────────────┐
│  monolith_domain (shared crate)  │
│  assets / browse_term / versions │
│  file read-write (L2 gated)      │
└────────────────┬─────────────────┘
                 ▼
        assets.sqlite (WAL)
                 ▲
┌────────────────┴─────────────────┐
│  Monolith.app (Tauri)            │
│  同一 Domain + 同一 DB（可选并发） │
└──────────────────────────────────┘
```

### 3.1 仓库布局（建议）

```text
monolith/
  src-tauri/                 # 现有 App（逐步改为依赖 shared）
  crates/
    monolith-domain/         # 从 assets.rs 等抽出的纯库（无 Tauri UI 依赖）
    monolith-mcp/            # bin：MCP stdio server
  docs/superpowers/specs/    # 本设计
```

过渡期允许 `monolith-mcp` 先直接链 `src-tauri` 内模块；稳定后必须收到 `monolith-domain`，App 只做 invoke 外壳。

### 3.2 库路径解析

顺序：

1. 环境变量 `MONOLITH_ASSETS_DB`（绝对路径）  
2. 默认：`{app_data_dir}/com.muqiang.monolith/assets.sqlite`  
   - macOS：`~/Library/Application Support/com.muqiang.monolith/assets.sqlite`  
   - Windows：`%APPDATA%\com.muqiang.monolith\assets.sqlite`  
   - Linux：`~/.local/share/com.muqiang.monolith/assets.sqlite`

启动时：`PRAGMA journal_mode=WAL;` + `busy_timeout`（建议 ≥ 5000ms）。

---

## 4. MCP 工具面（一期）

命名空间前缀：`monolith_`（或短名 `asset_`；实现时统一一种，下文用 `asset_`）。

### 4.1 读

| Tool | 入参 | 出参要点 |
|------|------|----------|
| `asset_list_tree` | — | browse_term 树 + 资产叶（含 `index_status`） |
| `asset_get` | `asset_id` **或** `path` | 实例 + 当前版本路径 + `source_path` + stale/exists |
| `asset_read_content` | `asset_id` **或** `path`；可选 `max_bytes` | UTF-8 文本；超限截断并标注 |
| `asset_list_versions` | `asset_id` | 版本列表（路径、时间、是否 current） |

### 4.2 写（登记 + 正文）

| Tool | 入参 | 行为 |
|------|------|------|
| `asset_register` | `path`；可选 `source_path`（本地或 http(s)）、`display_name`、`browse_term_id` | ≡ 生成资产；重复路径 → 错误并返回已有 id |
| `asset_write_content` | `asset_id` **或** `path`；`content`；可选 `expected_mtime` | 覆盖**当前版本**文件（L5）；成功返回新 mtime/size |

### 4.3 修订

| Tool | 入参 | 行为 |
|------|------|------|
| `asset_save_new_version` | `asset_id`；可选 `content`（缺省=当前文件内容再拷贝命名） | L6 时间戳新文件 + 切当前 |
| `asset_rename` | `asset_id`；`display_name` | 只改展示名 |
| `asset_move` | `asset_id`；`browse_term_id` | 改挂载夹 |
| `asset_relocate` | `asset_id`；`new_path` | 换磁盘路径；追加 `conversion_log` relocated；不改历史行 |
| `term_create` / `term_rename` / `term_delete` | 夹操作 | 删夹仅空夹 |

### 4.4 清除

| Tool | 入参 | 行为 |
|------|------|------|
| `asset_delete` | `asset_id` | 删 `asset_instance` + CASCADE versions；**不**删文件；**不**删 log |

### 4.5 Resources（可选一期，建议做）

| URI | 内容 |
|-----|------|
| `monolith://assets/tree` | 树 JSON（同 list_tree） |
| `monolith://assets/{id}` | get JSON |
| `monolith://assets/{id}/content` | 当前版本正文 |

### 4.6 错误约定

- 业务失败 → MCP `isError: true`，`message` 人话 + 稳定 `code`（如 `PATH_EXISTS` / `MTIME_CONFLICT` / `UNSUPPORTED_TYPE` / `NOT_FOUND` / `INVALID_INDEX`）。
- 不向 Agent 泄露本机无关绝对路径以外的敏感环境信息。

---

## 5. 并发模型

```text
UI thread / MCP request
        │
        ▼
  short transaction (rusqlite)
        │
  WAL writers serialize; readers concurrent
```

| 场景 | 行为 |
|------|------|
| UI 与 MCP 同时读 | 允许 |
| 同时写不同行 | WAL 串行化，可成功 |
| MCP `write_content` 时 UI 已改盘上文件 | 若带 `expected_mtime` → `MTIME_CONFLICT`；Agent 应 `read` 再决定 |
| UI 内存脏未保存，MCP 写盘 | **无法检测**（未落盘）；说明写入文档：Agent 写前宜提示用户保存，或依赖用户纪律 |

一期不实现「探测 App 是否在跑」的互斥锁。

---

## 6. 多 Agent 主机注册（App 侧）

MCP 二进制只有一份；**不同 Agent 读不同配置文件**。App「安装 Monolith MCP」UI：

1. 释放/更新 `monolith-mcp` 到固定路径（如  
   `~/Library/Application Support/com.muqiang.monolith/bin/monolith-mcp`）。
2. **unix**：再建无空格软链 `~/.local/bin/monolith-mcp` → 上述二进制（部分 MCP 宿主对 `Application Support` 路径 discovery 失败）。
3. **多选**目标主机（checkbox），对勾选者合并写入其 MCP 配置；`command` 优先写软链路径。
4. 同步安装/更新 **Skill** 到该主机约定目录（见 §6.2）。

### 6.1 一期建议支持的主机

| 主机 | MCP 配置落点（macOS 典型） | 备注 |
|------|---------------------------|------|
| **Cursor** | `~/.cursor/mcp.json`（或项目 `.cursor/mcp.json`，App 默认写用户级） | 一期必选 |
| **Claude Desktop** | `~/Library/Application Support/Claude/claude_desktop_config.json` 内 `mcpServers` | 一期建议支持 |
| 其它（Codex / 自定义） | 二期：导出 JSON 片段或用户自填路径 | 一期可用「复制配置」兜底 |

写入策略：**合并** key `monolith`（或 `monolith-assets`），不整文件覆盖；写前备份原文件。

配置骨架（各主机字段名以官方为准，语义相同）：

```json
{
  "mcpServers": {
    "monolith": {
      "command": "/Users/…/.local/bin/monolith-mcp",
      "args": []
    }
  }
}
```

真实二进制仍在 App Support `bin/`；Agent 配置只引用无空格软链。

### 6.2 Skill（一期必做）

| 项 | 约定 |
|----|------|
| 名称 | `monolith-assets` |
| 仓库路径 | `skills/monolith-assets/SKILL.md`（随 App 资源打包） |
| 内容 | 触发场景；tool 选用表；L1/L5/L6/L7；`expected_mtime`；禁止当「删文件」；剪藏登记示例流程 |
| Cursor 安装 | 复制到用户/项目 skills 约定目录（与现有 Cursor skills 布局对齐） |
| Claude Desktop | 若无独立 skills 目录：把 Skill 正文写入 MCP 配套 `prompts` 或附 `SKILL.md` 到 App Support 并在安装说明中指引；**最低限度**保证 Cursor 路径可用 |

**关系：**

```text
MCP tools     = Agent「能调用什么」
Skill         = Agent「该怎么调、何时调、禁区」
App 安装器    = 把二进制 + 配置 + Skill 装到用户勾选的 Agent 上
```

没有 Skill：模型可能乱调 tool 或把 `asset_delete` 理解成删磁盘文件——故 Skill **非可选文档**，算一期交付物。

---

## 7. 安全

1. **默认不**限制根目录到某一文件夹（本机个人知识库工具）；若后续需要，加 `MONOLITH_MCP_ALLOW_ROOTS`（多路径前缀白名单）再收紧 `register` / `relocate` / `write`。  
2. 拒绝 path traversal 到明显危险位置不作一期范围；L2 类型门禁优先。  
3. Tool 描述禁止暗示「可删除磁盘文件」。

---

## 8. 实现顺序

1. 抽出 / 稳定 Domain API（或 mcp crate 先复用 `assets` 模块 + WAL）。  
2. 实现 `monolith-mcp` stdio（推荐 `rmcp` 或官方 Rust MCP SDK）。  
3. 落地读工具 → 登记/写正文 → 修订/清除 → Resources。  
4. 编写 `skills/monolith-assets/SKILL.md`（与 tool 名严格一致）。  
5. App：「安装 Monolith MCP」多选主机 + 合并写配置 + 释放 bin + 安装 Skill（Cursor 必达）。  
6. 并发单测：WAL；`expected_mtime` 冲突。  
7. README：手工配置与 App 一键安装说明。

---

## 9. DoD（一期）

1. `monolith-mcp` 可被 Cursor 以 stdio 拉起并列出 tools。  
2. Agent 能 `list_tree` / `get` / `read_content`。  
3. Agent 能 `register` 已有文本文件；重复路径报错。  
4. Agent 能 `write_content`（L5）与 `save_new_version`（L6）；带错误 mtime 时冲突。  
5. Agent 能 `delete` 后文件仍在磁盘、库中无该资产。  
6. App 打开时 MCP 同时读写不致库损坏（WAL + busy_timeout）。  
7. 无第二套资产业务 SQL。  
8. **`monolith-assets` Skill 已随仓库/安装交付**，且描述与 tools 一致。  
9. App 安装器支持 **至少 Cursor**；UI 上可多选主机（Claude Desktop 能写则写，否则「复制配置」）。

---

## 10. 二期（不阻塞）

- 更多 Agent 主机预设（Codex、Windsurf 等）。  
- `convert_run` MCP（downmark）。  
- `MONOLITH_MCP_ALLOW_ROOTS` 沙箱。  
- 审计表 `mcp_audit_log`。  
- HTTP MCP（远程 Agent）。
