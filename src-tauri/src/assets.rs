//! Local document asset ledger (SQLite). Physical files are never moved or deleted by these APIs.

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

const DB_NAME: &str = "assets.sqlite";
const ROOT_CODE: &str = "root";
const MAX_TERM_DEPTH: usize = 5;

pub struct AssetDb(pub Mutex<Connection>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseTermDto {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub code: String,
    pub display_name: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDto {
    pub id: i64,
    pub display_name: String,
    pub file_type: String,
    pub browse_term_id: i64,
    pub current_version_id: Option<i64>,
    pub absolute_path: Option<String>,
    pub file_exists: bool,
    pub created_at: String,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTreeDto {
    pub terms: Vec<BrowseTermDto>,
    pub assets: Vec<AssetDto>,
    pub root_term_id: i64,
}

fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn stamp_local() -> String {
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(DB_NAME))
}

pub fn init_asset_db(app: &AppHandle) -> Result<AssetDb, String> {
    let path = db_path(app)?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS asset_browse_term (
          id INTEGER PRIMARY KEY,
          parent_id INTEGER REFERENCES asset_browse_term(id),
          code TEXT NOT NULL UNIQUE,
          display_name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          status TEXT NOT NULL DEFAULT 'ACTIVE',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS asset_instance (
          id INTEGER PRIMARY KEY,
          display_name TEXT NOT NULL,
          file_type TEXT NOT NULL,
          browse_term_id INTEGER NOT NULL REFERENCES asset_browse_term(id),
          current_version_id INTEGER,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS asset_version (
          id INTEGER PRIMARY KEY,
          asset_id INTEGER NOT NULL REFERENCES asset_instance(id) ON DELETE CASCADE,
          absolute_path TEXT NOT NULL UNIQUE,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS asset_relationship (
          id INTEGER PRIMARY KEY,
          from_asset_id INTEGER NOT NULL REFERENCES asset_instance(id) ON DELETE CASCADE,
          rel_type TEXT NOT NULL DEFAULT 'derived_from',
          to_external_path TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let ts = now_iso();
    conn.execute(
        "INSERT OR IGNORE INTO asset_browse_term (id, parent_id, code, display_name, sort_order, status, created_at, updated_at)
         VALUES (1, NULL, ?1, 'Root', 0, 'ACTIVE', ?2, ?2)",
        params![ROOT_CODE, ts],
    )
    .map_err(|e| e.to_string())?;

    Ok(AssetDb(Mutex::new(conn)))
}

fn root_id(conn: &Connection) -> Result<i64, String> {
    conn.query_row(
        "SELECT id FROM asset_browse_term WHERE code = ?1",
        params![ROOT_CODE],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

fn normalize_path(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err("Path must be absolute".into());
    }
    Ok(path.to_string())
}

fn file_type_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| {
            let base = Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if base == "dockerfile" {
                "dockerfile".into()
            } else {
                "txt".into()
            }
        })
}

fn display_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

fn term_depth(conn: &Connection, mut id: i64) -> Result<usize, String> {
    let mut depth = 1usize;
    loop {
        let parent: Option<i64> = conn
            .query_row(
                "SELECT parent_id FROM asset_browse_term WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        match parent {
            Some(p) => {
                depth += 1;
                if depth > MAX_TERM_DEPTH {
                    return Ok(depth);
                }
                id = p;
            }
            None => return Ok(depth),
        }
    }
}

fn is_descendant(conn: &Connection, ancestor: i64, mut node: i64) -> Result<bool, String> {
    loop {
        if node == ancestor {
            return Ok(true);
        }
        let parent: Option<i64> = conn
            .query_row(
                "SELECT parent_id FROM asset_browse_term WHERE id = ?1",
                params![node],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        match parent {
            Some(p) => node = p,
            None => return Ok(false),
        }
    }
}

fn list_terms(conn: &Connection) -> Result<Vec<BrowseTermDto>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_id, code, display_name, sort_order FROM asset_browse_term
             WHERE status = 'ACTIVE' ORDER BY sort_order, id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(BrowseTermDto {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                code: r.get(2)?,
                display_name: r.get(3)?,
                sort_order: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn list_assets(conn: &Connection) -> Result<Vec<AssetDto>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.display_name, a.file_type, a.browse_term_id, a.current_version_id,
                    a.created_at, v.absolute_path,
                    (SELECT r.to_external_path FROM asset_relationship r
                     WHERE r.from_asset_id = a.id AND r.rel_type = 'derived_from'
                     ORDER BY r.id LIMIT 1)
             FROM asset_instance a
             LEFT JOIN asset_version v ON v.id = a.current_version_id
             ORDER BY a.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let path: Option<String> = r.get(6)?;
            let exists = path.as_ref().map(|p| Path::new(p).is_file()).unwrap_or(false);
            Ok(AssetDto {
                id: r.get(0)?,
                display_name: r.get(1)?,
                file_type: r.get(2)?,
                browse_term_id: r.get(3)?,
                current_version_id: r.get(4)?,
                created_at: r.get(5)?,
                absolute_path: path,
                file_exists: exists,
                source_path: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_list_tree(db: State<'_, AssetDb>) -> Result<AssetTreeDto, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(AssetTreeDto {
        root_term_id: root_id(&conn)?,
        terms: list_terms(&conn)?,
        assets: list_assets(&conn)?,
    })
}

#[tauri::command]
pub fn asset_create_from_path(
    db: State<'_, AssetDb>,
    path: String,
    source_path: Option<String>,
) -> Result<AssetDto, String> {
    let abs = normalize_path(&path)?;
    if !Path::new(&abs).is_file() {
        return Err(format!("File not found: {abs}"));
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if let Some(existing) = conn
        .query_row(
            "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
            params![&abs],
            |r| r.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Err(format!("ALREADY_REGISTERED:{existing}"));
    }

    let root = root_id(&conn)?;
    let ts = now_iso();
    let name = display_name_of(&abs);
    let ftype = file_type_of(&abs);

    conn.execute(
        "INSERT INTO asset_instance (display_name, file_type, browse_term_id, current_version_id, created_at)
         VALUES (?1, ?2, ?3, NULL, ?4)",
        params![&name, &ftype, root, &ts],
    )
    .map_err(|e| e.to_string())?;
    let asset_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
        params![asset_id, &abs, &ts],
    )
    .map_err(|e| e.to_string())?;
    let ver_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
        params![ver_id, asset_id],
    )
    .map_err(|e| e.to_string())?;

    if let Some(src) = source_path {
        let src = src.trim();
        if !src.is_empty() {
            conn.execute(
                "INSERT INTO asset_relationship (from_asset_id, rel_type, to_external_path, created_at)
                 VALUES (?1, 'derived_from', ?2, ?3)",
                params![asset_id, src, &ts],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    list_assets(&conn)?
        .into_iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| "Created asset missing after insert".into())
}

#[tauri::command]
pub fn asset_delete(db: State<'_, AssetDb>, asset_id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // CASCADE versions + relationships; never touch filesystem
    let n = conn
        .execute("DELETE FROM asset_instance WHERE id = ?1", params![asset_id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Asset not found".into());
    }
    Ok(())
}

#[tauri::command]
pub fn asset_move(db: State<'_, AssetDb>, asset_id: i64, browse_term_id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM asset_browse_term WHERE id = ?1 AND status = 'ACTIVE'",
            params![browse_term_id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(false);
    if !exists {
        return Err("Browse term not found".into());
    }
    let n = conn
        .execute(
            "UPDATE asset_instance SET browse_term_id = ?1 WHERE id = ?2",
            params![browse_term_id, asset_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Asset not found".into());
    }
    Ok(())
}

#[tauri::command]
pub fn asset_rename(db: State<'_, AssetDb>, asset_id: i64, display_name: String) -> Result<(), String> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err("Display name required".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let n = conn
        .execute(
            "UPDATE asset_instance SET display_name = ?1 WHERE id = ?2",
            params![name, asset_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Asset not found".into());
    }
    Ok(())
}

#[tauri::command]
pub fn asset_relocate(db: State<'_, AssetDb>, asset_id: i64, new_path: String) -> Result<AssetDto, String> {
    let abs = normalize_path(&new_path)?;
    if !Path::new(&abs).is_file() {
        return Err(format!("File not found: {abs}"));
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let cur_ver: i64 = conn
        .query_row(
            "SELECT current_version_id FROM asset_instance WHERE id = ?1",
            params![asset_id],
            |r| r.get(0),
        )
        .map_err(|_| "Asset not found".to_string())?;

    if let Some(other) = conn
        .query_row(
            "SELECT asset_id FROM asset_version WHERE absolute_path = ?1 AND id != ?2",
            params![&abs, cur_ver],
            |r| r.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Err(format!("Path already registered to asset {other}"));
    }

    conn.execute(
        "UPDATE asset_version SET absolute_path = ?1 WHERE id = ?2",
        params![&abs, cur_ver],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE asset_instance SET file_type = ?1 WHERE id = ?2",
        params![file_type_of(&abs), asset_id],
    )
    .map_err(|e| e.to_string())?;

    list_assets(&conn)?
        .into_iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| "Asset missing after relocate".into())
}

#[tauri::command]
pub fn asset_find_by_path(db: State<'_, AssetDb>, path: String) -> Result<Option<AssetDto>, String> {
    let abs = normalize_path(&path)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let asset_id: Option<i64> = conn
        .query_row(
            "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
            params![&abs],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(id) = asset_id else {
        return Ok(None);
    };
    Ok(list_assets(&conn)?.into_iter().find(|a| a.id == id))
}

fn next_version_path(current: &Path) -> Result<PathBuf, String> {
    let parent = current
        .parent()
        .ok_or_else(|| "Current path has no parent directory".to_string())?;
    let stem = current
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;
    let ext = current
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let stamp = stamp_local();
    let mut candidate = parent.join(format!("{stem}_{stamp}{ext}"));
    let mut n = 2u32;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}_{stamp}_{n}{ext}"));
        n += 1;
        if n > 1000 {
            return Err("Could not allocate version file name".into());
        }
    }
    Ok(candidate)
}

#[tauri::command]
pub fn asset_save_new_version(
    db: State<'_, AssetDb>,
    asset_id: i64,
    content: String,
) -> Result<AssetDto, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let cur_path: String = conn
        .query_row(
            "SELECT v.absolute_path FROM asset_instance a
             JOIN asset_version v ON v.id = a.current_version_id
             WHERE a.id = ?1",
            params![asset_id],
            |r| r.get(0),
        )
        .map_err(|_| "Asset or current version not found".to_string())?;

    let new_path = next_version_path(Path::new(&cur_path))?;
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&new_path, content.as_bytes()).map_err(|e| e.to_string())?;
    let abs = new_path.to_string_lossy().into_owned();
    let ts = now_iso();

    conn.execute(
        "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
        params![asset_id, &abs, &ts],
    )
    .map_err(|e| e.to_string())?;
    let ver_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE asset_instance SET current_version_id = ?1, file_type = ?2 WHERE id = ?3",
        params![ver_id, file_type_of(&abs), asset_id],
    )
    .map_err(|e| e.to_string())?;

    list_assets(&conn)?
        .into_iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| "Asset missing after new version".into())
}

#[tauri::command]
pub fn term_create(
    db: State<'_, AssetDb>,
    parent_id: i64,
    display_name: String,
) -> Result<BrowseTermDto, String> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err("Folder name required".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let depth = term_depth(&conn, parent_id)?;
    if depth >= MAX_TERM_DEPTH {
        return Err(format!("Browse tree max depth is {MAX_TERM_DEPTH}"));
    }
    let ts = now_iso();
    let code = format!("term-{}", Local::now().timestamp_millis());
    conn.execute(
        "INSERT INTO asset_browse_term (parent_id, code, display_name, sort_order, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, 0, 'ACTIVE', ?4, ?4)",
        params![parent_id, &code, name, &ts],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(BrowseTermDto {
        id,
        parent_id: Some(parent_id),
        code,
        display_name: name.to_string(),
        sort_order: 0,
    })
}

#[tauri::command]
pub fn term_rename(db: State<'_, AssetDb>, term_id: i64, display_name: String) -> Result<(), String> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err("Folder name required".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let code: String = conn
        .query_row(
            "SELECT code FROM asset_browse_term WHERE id = ?1",
            params![term_id],
            |r| r.get(0),
        )
        .map_err(|_| "Term not found".to_string())?;
    if code == ROOT_CODE {
        return Err("Cannot rename root".into());
    }
    let ts = now_iso();
    conn.execute(
        "UPDATE asset_browse_term SET display_name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, &ts, term_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn term_move(db: State<'_, AssetDb>, term_id: i64, new_parent_id: i64) -> Result<(), String> {
    if term_id == new_parent_id {
        return Err("Cannot move folder into itself".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let code: String = conn
        .query_row(
            "SELECT code FROM asset_browse_term WHERE id = ?1",
            params![term_id],
            |r| r.get(0),
        )
        .map_err(|_| "Term not found".to_string())?;
    if code == ROOT_CODE {
        return Err("Cannot move root".into());
    }
    if is_descendant(&conn, term_id, new_parent_id)? {
        return Err("Cannot move folder into its descendant".into());
    }
    let depth = term_depth(&conn, new_parent_id)?;
    // moving term keeps its subtree; only check parent depth + 1 for the node itself
    if depth >= MAX_TERM_DEPTH {
        return Err(format!("Browse tree max depth is {MAX_TERM_DEPTH}"));
    }
    let ts = now_iso();
    conn.execute(
        "UPDATE asset_browse_term SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_parent_id, &ts, term_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn term_delete(db: State<'_, AssetDb>, term_id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let code: String = conn
        .query_row(
            "SELECT code FROM asset_browse_term WHERE id = ?1",
            params![term_id],
            |r| r.get(0),
        )
        .map_err(|_| "Term not found".to_string())?;
    if code == ROOT_CODE {
        return Err("Cannot delete root".into());
    }
    let child_terms: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM asset_browse_term WHERE parent_id = ?1",
            params![term_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if child_terms > 0 {
        return Err("Folder is not empty (has subfolders)".into());
    }
    let child_assets: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM asset_instance WHERE browse_term_id = ?1",
            params![term_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if child_assets > 0 {
        return Err("Folder is not empty (has assets)".into());
    }
    conn.execute("DELETE FROM asset_browse_term WHERE id = ?1", params![term_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
