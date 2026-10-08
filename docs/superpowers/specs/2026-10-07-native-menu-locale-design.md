# Monolith — 原生菜单跟随系统语言

> 状态：已批准 / 已实现  
> 日期：2026-10-07

## 决定

1. **默认 = 系统语言**：原生菜单随 OS UI 语言。解析顺序：`MONOLITH_LANG` → macOS `AppleLocale`/`AppleLanguages` → `LC_*`/`LANG`（跳过 `C`/`POSIX`）。与前端 `navigator.language` 同一规则：`zh*` → 中文，其它 → 英文。
2. **多语能力**：菜单字符串走键表（`MenuMsg`），新增语种 = 加 `Locale` 变体 + 表，不硬编码散落英文。
3. **系统预置项**：剪切 / 复制 / 粘贴 / 全选 / 最小化等 `PredefinedMenuItem(..., None)` 交给 OS 本地化，不抢译。
4. **本期不做**：设置里手动切语言、运行中热切换菜单（需重建 menu；可后置）。

## 验收

1. 系统为中文时（即使进程 `LANG=C.UTF-8`）：菜单「文件」「目录导入…」。
2. `MONOLITH_LANG=en` 启动时菜单为英文。
3. 单元测试覆盖 locale 解析与中英菜单键。
