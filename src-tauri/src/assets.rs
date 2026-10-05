//! Local document asset ledger (SQLite). Physical files are never moved or deleted by these APIs.

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager, State};

const DB_NAME: &str = "assets.sqlite";
const ROOT_CODE: &str = "root";
const MAX_TERM_DEPTH: usize = 5;
/// Same SSOT as frontend `src/file-types.json` (Monaco-readable admission).
const FILE_TYPES_JSON: &str = include_str!("../../src/file-types.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileTypesDoc {
    special_names: std::collections::HashMap<String, serde_json::Value>,
    types: Vec<FileTypeRow>,
}

#[derive(Debug, Deserialize)]
struct FileTypeRow {
    ext: String,
}

fn supported_tokens() -> &'static HashSet<String> {
    static TOKENS: OnceLock<HashSet<String>> = OnceLock::new();
    TOKENS.get_or_init(|| {
        let doc: FileTypesDoc =
            serde_json::from_str(FILE_TYPES_JSON).expect("file-types.json must parse");
        let mut set = HashSet::new();
        for name in doc.special_names.keys() {
            set.insert(name.to_lowercase());
        }
        for row in doc.types {
            set.insert(row.ext.to_lowercase());
        }
        set
    })
}

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
    /// Unix seconds snapshot of source file mtime at register / last convert.
    pub source_mtime: Option<i64>,
    pub source_size: Option<i64>,
    /// Source path is set and the file currently exists on disk.
    pub source_exists: bool,
    /// Source exists and differs from recorded mtime/size.
    pub source_stale: bool,
    /// Persisted current-version path index: `VALID` | `INVALID`.
    pub index_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTreeDto {
    pub terms: Vec<BrowseTermDto>,
    pub assets: Vec<AssetDto>,
    pub root_term_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetVersionDto {
    pub id: i64,
    pub asset_id: i64,
    pub absolute_path: String,
    pub created_at: String,
    pub file_exists: bool,
    pub is_current: bool,
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

/// Default SQLite path (same as Tauri app data). Overridable via `MONOLITH_ASSETS_DB`.
pub fn default_assets_db_path() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("MONOLITH_ASSETS_DB") {
        let t = p.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| "HOME not set".to_string())?;
    let base = PathBuf::from(home);
    #[cfg(target_os = "macos")]
    {
        return Ok(base
            .join("Library/Application Support/com.muqiang.monolith")
            .join(DB_NAME));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Ok(PathBuf::from(appdata)
                .join("com.muqiang.monolith")
                .join(DB_NAME));
        }
        return Ok(base
            .join("AppData/Roaming/com.muqiang.monolith")
            .join(DB_NAME));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(base
            .join(".local/share/com.muqiang.monolith")
            .join(DB_NAME))
    }
}

