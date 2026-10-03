# Monolith — 本机 Monaco 文本编辑器（v1）

> 状态：已批准 / 开工依据  
> 日期：2026-10-03  
> 栈：Tauri 2 + Vite + Vue 3 + Monaco + markdown-it

## 1. 目标

macOS 本机多标签文本编辑器：编辑 / 预览 / 左右对照三态；打开、保存、另存为、最近文件；Monaco 支持的文本类型均可编辑。

## 2. 冻结决定

1. 预览：Markdown 渲染 HTML；其它类型右侧只读 Monaco 高亮。
2. 多标签页（非单文件）。
3. macOS 原生菜单栏 + 窗口内工具栏双入口。
4. 工程路径：`/Users/muqiang/.openclaw/workspace/monolith`（独立，不进 healix-cdh）。
5. 编码 v1 固定 UTF-8；二进制拒绝打开。
6. 应用名：Monolith。

## 3. 界面

- 菜单：文件（新建/打开/保存/另存为/最近打开/关闭）、编辑、视图（三态）、窗口
- 工具栏分两横条：① 文件（新建/打开/保存/另存/最近）+ 视图 + 左右背景色；② 格式（撤销/重做/查找 + MD 插入）
- Tab 栏 + 主区（编辑 / 预览 / 对照可拖分割）+ 状态栏（路径、语言、行列、脏标记）
- 对照模式：左右比例滚动同步；当前行双侧高亮；左右背景色可分别设置（localStorage 持久化）

## 4. 文件与 Tab

每 Tab：path | content | dirty | language | viewMode。  
最近打开最多 20 条，存 app data JSON。关闭脏 Tab 前确认。

## 5. 预览

- `.md` / `.markdown`：markdown-it（GFM 倾向）
- 其它：只读 Monaco，同语言
- 单向：编辑 → 预览

## 6. Rust 命令

`read_text_file` / `write_text_file` / `list_recent` / `push_recent` / `clear_recent`

## 7. v1 不做

插件、Git、云同步、多窗口、打印、HTML 沙箱执行、系统默认打开方式自动化。

## 8. DoD

- `npm run tauri dev` 可启动
- 三态切换、多 Tab、打开保存另存、最近列表、MD 预览与非 MD 只读预览可用
