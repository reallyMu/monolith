//! App-level settings (working directories, defaults). Stored as JSON under App Support.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Import directory: web clips and Convert→Markdown default output.
    pub inbox_dir: String,
    /// Preferred directory for Open / Convert file dialogs (empty = OS default).
    pub default_open_dir: String,
    /// Preferred directory when Save As without an existing path (empty = OS default).
    pub default_save_dir: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let downloads = home.join("Downloads");
        Self {
            inbox_dir: downloads
                .join("MonolithInbox")
                .to_string_lossy()
                .into_owned(),
            default_open_dir: String::new(),
            default_save_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub settings: AppSettings,
    /// Resolved paths for display (not all editable).
    pub assets_db_path: String,
    pub app_data_dir: String,
    pub mcp_binary_path: String,
    pub clipper_release_dir: String,
}

pub(crate) fn app_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME not set".to_string())?;
    Ok(PathBuf::from(home).join("Library/Application Support/com.muqiang.monolith"))
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join(SETTINGS_FILE))
}

pub fn load_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.is_file() {
        return Ok(AppSettings::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut s: AppSettings = serde_json::from_str(&raw).unwrap_or_default();
    // Fill empties with defaults so UI always shows concrete inbox path.
    let def = AppSettings::default();
    if s.inbox_dir.trim().is_empty() {
        s.inbox_dir = def.inbox_dir;
    }
    Ok(s)
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let dir = app_data_dir()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(SETTINGS_FILE);
    let pretty = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, pretty + "\n").map_err(|e| e.to_string())
}

/// Inbox directory from settings; create if missing.
pub fn resolve_inbox_dir() -> Result<PathBuf, String> {
    let s = load_settings()?;
    let dir = PathBuf::from(s.inbox_dir.trim());
    if dir.as_os_str().is_empty() {
        return Err("inbox_dir is empty".into());
    }
    if !dir.is_absolute() {
        return Err("inbox_dir must be an absolute path".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn validate_optional_dir(label: &str, value: &str) -> Result<(), String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(());
    }
    let p = Path::new(t);
    if !p.is_absolute() {
        return Err(format!("{label} must be an absolute path"));
    }
    Ok(())
}

#[tauri::command]
pub fn settings_get() -> Result<SettingsView, String> {
    let settings = load_settings()?;
    let data = app_data_dir()?;
    let assets = crate::assets::default_assets_db_path()?;
    let mcp = data.join("bin/monolith-mcp");
    let clipper = crate::clipper::clipper_release_dir().unwrap_or_else(|_| data.join("MonolithClipper"));
    Ok(SettingsView {
        settings,
        assets_db_path: assets.to_string_lossy().into_owned(),
        app_data_dir: data.to_string_lossy().into_owned(),
        mcp_binary_path: mcp.to_string_lossy().into_owned(),
        clipper_release_dir: clipper.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn settings_save(settings: AppSettings) -> Result<SettingsView, String> {
    let mut s = settings;
    s.inbox_dir = s.inbox_dir.trim().to_string();
    s.default_open_dir = s.default_open_dir.trim().to_string();
    s.default_save_dir = s.default_save_dir.trim().to_string();
    if s.inbox_dir.is_empty() {
        return Err("导入目录不能为空".into());
    }
    let inbox = PathBuf::from(&s.inbox_dir);
    if !inbox.is_absolute() {
        return Err("导入目录必须是绝对路径".into());
    }
    validate_optional_dir("default_open_dir", &s.default_open_dir)?;
    validate_optional_dir("default_save_dir", &s.default_save_dir)?;
    fs::create_dir_all(&inbox).map_err(|e| format!("无法创建导入目录: {e}"))?;
    save_settings(&s)?;
    settings_get()
}
