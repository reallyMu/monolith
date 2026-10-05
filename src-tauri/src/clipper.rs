//! Web clipper install helpers (extension release under App Support).

use crate::assets::AssetDb;
use rusqlite::params;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

/// Unpacked Chrome extension — App-managed under Application Support (not Downloads).
const EXT_DIR_NAME: &str = "MonolithClipper";
const INBOX_DIR_NAME: &str = "MonolithInbox";

fn dirs_downloads() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads"))
}

fn app_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME not set".to_string())?;
    Ok(PathBuf::from(home).join("Library/Application Support/com.muqiang.monolith"))
}

/// Released unpacked extension path: `…/com.muqiang.monolith/MonolithClipper`.
pub fn clipper_release_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join(EXT_DIR_NAME))
}

fn app_clipper_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    let dir = clipper_release_dir()?;
    fs::create_dir_all(dir.parent().unwrap_or(Path::new("/"))).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn bundled_clipper_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let res = app.path().resource_dir().map_err(|e| e.to_string())?;
    let p = res.join("clipper-extension");
    if p.is_dir() && p.join("manifest.json").is_file() {
        return Ok(p);
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("clipper-extension");
    if dev.is_dir() && dev.join("manifest.json").is_file() {
        return Ok(dev);
    }
    Err(format!(
        "Bundled clipper not found under {} or {}",
        p.display(),
        dev.display()
    ))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Ensure clip inbox exists (from settings; default ~/Downloads/MonolithInbox).
#[tauri::command]
pub fn clipper_inbox_dir() -> Result<String, String> {
    let dir = crate::settings::resolve_inbox_dir().or_else(|_| -> Result<PathBuf, String> {
        let dir = dirs_downloads()
            .unwrap_or_else(|| std::env::temp_dir().join("Downloads"))
            .join(INBOX_DIR_NAME);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    })?;
    Ok(dir.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxEntryDto {
    pub path: String,
    pub name: String,
    pub registered: bool,
    pub asset_id: Option<i64>,
}

/// List Markdown clips in MonolithInbox and whether each is already an asset.
#[tauri::command]
pub fn clipper_list_inbox(db: State<'_, AssetDb>) -> Result<Vec<InboxEntryDto>, String> {
    let dir = PathBuf::from(clipper_inbox_dir()?);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    let rd = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        let path = ent.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "md" {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled.md")
            .to_string();
        let abs = path
            .canonicalize()
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let asset_id: Option<i64> = conn
            .query_row(
                "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
                params![&abs],
                |r| r.get(0),
            )
            .ok();
        // Also try non-canonical form (registration may have stored the raw path).
        let asset_id = asset_id.or_else(|| {
            let raw = dir.join(&name).to_string_lossy().into_owned();
            conn.query_row(
                "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
                params![&raw],
                |r| r.get(0),
            )
            .ok()
        });
        entries.push(InboxEntryDto {
            path: abs,
            name,
            registered: asset_id.is_some(),
            asset_id,
        });
    }
    entries.sort_by(|a, b| {
        let ma = fs::metadata(&a.path).and_then(|m| m.modified()).ok();
        let mb = fs::metadata(&b.path).and_then(|m| m.modified()).ok();
        mb.cmp(&ma).then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

#[tauri::command]
pub fn clipper_ensure_extension(app: AppHandle) -> Result<String, String> {
    let bundled = bundled_clipper_dir(&app)?;
    let dest = app_clipper_dir(&app)?;
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    copy_dir_recursive(&bundled, &dest)?;
    Ok(dest.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn clipper_extension_dir(app: AppHandle) -> Result<String, String> {
    let dest = app_clipper_dir(&app)?;
    if !dest.join("manifest.json").is_file() {
        return clipper_ensure_extension(app);
    }
    Ok(dest.to_string_lossy().into_owned())
}

/// Reveal extension folder in Finder and open Chrome extensions page.
#[tauri::command]
pub fn clipper_open_install(app: AppHandle) -> Result<String, String> {
    let dest = clipper_ensure_extension(app)?;
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&dest).status();
        let _ = std::process::Command::new("open")
            .arg("-a")
            .arg("Google Chrome")
            .arg("chrome://extensions")
            .status();
    }
    Ok(dest)
}
