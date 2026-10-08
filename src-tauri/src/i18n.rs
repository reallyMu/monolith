//! Native UI locale (menus). Follows OS language; tables are keyed for more locales later.
//! Frontend `src/i18n` uses the same rule: `zh*` → Zh, else En (until more packs ship).

use std::env;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Zh,
    En,
}

static LOCALE: OnceLock<Locale> = OnceLock::new();

pub fn locale() -> Locale {
    *LOCALE.get_or_init(detect_locale)
}

/// Prefer explicit override, then OS UI language, then process locale tags.
pub fn detect_locale() -> Locale {
    if let Ok(v) = env::var("MONOLITH_LANG") {
        if let Some(loc) = parse_locale_tag(&v) {
            return loc;
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(v) = macos_system_lang() {
            if let Some(loc) = parse_locale_tag(&v) {
                return loc;
            }
        }
    }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = env::var(key) {
            if let Some(loc) = parse_locale_tag(&v) {
                return loc;
            }
        }
    }
    Locale::En
}

/// macOS GUI apps often inherit `LANG=C` from the launcher; System Settings language
/// lives in AppleLocale / AppleLanguages.
#[cfg(target_os = "macos")]
fn macos_system_lang() -> Option<String> {
    if let Some(s) = defaults_read_global("AppleLocale") {
        return Some(s);
    }
    // AppleLanguages prints a plist list; first quoted token is enough.
    let raw = defaults_read_global_raw("AppleLanguages")?;
    for part in raw.split(['"', '\'', ',', '(', ')', '\n', ' ', '\t']) {
        let t = part.trim();
        if !t.is_empty() && t.chars().any(|c| c.is_ascii_alphabetic()) {
            return Some(t.to_string());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn defaults_read_global(key: &str) -> Option<String> {
    let s = defaults_read_global_raw(key)?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[cfg(target_os = "macos")]
fn defaults_read_global_raw(key: &str) -> Option<String> {
    let out = std::process::Command::new("defaults")
        .args(["read", "-g", key])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

fn parse_locale_tag(raw: &str) -> Option<Locale> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let primary = t
        .split(['.', '@', '_', '-'])
        .next()
        .unwrap_or(t)
        .to_ascii_lowercase();
    // C / POSIX / C.UTF-8 → no opinion; try next source.
    if primary == "c" || primary == "posix" {
        return None;
    }
    if primary.starts_with("zh") {
        Some(Locale::Zh)
    } else {
        // Known pack or future packs: unknown → En until a table exists.
        Some(Locale::En)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MenuMsg {
    File,
    Edit,
    View,
    Window,
    New,
    Open,
    Save,
    SaveAs,
    ConvertMd,
    BrowseInbox,
    FolderImport,
    RegisterAsset,
    SaveNewVersion,
    Settings,
    CloseTab,
    ClearRecent,
    OpenRecent,
    RecentEmpty,
    QuitApp,
    Undo,
    Redo,
    Find,
    EditOnly,
    PreviewOnly,
    CompareRender,
    CycleView,
}

pub fn menu(msg: MenuMsg) -> &'static str {
    match locale() {
        Locale::Zh => menu_zh(msg),
        Locale::En => menu_en(msg),
    }
}

fn menu_zh(msg: MenuMsg) -> &'static str {
    match msg {
        MenuMsg::File => "文件",
        MenuMsg::Edit => "编辑",
        MenuMsg::View => "视图",
        MenuMsg::Window => "窗口",
        MenuMsg::New => "新建",
        MenuMsg::Open => "打开…",
        MenuMsg::Save => "保存",
        MenuMsg::SaveAs => "另存为…",
        MenuMsg::ConvertMd => "转换为 Markdown…",
        MenuMsg::BrowseInbox => "导入资源浏览器…",
        MenuMsg::FolderImport => "目录导入…",
        MenuMsg::RegisterAsset => "登记资产…",
        MenuMsg::SaveNewVersion => "保存为新版本",
        MenuMsg::Settings => "系统设置…",
        MenuMsg::CloseTab => "关闭标签页",
        MenuMsg::ClearRecent => "清空最近",
        MenuMsg::OpenRecent => "打开最近使用的",
        MenuMsg::RecentEmpty => "（空）",
        MenuMsg::QuitApp => "退出 Monolith",
        MenuMsg::Undo => "撤销",
        MenuMsg::Redo => "重做",
        MenuMsg::Find => "查找…",
        MenuMsg::EditOnly => "仅编辑",
        MenuMsg::PreviewOnly => "仅预览",
        MenuMsg::CompareRender => "渲染对照",
        MenuMsg::CycleView => "切换 编辑 / 预览 / 对照",
    }
}

fn menu_en(msg: MenuMsg) -> &'static str {
    match msg {
        MenuMsg::File => "File",
        MenuMsg::Edit => "Edit",
        MenuMsg::View => "View",
        MenuMsg::Window => "Window",
        MenuMsg::New => "New",
        MenuMsg::Open => "Open…",
        MenuMsg::Save => "Save",
        MenuMsg::SaveAs => "Save As…",
        MenuMsg::ConvertMd => "Convert to Markdown…",
        MenuMsg::BrowseInbox => "Import Resource Browser…",
        MenuMsg::FolderImport => "Import Folder…",
        MenuMsg::RegisterAsset => "Register Asset…",
        MenuMsg::SaveNewVersion => "Save as New Version",
        MenuMsg::Settings => "Settings…",
        MenuMsg::CloseTab => "Close Tab",
        MenuMsg::ClearRecent => "Clear Recent",
        MenuMsg::OpenRecent => "Open Recent",
        MenuMsg::RecentEmpty => "(Empty)",
        MenuMsg::QuitApp => "Quit Monolith",
        MenuMsg::Undo => "Undo",
        MenuMsg::Redo => "Redo",
        MenuMsg::Find => "Find…",
        MenuMsg::EditOnly => "Edit Only",
        MenuMsg::PreviewOnly => "Preview Only",
        MenuMsg::CompareRender => "Compare Render",
        MenuMsg::CycleView => "Cycle Edit / Preview / Compare",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zh_tags() {
        assert_eq!(parse_locale_tag("zh_CN.UTF-8"), Some(Locale::Zh));
        assert_eq!(parse_locale_tag("zh-Hans"), Some(Locale::Zh));
        assert_eq!(parse_locale_tag("ZH_TW"), Some(Locale::Zh));
    }

    #[test]
    fn parses_en_and_skips_c() {
        assert_eq!(parse_locale_tag("en_US.UTF-8"), Some(Locale::En));
        assert_eq!(parse_locale_tag("ja_JP.UTF-8"), Some(Locale::En));
        assert_eq!(parse_locale_tag("C"), None);
        assert_eq!(parse_locale_tag("C.UTF-8"), None);
        assert_eq!(parse_locale_tag("POSIX"), None);
        assert_eq!(parse_locale_tag(""), None);
    }

    #[test]
    fn menu_zh_and_en_differ_for_file() {
        assert_eq!(menu_zh(MenuMsg::File), "文件");
        assert_eq!(menu_en(MenuMsg::File), "File");
        assert_eq!(menu_zh(MenuMsg::FolderImport), "目录导入…");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_system_lang_readable() {
        // Machine under test has System Settings language; must not panic.
        let _ = macos_system_lang();
    }
}