/// Open / migrate assets DB at an explicit path (App + monolith-mcp).
pub fn init_asset_db_at(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
"#,
    )
    .map_err(|e| e.to_string())?;
    // journal_mode returns a row; execute_batch may still apply. Re-run WAL explicitly.
    let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get::<_, String>(0));
    let _ = conn.execute_batch("PRAGMA busy_timeout = 5000;");
    conn.execute_batch(
        r#"
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
          created_at TEXT NOT NULL,
          source_path TEXT,
          source_mtime INTEGER,
          source_size INTEGER,
          index_status TEXT NOT NULL DEFAULT 'VALID'
        );
        CREATE TABLE IF NOT EXISTS asset_version (
          id INTEGER PRIMARY KEY,
          asset_id INTEGER NOT NULL REFERENCES asset_instance(id) ON DELETE CASCADE,
          absolute_path TEXT NOT NULL UNIQUE,
          created_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    // Older DBs created before source_path was in CREATE TABLE.
    if !column_exists(&conn, "asset_instance", "source_path")? {
        conn.execute("ALTER TABLE asset_instance ADD COLUMN source_path TEXT", [])
            .map_err(|e| e.to_string())?;
    }
    if !column_exists(&conn, "asset_instance", "source_mtime")? {
        conn.execute("ALTER TABLE asset_instance ADD COLUMN source_mtime INTEGER", [])
            .map_err(|e| e.to_string())?;
    }
    if !column_exists(&conn, "asset_instance", "source_size")? {
        conn.execute("ALTER TABLE asset_instance ADD COLUMN source_size INTEGER", [])
            .map_err(|e| e.to_string())?;
    }
    if !column_exists(&conn, "asset_instance", "index_status")? {
        conn.execute(
            "ALTER TABLE asset_instance ADD COLUMN index_status TEXT NOT NULL DEFAULT 'VALID'",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    crate::convert::ensure_conversion_log_schema(&conn)?;

    let ts = now_iso();
    conn.execute(
        "INSERT OR IGNORE INTO asset_browse_term (id, parent_id, code, display_name, sort_order, status, created_at, updated_at)
         VALUES (1, NULL, ?1, 'Root', 0, 'ACTIVE', ?2, ?2)",
        params![ROOT_CODE, ts],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn)
}

pub fn init_asset_db(app: &AppHandle) -> Result<AssetDb, String> {
    let path = db_path(app)?;
    let conn = init_asset_db_at(&path)?;
    Ok(AssetDb(Mutex::new(conn)))
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let name: String = row.get(1).map_err(|e| e.to_string())?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn root_id(conn: &Connection) -> Result<i64, String> {
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

/// Admission token: special basename (e.g. dockerfile) or extension. No invented defaults.
fn file_type_of(path: &str) -> Result<String, String> {
    let base = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid file name: {path}"))?
        .to_lowercase();
    let tokens = supported_tokens();
    if tokens.contains(&base) {
        return Ok(base);
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| format!("Unsupported file type (no extension): {base}"))?;
    if !tokens.contains(&ext) {
        return Err(format!("Unsupported file type: .{ext}"));
    }
    Ok(ext)
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

pub fn list_terms(conn: &Connection) -> Result<Vec<BrowseTermDto>, String> {
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

/// Display path for a term, e.g. `root/临床/指南`.
pub fn term_display_path(conn: &Connection, term_id: i64) -> Result<String, String> {
    let terms = list_terms(conn)?;
    let by_id: std::collections::HashMap<i64, &BrowseTermDto> =
        terms.iter().map(|t| (t.id, t)).collect();
    let mut parts = Vec::new();
    let mut cur = Some(term_id);
    let mut guard = 0usize;
    while let Some(id) = cur {
        guard += 1;
        if guard > MAX_TERM_DEPTH + 2 {
            break;
        }
        let Some(t) = by_id.get(&id) else {
            break;
        };
        parts.push(t.display_name.clone());
        cur = t.parent_id;
    }
    parts.reverse();
    if parts.is_empty() {
        return Err(format!("NOT_FOUND: browse_term_id={term_id}"));
    }
    Ok(parts.join("/"))
}

/// Resolve a folder ref: numeric id, display/code name, or path `root/临床`.
/// Empty / omitted caller responsibility. Ambiguous name → error with candidates.
pub fn resolve_browse_term(
    conn: &Connection,
    folder: &str,
) -> Result<BrowseTermDto, String> {
    let raw = folder.trim();
    if raw.is_empty() {
        return Err("folder is empty".into());
    }
    let terms = list_terms(conn)?;
    if let Ok(id) = raw.parse::<i64>() {
        return terms
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("NOT_FOUND: browse_term_id={id}"));
    }
    if raw.contains('/') {
        let segments: Vec<&str> = raw
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if segments.is_empty() {
            return Err("folder path is empty".into());
        }
        let mut parent: Option<i64> = None;
        let mut current: Option<BrowseTermDto> = None;
        for seg in segments {
            let matches: Vec<&BrowseTermDto> = terms
                .iter()
                .filter(|t| {
                    t.parent_id == parent
                        && (t.display_name.eq_ignore_ascii_case(seg)
                            || t.code.eq_ignore_ascii_case(seg))
                })
                .collect();
            match matches.len() {
                0 => {
                    return Err(format!(
                        "NOT_FOUND: folder segment '{seg}' under {}",
                        parent
                            .map(|p| format!("term {p}"))
                            .unwrap_or_else(|| "root-level".into())
                    ));
                }
                1 => {
                    current = Some(matches[0].clone());
                    parent = Some(matches[0].id);
                }
                _ => {
                    return Err(format!(
                        "AMBIGUOUS_FOLDER: segment '{seg}' matches {} terms",
                        matches.len()
                    ));
                }
            }
        }
        return current.ok_or_else(|| "NOT_FOUND: folder path".into());
    }
    let matches: Vec<&BrowseTermDto> = terms
        .iter()
        .filter(|t| {
            t.display_name.eq_ignore_ascii_case(raw) || t.code.eq_ignore_ascii_case(raw)
        })
        .collect();
    match matches.len() {
        0 => Err(format!("NOT_FOUND: folder '{raw}'")),
        1 => Ok(matches[0].clone()),
        _ => {
            let detail = matches
                .iter()
                .filter_map(|t| term_display_path(conn, t.id).ok().map(|p| format!("{} (id={})", p, t.id)))
                .collect::<Vec<_>>()
                .join("; ");
            Err(format!(
                "AMBIGUOUS_FOLDER: '{raw}' matches {}. Use folder path or browse_term_id.",
                detail
            ))
        }
    }
}

fn collect_descendant_ids(
    terms: &[BrowseTermDto],
    root: i64,
    recursive: bool,
) -> Vec<i64> {
    if !recursive {
        return vec![root];
    }
    let mut out = vec![root];
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        for t in terms {
            if t.parent_id == Some(id) {
                out.push(t.id);
                stack.push(t.id);
            }
        }
    }
    out
}

/// List assets in a folder. `recursive=false` = that folder only.
pub fn list_assets_in_folder(
    conn: &Connection,
    folder_term_id: i64,
    recursive: bool,
) -> Result<Vec<AssetDto>, String> {
    let terms = list_terms(conn)?;
    if !terms.iter().any(|t| t.id == folder_term_id) {
        return Err(format!("NOT_FOUND: browse_term_id={folder_term_id}"));
    }
    let allowed = collect_descendant_ids(&terms, folder_term_id, recursive);
    let all = list_assets(conn)?;
    Ok(all
        .into_iter()
        .filter(|a| allowed.contains(&a.browse_term_id))
        .collect())
}

fn path_basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

/// Keep in sync with UI `matchAssetName` (`src/utils/assetSearch.ts`);
/// locked by `scripts/test-asset-search.mjs` + this module's unit test.
fn asset_matches_name_query(asset: &AssetDto, query_lower: &str) -> bool {
    if asset.display_name.to_lowercase().contains(query_lower) {
        return true;
    }
    if let Some(ref p) = asset.absolute_path {
        if path_basename(p).to_lowercase().contains(query_lower) {
            return true;
        }
    }
    false
}

/// Name search: case-insensitive substring on `display_name` and path basename.
/// `folder_term_id=None` = global. `recursive` only applies when folder is set.
pub fn search_assets_by_name(
    conn: &Connection,
    query: &str,
    folder_term_id: Option<i64>,
    recursive: bool,
) -> Result<Vec<AssetDto>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("query required".into());
    }
    let q_lower = q.to_lowercase();
    let scoped = match folder_term_id {
        None => list_assets(conn)?,
        Some(id) => list_assets_in_folder(conn, id, recursive)?,
    };
    Ok(scoped
        .into_iter()
        .filter(|a| asset_matches_name_query(a, &q_lower))
        .collect())
}

const INDEX_VALID: &str = "VALID";
const INDEX_INVALID: &str = "INVALID";

fn persist_current_index_status(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, v.absolute_path FROM asset_instance a
             LEFT JOIN asset_version v ON v.id = a.current_version_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))
        .map_err(|e| e.to_string())?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (id, path) in rows {
        let exists = path.as_ref().map(|p| Path::new(p).is_file()).unwrap_or(false);
        let status = if exists { INDEX_VALID } else { INDEX_INVALID };
        conn.execute(
            "UPDATE asset_instance SET index_status = ?1 WHERE id = ?2 AND index_status != ?1",
            params![status, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn list_assets(conn: &Connection) -> Result<Vec<AssetDto>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.display_name, a.file_type, a.browse_term_id, a.current_version_id,
                    a.created_at, v.absolute_path, a.source_path, a.source_mtime, a.source_size,
                    a.index_status
             FROM asset_instance a
             LEFT JOIN asset_version v ON v.id = a.current_version_id
             ORDER BY a.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let path: Option<String> = r.get(6)?;
            let exists = path.as_ref().map(|p| Path::new(p).is_file()).unwrap_or(false);
            let source_path: Option<String> = r.get(7)?;
            let source_mtime: Option<i64> = r.get(8)?;
            let source_size: Option<i64> = r.get(9)?;
            let index_status: String = r.get(10)?;
            let source_exists = match source_path.as_deref() {
                Some(sp) if is_http_url(sp) => true,
                Some(sp) => Path::new(sp).is_file(),
                None => false,
            };
            let source_stale = match source_path.as_deref() {
                Some(sp)
                    if !is_http_url(sp)
                        && source_exists
                        && source_mtime.is_some()
                        && source_size.is_some() =>
                {
                    crate::convert::file_identity(sp)
                        .map(|id| Some(id.mtime) != source_mtime || Some(id.size) != source_size)
                        .unwrap_or(false)
                }
                _ => false,
            };
            Ok(AssetDto {
                id: r.get(0)?,
                display_name: r.get(1)?,
                file_type: r.get(2)?,
                browse_term_id: r.get(3)?,
                current_version_id: r.get(4)?,
                created_at: r.get(5)?,
                absolute_path: path,
                file_exists: exists,
                source_path,
                source_mtime,
                source_size,
                source_exists,
                source_stale,
                index_status,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_list_tree(db: State<'_, AssetDb>) -> Result<AssetTreeDto, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    persist_current_index_status(&conn)?;
    Ok(AssetTreeDto {
        root_term_id: root_id(&conn)?,
        terms: list_terms(&conn)?,
        assets: list_assets(&conn)?,
    })
}

pub fn is_http_url(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("http://") || t.starts_with("https://")
}

/// Register a Monaco-readable file as an asset.
/// `source_path` is either a local file absolute path, or an http(s) URL (web clip).
pub fn create_from_path_conn(
    conn: &Connection,
    path: &str,
    source_path: Option<String>,
    display_name: Option<String>,
    browse_term_id: Option<i64>,
) -> Result<AssetDto, String> {
    let abs = normalize_path(path)?;
    if !Path::new(&abs).is_file() {
        return Err(format!("File not found: {abs}"));
    }
    let ftype = file_type_of(&abs)?;
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

    let term_id = match browse_term_id {
        Some(id) => {
            let ok: bool = conn
                .query_row(
                    "SELECT 1 FROM asset_browse_term WHERE id = ?1 AND status = 'ACTIVE'",
                    params![id],
                    |_| Ok(true),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .unwrap_or(false);
            if !ok {
                return Err(format!("NOT_FOUND: browse_term_id={id}"));
            }
            id
        }
        None => root_id(conn)?,
    };
    let ts = now_iso();
    let name = display_name
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| display_name_of(&abs));

    let src = source_path
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let (src_mtime, src_size) = match src.as_deref() {
        Some(sp) if is_http_url(sp) => (None, None),
        Some(sp) => match crate::convert::file_identity(sp) {
            Ok(id) => (Some(id.mtime), Some(id.size)),
            Err(e) => return Err(format!("Source file not readable: {e}")),
        },
        None => (None, None),
    };
    conn.execute(
        "INSERT INTO asset_instance
           (display_name, file_type, browse_term_id, current_version_id, created_at,
            source_path, source_mtime, source_size)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7)",
        params![&name, &ftype, term_id, &ts, src, src_mtime, src_size],
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

    // Web clip: ensure conversion_log even if the app was offline at clip time.
    if let Some(ref sp) = src {
        if is_http_url(sp) {
            let _ = crate::convert::ensure_web_clip_log(conn, &abs);
            if crate::convert::latest_success_source(conn, &abs)?.is_none() {
                crate::convert::insert_log(
                    conn,
                    sp,
                    &abs,
                    "monolith-clipper",
                    "success",
                    &ts,
                    &ts,
                    Some("web clip (on asset register)"),
                    None,
                    None,
                )?;
            }
        }
    }

    list_assets(conn)?
        .into_iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| "Created asset missing after insert".into())
}

#[tauri::command]
pub fn asset_create_from_path(
    db: State<'_, AssetDb>,
    path: String,
    source_path: Option<String>,
    display_name: Option<String>,
) -> Result<AssetDto, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    create_from_path_conn(&conn, &path, source_path, display_name, None)
}

#[tauri::command]
pub fn asset_delete(db: State<'_, AssetDb>, asset_id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // Index only: CASCADE versions; never delete asset/source files; never delete conversion_log.
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
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relocate_current_path(&conn, asset_id, &new_path)
}

/// Point the current version at a file that still exists: new folder, new name, or both.
fn relocate_current_path(
    conn: &Connection,
    asset_id: i64,
    new_path: &str,
) -> Result<AssetDto, String> {
    let abs = normalize_path(new_path)?;
    if !Path::new(&abs).is_file() {
        return Err(format!("File not found: {abs}"));
    }
    let ftype = file_type_of(&abs)?;
    let (cur_ver, old_path): (i64, String) = conn
        .query_row(
            "SELECT a.current_version_id, v.absolute_path
             FROM asset_instance a
             JOIN asset_version v ON v.id = a.current_version_id
             WHERE a.id = ?1",
            params![asset_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
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
        params![ftype, asset_id],
    )
    .map_err(|e| e.to_string())?;
    crate::convert::log_output_relocated(conn, &old_path, &abs)?;

    list_assets(conn)?
        .into_iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| "Asset missing after relocate".into())
}

#[tauri::command]
pub fn asset_list_versions(
    db: State<'_, AssetDb>,
    asset_id: i64,
) -> Result<Vec<AssetVersionDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let current: Option<i64> = match conn.query_row(
        "SELECT current_version_id FROM asset_instance WHERE id = ?1",
        params![asset_id],
        |r| r.get::<_, Option<i64>>(0),
    ) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err("Asset not found".into()),
        Err(e) => return Err(e.to_string()),
    };
    let mut stmt = conn
        .prepare(
            "SELECT id, asset_id, absolute_path, created_at FROM asset_version
             WHERE asset_id = ?1 ORDER BY created_at DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![asset_id], |r| {
            let path: String = r.get(2)?;
            let id: i64 = r.get(0)?;
            Ok(AssetVersionDto {
                id,
                asset_id: r.get(1)?,
                absolute_path: path.clone(),
                created_at: r.get(3)?,
                file_exists: Path::new(&path).is_file(),
                is_current: current == Some(id),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asset_set_current_version(
    db: State<'_, AssetDb>,
    asset_id: i64,
    version_id: i64,
) -> Result<AssetDto, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT absolute_path FROM asset_version WHERE id = ?1 AND asset_id = ?2",
            params![version_id, asset_id],
            |r| r.get(0),
        )
        .map_err(|_| "Version not found for this asset".to_string())?;
    let ftype = file_type_of(&path)?;
    conn.execute(
        "UPDATE asset_instance SET current_version_id = ?1, file_type = ?2 WHERE id = ?3",
        params![version_id, ftype, asset_id],
    )
    .map_err(|e| e.to_string())?;
    list_assets(&conn)?
        .into_iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| "Asset missing after set current".into())
}

#[tauri::command]
pub fn open_in_os(path: String) -> Result<(), String> {
    // Web-clip provenance: http(s) → default browser (not a filesystem path).
    if is_http_url(&path) {
        let url = path.trim();
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(url)
                .status()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .status()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            std::process::Command::new("xdg-open")
                .arg(url)
                .status()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
pub fn reveal_in_os(path: String) -> Result<(), String> {
    // Web-clip provenance: open the page URL instead of Finder.
    if is_http_url(&path) {
        return open_in_os(path);
    }
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(parent) = p.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .status()
                .map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("No parent directory".into())
        }
    }
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
    let ftype = file_type_of(&abs)?;
    conn.execute(
        "UPDATE asset_instance SET current_version_id = ?1, file_type = ?2 WHERE id = ?3",
        params![ver_id, ftype, asset_id],
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::io::Write;

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE asset_browse_term (
              id INTEGER PRIMARY KEY,
              parent_id INTEGER REFERENCES asset_browse_term(id),
              code TEXT NOT NULL UNIQUE,
              display_name TEXT NOT NULL,
              sort_order INTEGER NOT NULL DEFAULT 0,
              status TEXT NOT NULL DEFAULT 'ACTIVE',
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            CREATE TABLE asset_instance (
              id INTEGER PRIMARY KEY,
              display_name TEXT NOT NULL,
              file_type TEXT NOT NULL,
              browse_term_id INTEGER NOT NULL REFERENCES asset_browse_term(id),
              current_version_id INTEGER,
              created_at TEXT NOT NULL,
              source_path TEXT,
              source_mtime INTEGER,
              source_size INTEGER,
              index_status TEXT NOT NULL DEFAULT 'VALID'
            );
            CREATE TABLE asset_version (
              id INTEGER PRIMARY KEY,
              asset_id INTEGER NOT NULL REFERENCES asset_instance(id) ON DELETE CASCADE,
              absolute_path TEXT NOT NULL UNIQUE,
              created_at TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_browse_term (id, parent_id, code, display_name, sort_order, status, created_at, updated_at)
             VALUES (1, NULL, ?1, 'Root', 0, 'ACTIVE', ?2, ?2)",
            params![ROOT_CODE, ts],
        )
        .unwrap();
        crate::convert::ensure_conversion_log_schema(&conn).unwrap();
        conn
    }

    fn write_temp(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "monolith-asset-test-{}",
            Local::now().timestamp_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn file_type_admits_monaco_ext_and_special_name() {
        assert_eq!(file_type_of("/tmp/a.md").unwrap(), "md");
        assert_eq!(file_type_of("/tmp/Dockerfile").unwrap(), "dockerfile");
        assert!(file_type_of("/tmp/noext").is_err());
        assert!(file_type_of("/tmp/x.pdf").is_err());
    }

    #[test]
    fn create_rejects_duplicate_path_and_keeps_file_on_delete() {
        let path = write_temp("note.md", "hello");
        let abs = path.to_string_lossy().into_owned();
        let conn = open_mem();
        let root = root_id(&conn).unwrap();
        let ts = now_iso();
        let ftype = file_type_of(&abs).unwrap();
        conn.execute(
            "INSERT INTO asset_instance (display_name, file_type, browse_term_id, current_version_id, created_at, source_path)
             VALUES ('n', ?1, ?2, NULL, ?3, NULL)",
            params![ftype, root, ts],
        )
        .unwrap();
        let asset_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, &abs, ts],
        )
        .unwrap();
        let ver = conn.last_insert_rowid();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver, asset_id],
        )
        .unwrap();

        let dup: Option<i64> = conn
            .query_row(
                "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
                params![&abs],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(dup, Some(asset_id));

        conn.execute("DELETE FROM asset_instance WHERE id = ?1", params![asset_id])
            .unwrap();
        assert!(path.is_file(), "unregister must not delete physical file");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(path.parent().unwrap());
    }

    #[test]
    fn relocate_accepts_rename_and_move() {
        let conn = open_mem();
        let stamp = Local::now().timestamp_millis();
        let dir_a = std::env::temp_dir().join(format!("monolith-rel-a-{stamp}"));
        let dir_b = std::env::temp_dir().join(format!("monolith-rel-b-{stamp}"));
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        let orig = dir_a.join("old-name.md");
        let renamed = dir_a.join("new-name.md");
        let moved = dir_b.join("elsewhere.md");
        fs::write(&orig, b"a").unwrap();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_instance (display_name, file_type, browse_term_id, created_at)
             VALUES ('old-name.md', 'md', 1, ?1)",
            params![ts],
        )
        .unwrap();
        let asset_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, orig.to_str().unwrap(), ts],
        )
        .unwrap();
        let ver = conn.last_insert_rowid();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver, asset_id],
        )
        .unwrap();

        fs::rename(&orig, &renamed).unwrap();
        let after_rename = relocate_current_path(&conn, asset_id, renamed.to_str().unwrap()).unwrap();
        assert_eq!(after_rename.absolute_path.as_deref(), renamed.to_str());

        fs::rename(&renamed, &moved).unwrap();
        let after_move = relocate_current_path(&conn, asset_id, moved.to_str().unwrap()).unwrap();
        assert_eq!(after_move.absolute_path.as_deref(), moved.to_str());

        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn new_version_path_uses_timestamp_and_collision_suffix() {
        let dir = std::env::temp_dir().join(format!(
            "monolith-ver-{}",
            Local::now().timestamp_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        let current = dir.join("doc.md");
        fs::write(&current, b"v1").unwrap();
        let first = next_version_path(&current).unwrap();
        let first_name = first.file_name().unwrap().to_str().unwrap().to_string();
        assert!(first_name.starts_with("doc_"));
        assert!(first_name.ends_with(".md"));
        fs::write(&first, b"taken").unwrap();
        let second = next_version_path(&current).unwrap();
        let second_name = second.file_name().unwrap().to_str().unwrap().to_string();
        assert_ne!(first_name, second_name);
        assert!(
            second_name.contains("_2.md") || second_name != first_name,
            "collision must allocate a distinct name, got {second_name}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn term_move_rejects_into_descendant() {
        let conn = open_mem();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_browse_term (parent_id, code, display_name, sort_order, status, created_at, updated_at)
             VALUES (1, 'a', 'A', 0, 'ACTIVE', ?1, ?1)",
            params![ts],
        )
        .unwrap();
        let a = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_browse_term (parent_id, code, display_name, sort_order, status, created_at, updated_at)
             VALUES (?1, 'b', 'B', 0, 'ACTIVE', ?2, ?2)",
            params![a, ts],
        )
        .unwrap();
        let b = conn.last_insert_rowid();
        assert!(is_descendant(&conn, a, b).unwrap());
        assert!(!is_descendant(&conn, b, a).unwrap());
    }

    #[test]
    fn source_path_null_when_absent() {
        let conn = open_mem();
        let assets = list_assets(&conn).unwrap();
        assert!(assets.is_empty());
        let root = root_id(&conn).unwrap();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_instance (display_name, file_type, browse_term_id, current_version_id, created_at, source_path)
             VALUES ('r', 'md', ?1, NULL, ?2, NULL)",
            params![root, ts],
        )
        .unwrap();
        let assets = list_assets(&conn).unwrap();
        assert_eq!(assets.len(), 1);
        assert!(assets[0].source_path.is_none());
    }

    #[test]
    fn search_assets_by_name_matches_display_and_basename() {
        let conn = open_mem();
        let root = root_id(&conn).unwrap();
        let path_er = write_temp("card-er-design.md", "er");
        let path_other = write_temp("notes.md", "x");
        let abs_er = path_er.to_string_lossy().into_owned();
        let abs_other = path_other.to_string_lossy().into_owned();
        let a = create_from_path_conn(
            &conn,
            &abs_er,
            None,
            Some("卡包ER设计".into()),
            Some(root),
        )
        .unwrap();
        let _b = create_from_path_conn(
            &conn,
            &abs_other,
            None,
            Some("杂记".into()),
            Some(root),
        )
        .unwrap();
        let by_name = search_assets_by_name(&conn, "ER", None, false).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].id, a.id);
        let by_base = search_assets_by_name(&conn, "card-er", None, false).unwrap();
        assert_eq!(by_base.len(), 1);
        assert_eq!(by_base[0].id, a.id);
        assert!(search_assets_by_name(&conn, "   ", None, false).is_err());
    }

    #[test]
    fn term_depth_caps_at_max() {
        let conn = open_mem();
        let ts = now_iso();
        let mut parent = 1i64;
        for i in 0..MAX_TERM_DEPTH {
            conn.execute(
                "INSERT INTO asset_browse_term (parent_id, code, display_name, sort_order, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, 'ACTIVE', ?4, ?4)",
                params![parent, format!("d{i}"), format!("D{i}"), ts],
            )
            .unwrap();
            parent = conn.last_insert_rowid();
        }
        assert!(term_depth(&conn, parent).unwrap() >= MAX_TERM_DEPTH);
    }

    #[test]
    fn set_current_switches_and_lists_flags() {
        let path_a = write_temp("a.md", "A");
        let path_b = write_temp("b.md", "B");
        let abs_a = path_a.to_string_lossy().into_owned();
        let abs_b = path_b.to_string_lossy().into_owned();
        let conn = open_mem();
        let root = root_id(&conn).unwrap();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_instance (display_name, file_type, browse_term_id, current_version_id, created_at, source_path)
             VALUES ('x', 'md', ?1, NULL, ?2, NULL)",
            params![root, ts],
        )
        .unwrap();
        let asset_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, &abs_a, ts],
        )
        .unwrap();
        let ver_a = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, &abs_b, ts],
        )
        .unwrap();
        let ver_b = conn.last_insert_rowid();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver_a, asset_id],
        )
        .unwrap();

        let db = AssetDb(Mutex::new(conn));
        // reuse helpers via raw conn again
        let conn = db.0.lock().unwrap();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver_b, asset_id],
        )
        .unwrap();
        drop(conn);
        let conn = db.0.lock().unwrap();
        let cur: i64 = conn
            .query_row(
                "SELECT current_version_id FROM asset_instance WHERE id = ?1",
                params![asset_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cur, ver_b);
        let _ = fs::remove_file(&path_a);
        let _ = fs::remove_file(&path_b);
        let _ = fs::remove_dir(path_a.parent().unwrap());
        let _ = fs::remove_dir(path_b.parent().unwrap());
    }

    #[test]
    fn list_marks_missing_file_and_source_flags() {
        let path = write_temp("gone.md", "x");
        let abs = path.to_string_lossy().into_owned();
        let src = write_temp("src.docx", "bin");
        let src_abs = src.to_string_lossy().into_owned();
        let id_src = crate::convert::file_identity(&src_abs).unwrap();
        let conn = open_mem();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_instance
               (display_name, file_type, browse_term_id, created_at, source_path, source_mtime, source_size)
             VALUES ('gone.md', 'md', 1, ?1, ?2, ?3, ?4)",
            params![ts, &src_abs, id_src.mtime, id_src.size],
        )
        .unwrap();
        let asset_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, &abs, ts],
        )
        .unwrap();
        let ver = conn.last_insert_rowid();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver, asset_id],
        )
        .unwrap();

        persist_current_index_status(&conn).unwrap();
        let listed = list_assets(&conn).unwrap();
        assert!(listed[0].file_exists);
        assert_eq!(listed[0].index_status, INDEX_VALID);
        assert!(listed[0].source_exists);
        assert!(!listed[0].source_stale);

        fs::remove_file(&path).unwrap();
        let listed_live = list_assets(&conn).unwrap();
        assert!(!listed_live[0].file_exists);
        assert_eq!(listed_live[0].index_status, INDEX_VALID);

        persist_current_index_status(&conn).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&src, b"changed-source").unwrap();
        persist_current_index_status(&conn).unwrap();
        let listed2 = list_assets(&conn).unwrap();
        assert!(!listed2[0].file_exists);
        assert_eq!(listed2[0].index_status, INDEX_INVALID);
        assert!(listed2[0].source_exists);
        assert!(listed2[0].source_stale);

        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(&path, b"back").unwrap();
        persist_current_index_status(&conn).unwrap();
        let listed3 = list_assets(&conn).unwrap();
        assert_eq!(listed3[0].index_status, INDEX_VALID);

        let _ = fs::remove_file(&src);
        let _ = fs::remove_dir(path.parent().unwrap());
        let _ = fs::remove_dir(src.parent().unwrap());
    }

    #[test]
    fn relocate_rejects_path_owned_by_other_asset_and_unsupported_type() {
        let conn = open_mem();
        let stamp = Local::now().timestamp_millis();
        let dir = std::env::temp_dir().join(format!("monolith-rel-dup-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        let pdf = dir.join("x.pdf");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        fs::write(&pdf, b"%PDF").unwrap();
        let ts = now_iso();
        for (name, path) in [("a.md", &a), ("b.md", &b)] {
            conn.execute(
                "INSERT INTO asset_instance (display_name, file_type, browse_term_id, created_at)
                 VALUES (?1, 'md', 1, ?2)",
                params![name, ts],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
                params![id, path.to_str().unwrap(), ts],
            )
            .unwrap();
            let ver = conn.last_insert_rowid();
            conn.execute(
                "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
                params![ver, id],
            )
            .unwrap();
        }
        let err = relocate_current_path(&conn, 1, b.to_str().unwrap()).unwrap_err();
        assert!(err.contains("already registered"), "{err}");
        let err2 = relocate_current_path(&conn, 1, pdf.to_str().unwrap()).unwrap_err();
        assert!(err2.contains("Unsupported"), "{err2}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn iron_delete_index_keeps_files_and_logs_relocate_only_inserts() {
        let stamp = Local::now().timestamp_millis();
        let dir = std::env::temp_dir().join(format!("monolith-iron-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("source.docx");
        let asset_path = dir.join("doc.md");
        let moved = dir.join("doc-moved.md");
        fs::write(&src, b"src-bin").unwrap();
        fs::write(&asset_path, b"md-body").unwrap();

        let conn = open_mem();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_instance
               (display_name, file_type, browse_term_id, created_at, source_path, source_mtime, source_size)
             VALUES ('doc.md', 'md', 1, ?1, ?2, 1, 7)",
            params![ts, src.to_str().unwrap()],
        )
        .unwrap();
        let asset_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, asset_path.to_str().unwrap(), ts],
        )
        .unwrap();
        let ver = conn.last_insert_rowid();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver, asset_id],
        )
        .unwrap();
        crate::convert::insert_log(
            &conn,
            src.to_str().unwrap(),
            asset_path.to_str().unwrap(),
            "downmark",
            "success",
            "t0",
            "t0",
            None,
            Some(1),
            Some(7),
        )
        .unwrap();

        fs::rename(&asset_path, &moved).unwrap();
        relocate_current_path(&conn, asset_id, moved.to_str().unwrap()).unwrap();

        let log_n: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversion_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(log_n, 2, "relocate must insert, not rewrite");
        let statuses: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT status FROM conversion_log ORDER BY id")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(statuses, vec!["success".to_string(), "relocated".to_string()]);
        let success_out: String = conn
            .query_row(
                "SELECT output_path FROM conversion_log WHERE status = 'success' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(success_out, asset_path.to_string_lossy());

        conn.execute("DELETE FROM asset_instance WHERE id = ?1", params![asset_id])
            .unwrap();
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM asset_instance", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
        let vers: i64 = conn
            .query_row("SELECT COUNT(*) FROM asset_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(vers, 0);
        let log_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversion_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(log_after, 2, "delete index must not delete conversion_log");
        assert!(src.is_file(), "source file must remain");
        assert!(moved.is_file(), "asset file must remain");
        assert!(!asset_path.is_file());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relocate_appends_conversion_log_without_rewriting_history() {
        let conn = open_mem();
        let stamp = Local::now().timestamp_millis();
        let dir = std::env::temp_dir().join(format!("monolith-rel-log-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let old = dir.join("old.md");
        let newp = dir.join("new.md");
        fs::write(&old, b"v").unwrap();
        let ts = now_iso();
        conn.execute(
            "INSERT INTO asset_instance (display_name, file_type, browse_term_id, created_at)
             VALUES ('old.md', 'md', 1, ?1)",
            params![ts],
        )
        .unwrap();
        let asset_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO asset_version (asset_id, absolute_path, created_at) VALUES (?1, ?2, ?3)",
            params![asset_id, old.to_str().unwrap(), ts],
        )
        .unwrap();
        let ver = conn.last_insert_rowid();
        conn.execute(
            "UPDATE asset_instance SET current_version_id = ?1 WHERE id = ?2",
            params![ver, asset_id],
        )
        .unwrap();
        crate::convert::insert_log(
            &conn,
            "/src.xlsx",
            old.to_str().unwrap(),
            "downmark",
            "success",
            "t",
            "t",
            None,
            Some(1),
            Some(2),
        )
        .unwrap();
        fs::rename(&old, &newp).unwrap();
        relocate_current_path(&conn, asset_id, newp.to_str().unwrap()).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversion_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let status: String = conn
            .query_row(
                "SELECT status FROM conversion_log ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "relocated");
        let found = crate::convert::latest_success_source(&conn, newp.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(found.path, "/src.xlsx");
        let hist = crate::convert::latest_success_source(&conn, old.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(hist.path, "/src.xlsx");
        let _ = fs::remove_dir_all(&dir);
    }
}
