//! Web clipper install helpers (extension release under App Support).

use crate::assets::AssetDb;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
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
    /// Unix seconds (mtime). 0 if unknown.
    pub mtime: i64,
    /// conversion_log input_path (URL or source file); empty if none.
    pub source: String,
    /// `clip` | `convert` | `unknown`
    pub kind: String,
}

fn mtime_secs(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn classify_kind(tool: &str, input: &str) -> &'static str {
    let t = tool.trim();
    let inp = input.trim();
    if inp.starts_with("http://") || inp.starts_with("https://") || t == "monolith-clipper" {
        "clip"
    } else if !inp.is_empty() || t == "downmark" {
        "convert"
    } else {
        "unknown"
    }
}

/// Latest success (input_path, tool) per output_path. Read-only; does not insert logs.
fn latest_success_origins(conn: &Connection) -> Result<HashMap<String, (String, String)>, String> {
    let mut map = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT output_path, input_path, tool FROM conversion_log
             WHERE status = 'success'
             ORDER BY finished_at DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (out, inp, tool) = row.map_err(|e| e.to_string())?;
        map.entry(out).or_insert((inp, tool));
    }
    Ok(map)
}

fn lookup_asset_id(conn: &rusqlite::Connection, abs: &str, dir: &Path, name: &str) -> Option<i64> {
    let asset_id: Option<i64> = conn
        .query_row(
            "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
            params![abs],
            |r| r.get(0),
        )
        .ok();
    asset_id.or_else(|| {
        let raw = dir.join(name).to_string_lossy().into_owned();
        conn.query_row(
            "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
            params![&raw],
            |r| r.get(0),
        )
        .ok()
    })
}

/// List top-level `.md` files in `dir` and whether each path is already an asset.
fn list_md_entries_in_dir(
    dir: &Path,
    conn: &Connection,
) -> Result<Vec<InboxEntryDto>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let origins = latest_success_origins(conn)?;
    let mut entries = Vec::new();
    let rd = fs::read_dir(dir).map_err(|e| e.to_string())?;
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
            .unwrap_or(path.clone())
            .to_string_lossy()
            .into_owned();
        let asset_id = lookup_asset_id(conn, &abs, dir, &name);
        let raw = dir.join(&name).to_string_lossy().into_owned();
        let (source, kind) = origins
            .get(&abs)
            .or_else(|| origins.get(&raw))
            .map(|(inp, tool)| (inp.clone(), classify_kind(tool, inp).to_string()))
            .unwrap_or_else(|| (String::new(), "unknown".into()));
        entries.push(InboxEntryDto {
            path: abs,
            name,
            registered: asset_id.is_some(),
            asset_id,
            mtime: mtime_secs(&path),
            source,
            kind,
        });
    }
    entries.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

/// List Markdown files in the import directory and whether each is already an asset.
#[tauri::command]
pub fn clipper_list_inbox(db: State<'_, AssetDb>) -> Result<Vec<InboxEntryDto>, String> {
    let dir = PathBuf::from(clipper_inbox_dir()?);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list_md_entries_in_dir(&dir, &conn)
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

#[cfg(test)]
mod tests {
    use super::classify_kind;

    #[test]
    fn classify_clip_from_url_or_tool() {
        assert_eq!(classify_kind("", "https://example.com/a"), "clip");
        assert_eq!(classify_kind("monolith-clipper", "https://x.test"), "clip");
    }

    #[test]
    fn classify_convert_from_path_or_downmark() {
        assert_eq!(classify_kind("downmark", "/data/a.pdf"), "convert");
        assert_eq!(classify_kind("", "/data/a.docx"), "convert");
        assert_eq!(classify_kind("", ""), "unknown");
    }
}
