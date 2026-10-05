# Monolith MCP + Inbox 浏览 — 实现计划

> 设计：`docs/superpowers/specs/2026-10-05-monolith-mcp-assets-design.md`  
> 附加需求：工具栏浏览 `MonolithInbox`，标记已/未资产化，选择打开。

## 文件

| 路径 | 职责 |
|------|------|
| `src-tauri/src/clipper.rs` / inbox API | 列出 Inbox `.md` + 是否已登记 |
| `src/App.vue` + i18n | 工具栏 + 弹层列表 |
| `src-tauri` WAL + `open_db_at` | MCP/UI 共用库 |
| `src-tauri/src/bin/monolith_mcp.rs` | stdio MCP server |
| `skills/monolith-assets/SKILL.md` | Agent 调用说明书 |
| App 安装 MCP（合并 mcp.json） | 释放 bin + 写 Cursor 配置 |

## 任务顺序

1. Inbox list API + toolbar UI  
2. WAL + db path helper（无 AppHandle）  
3. Skill  
4. monolith-mcp：initialize / tools/list / tools/call（读+登记+写+删）  
5. App「安装 MCP」写入 Cursor  
6. 构建验证  

---

I'm using the writing-plans skill to create the implementation plan; executing it in this session.
