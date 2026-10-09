//! Non-text → Markdown conversion (downmark) + conversion_log.

use crate::assets::AssetDb;
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

const CONVERT_TYPES_JSON: &str = include_str!("../../src/convert-types.json");

#[derive(Debug, Deserialize)]
struct ConvertTypesDoc {
    tools: HashMap<String, ToolExts>,
}

#[derive(Debug, Deserialize)]
struct ToolExts {
    extensions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConvertTool {
    Downmark,
}

impl ConvertTool {
    fn as_str(self) -> &'static str {
        match self {
            ConvertTool::Downmark => "downmark",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "downmark" => Some(ConvertTool::Downmark),
            _ => None,
        }
    }
}

fn ext_to_tool() -> &'static HashMap<String, ConvertTool> {
    static MAP: OnceLock<HashMap<String, ConvertTool>> = OnceLock::new();
    MAP.get_or_init(|| {
        let doc: ConvertTypesDoc =
            serde_json::from_str(CONVERT_TYPES_JSON).expect("convert-types.json must parse");
        let mut map = HashMap::new();
        for (tool_name, row) in doc.tools {
            let tool = ConvertTool::from_str(&tool_name)
                .unwrap_or_else(|| panic!("unknown tool in convert-types.json: {tool_name}"));
            for ext in row.extensions {
                let key = ext.to_lowercase();
                if let Some(prev) = map.insert(key.clone(), tool) {
                    panic!("extension {key} claimed by both {:?} and {:?}", prev, tool);
                }
            }
        }
        map
    })
}

pub fn tool_for_path(path: &str) -> Result<ConvertTool, String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| format!("Not a convertible path (no extension): {path}"))?;
    ext_to_tool()
        .get(&ext)
        .copied()
        .ok_or_else(|| format!("No converter registered for .{ext}"))
}

fn numbered_md_name(stem: &str, n: u32) -> String {
    if n <= 1 {
        format!("{stem}.md")
    } else {
        format!("{stem}-{n}.md")
    }
}

fn sources_equal(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();
    if a == b {
        return true;
    }
    if crate::assets::is_http_url(a) || crate::assets::is_http_url(b) {
        return false;
    }
    let ca = Path::new(a)
        .canonicalize()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    let cb = Path::new(b)
        .canonicalize()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    match (ca, cb) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

fn normalize_source_key(source: &str) -> String {
    let s = source.trim();
    if crate::assets::is_http_url(s) {
        return s.to_string();
    }
    Path::new(s)
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| s.to_string())
}

fn latest_logged_output(conn: &Connection, source: &str) -> Result<Option<String>, String> {
    let key = normalize_source_key(source);
    let raw = source.trim();
    let row: Option<String> = conn
        .query_row(
            "SELECT output_path FROM conversion_log
             WHERE status = 'success' AND (input_path = ?1 OR input_path = ?2)
             ORDER BY finished_at DESC, id DESC
             LIMIT 1",
            params![&key, raw],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(start) = row else {
        return Ok(None);
    };
    let mut cursor = start;
    for _ in 0..32 {
        let next: Option<String> = conn
            .query_row(
                "SELECT output_path FROM conversion_log
                 WHERE input_path = ?1 AND status = 'relocated'
                 ORDER BY finished_at DESC, id DESC
                 LIMIT 1",
                params![&cursor],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match next {
            Some(n) if n != cursor => cursor = n,
            _ => break,
        }
    }
    Ok(Some(cursor))
}

fn output_belongs_to_source(conn: &Connection, md_path: &Path, source: &str) -> bool {
    let abs = md_path
        .canonicalize()
        .unwrap_or_else(|_| md_path.to_path_buf());
    let abs_s = abs.to_string_lossy();
    let raw = md_path.to_string_lossy();
    if let Ok(Some(src)) = latest_success_source(conn, abs_s.as_ref()) {
        if sources_equal(&src.path, source) {
            return true;
        }
    } else if abs_s.as_ref() != raw.as_ref() {
        if let Ok(Some(src)) = latest_success_source(conn, raw.as_ref()) {
            if sources_equal(&src.path, source) {
                return true;
            }
        }
    }
    sidecar_source_url(md_path)
        .or_else(|| fs::read_to_string(md_path).ok().and_then(|c| source_url_from_markdown(&c)))
        .is_some_and(|u| sources_equal(&u, source))
}

fn uniquify_md_in_dir(dir: &Path, stem: &str, source: &str, conn: &Connection) -> PathBuf {
    let mut n = 1u32;
    loop {
        let p = dir.join(numbered_md_name(stem, n));
        if !p.exists() || output_belongs_to_source(conn, &p, source) {
            return p;
        }
        n = n.saturating_add(1);
        if n > 10_000 {
            return dir.join(format!("{stem}-{}.md", chrono::Local::now().timestamp_millis()));
        }
    }
}

/// Prefer `output_dir/{stem}.md` when dir is non-empty; else source-adjacent `{stem}.md`.
pub fn resolve_default_output(input: &str, convert_output_dir: &str) -> Result<String, String> {
    let p = Path::new(input);
    if !p.is_absolute() {
        return Err("Path must be absolute".into());
    }
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid file name: {input}"))?;
    let dir = convert_output_dir.trim();
    if !dir.is_empty() {
        return Ok(Path::new(dir)
            .join(format!("{stem}.md"))
            .to_string_lossy()
            .into_owned());
    }
    let parent = p
        .parent()
        .ok_or_else(|| format!("Invalid parent for: {input}"))?;
    Ok(parent.join(format!("{stem}.md")).to_string_lossy().into_owned())
}

/// Inbox target: same source overwrites last MD; other source with the same stem gets `-2`, `-3`, …
pub fn resolve_import_output(
    conn: &Connection,
    input: &str,
    convert_output_dir: &str,
) -> Result<String, String> {
    let p = Path::new(input);
    if !p.is_absolute() {
        return Err("Path must be absolute".into());
    }
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid file name: {input}"))?;
    let dir = convert_output_dir.trim();
    if dir.is_empty() {
        return resolve_default_output(input, dir);
    }
    if let Some(prev) = latest_logged_output(conn, input)? {
        let prev_p = Path::new(&prev);
        if !prev_p.exists() || output_belongs_to_source(conn, prev_p, input) {
            return Ok(prev);
        }
    }
    Ok(uniquify_md_in_dir(Path::new(dir), stem, input, conn)
        .to_string_lossy()
        .into_owned())
}

fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Snapshot of a file's identity for stale detection (path + mtime + size).
#[derive(Debug, Clone, Copy)]
pub struct FileIdentity {
    pub mtime: i64,
    pub size: i64,
}

pub fn file_identity(path: &str) -> Result<FileIdentity, String> {
    let meta = fs::metadata(path).map_err(|e| format!("stat {path}: {e}"))?;
    let size = meta.len() as i64;
    let mtime = meta
        .modified()
        .map_err(|e| format!("mtime {path}: {e}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;
    Ok(FileIdentity { mtime, size })
}

pub fn ensure_conversion_log_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS conversion_log (
          id INTEGER PRIMARY KEY,
          input_path TEXT NOT NULL,
          output_path TEXT NOT NULL,
          tool TEXT NOT NULL,
          status TEXT NOT NULL,
          started_at TEXT NOT NULL,
          finished_at TEXT NOT NULL,
          message TEXT,
          input_mtime INTEGER,
          input_size INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_conversion_log_output_finished
          ON conversion_log(output_path, finished_at DESC);
        "#,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_log(
    conn: &Connection,
    input_path: &str,
    output_path: &str,
    tool: &str,
    status: &str,
    started_at: &str,
    finished_at: &str,
    message: Option<&str>,
    input_mtime: Option<i64>,
    input_size: Option<i64>,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO conversion_log
           (input_path, output_path, tool, status, started_at, finished_at, message, input_mtime, input_size)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            input_path,
            output_path,
            tool,
            status,
            started_at,
            finished_at,
            message,
            input_mtime,
            input_size
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionSourceDto {
    pub path: String,
    pub recorded_mtime: Option<i64>,
    pub recorded_size: Option<i64>,
    pub current_mtime: Option<i64>,
    pub current_size: Option<i64>,
    pub exists: bool,
    /// True when source exists and mtime or size differs from the recorded snapshot.
    pub stale: bool,
}

fn enrich_source(
    path: String,
    recorded_mtime: Option<i64>,
    recorded_size: Option<i64>,
) -> ConversionSourceDto {
    enrich_source_for_output(path, recorded_mtime, recorded_size, None)
}

fn enrich_source_for_output(
    path: String,
    recorded_mtime: Option<i64>,
    recorded_size: Option<i64>,
    output_path: Option<&str>,
) -> ConversionSourceDto {
    if crate::assets::is_http_url(&path) {
        return ConversionSourceDto {
            path,
            recorded_mtime: None,
            recorded_size: None,
            current_mtime: None,
            current_size: None,
            exists: true,
            stale: false,
        };
    }
    let (exists, current_mtime, current_size) = match file_identity(&path) {
        Ok(id) => (true, Some(id.mtime), Some(id.size)),
        Err(_) => (false, None, None),
    };
    let track = crate::assets::should_track_source_stale(&path, output_path);
    let stale = track
        && exists
        && recorded_mtime.is_some()
        && recorded_size.is_some()
        && (current_mtime != recorded_mtime || current_size != recorded_size);
    ConversionSourceDto {
        path,
        recorded_mtime,
        recorded_size,
        current_mtime,
        current_size,
        exists,
        stale,
    }
}

/// Page URL from Markdown YAML frontmatter (`source` / `url`).
pub fn source_url_from_markdown(content: &str) -> Option<String> {
    let rest = content.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let fm = &rest[..end];
    for line in fm.lines() {
        let line = line.trim();
        for key in ["source:", "url:", "source_url:"] {
            if let Some(v) = line.strip_prefix(key) {
                let v = v.trim().trim_matches('"').trim_matches('\'');
                if v.starts_with("http://") || v.starts_with("https://") {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn sidecar_source_url(md_path: &Path) -> Option<String> {
    let parent = md_path.parent()?;
    let stem = md_path.file_stem()?.to_string_lossy();
    let p = parent.join(format!("{stem}.monolith-clip.json"));
    let raw = fs::read_to_string(&p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let s = v.get("source").and_then(|x| x.as_str())?.trim();
    if s.starts_with("http://") || s.starts_with("https://") {
        Some(s.to_string())
    } else {
        None
    }
}

fn abs_output_path(output_path: &str) -> String {
    let p = Path::new(output_path);
    if p.is_file() {
        p.canonicalize()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_else(|_| output_path.to_string())
    } else {
        output_path.to_string()
    }
}

/// If no conversion_log yet, materialize one from sidecar / frontmatter (Monolith may
/// have been offline when the browser extension clipped the page).
pub fn ensure_web_clip_log(
    conn: &Connection,
    output_path: &str,
) -> Result<Option<ConversionSourceDto>, String> {
    let abs = abs_output_path(output_path);
    if let Some(src) = latest_success_source(conn, &abs)? {
        return Ok(Some(src));
    }
    if abs != output_path {
        if let Some(src) = latest_success_source(conn, output_path)? {
            return Ok(Some(src));
        }
    }

    let md = Path::new(&abs);
    let url = sidecar_source_url(md)
        .or_else(|| fs::read_to_string(md).ok().and_then(|c| source_url_from_markdown(&c)));
    let Some(url) = url else {
        return Ok(None);
    };

    let ts = now_iso();
    insert_log(
        conn,
        &url,
        &abs,
        "monolith-clipper",
        "success",
        &ts,
        &ts,
        Some("web clip (materialized on open; app may have been offline at clip time)"),
        None,
        None,
    )?;
    latest_success_source(conn, &abs)
}

/// Latest successful conversion source for an output MD path (C3), with freshness.
/// Follows append-only `relocated` events when the current path has no direct success row.
pub fn latest_success_source(
    conn: &Connection,
    output_path: &str,
) -> Result<Option<ConversionSourceDto>, String> {
    let mut cursor = output_path.to_string();
    for _ in 0..32 {
        let row: Option<(String, Option<i64>, Option<i64>)> = conn
            .query_row(
                "SELECT input_path, input_mtime, input_size FROM conversion_log
                 WHERE output_path = ?1 AND status = 'success'
                 ORDER BY finished_at DESC, id DESC
                 LIMIT 1",
                params![&cursor],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((path, mtime, size)) = row {
            return Ok(Some(enrich_source_for_output(
                path,
                mtime,
                size,
                Some(output_path),
            )));
        }
        let prev: Option<String> = conn
            .query_row(
                "SELECT input_path FROM conversion_log
                 WHERE output_path = ?1 AND status = 'relocated'
                 ORDER BY finished_at DESC, id DESC
                 LIMIT 1",
                params![&cursor],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(prev) = prev else {
            return Ok(None);
        };
        if prev == cursor {
            return Ok(None);
        }
        cursor = prev;
    }
    Ok(None)
}

/// Append-only index relocate event: old output path → new output path. Never updates prior rows.
pub fn log_output_relocated(
    conn: &Connection,
    old_output_path: &str,
    new_output_path: &str,
) -> Result<i64, String> {
    let ts = now_iso();
    insert_log(
        conn,
        old_output_path,
        new_output_path,
        "relocate",
        "relocated",
        &ts,
        &ts,
        Some(&format!("{old_output_path} -> {new_output_path}")),
        None,
        None,
    )
}

pub struct ConvertRuntime {
    pub(crate) child: Mutex<Option<Child>>,
    pub(crate) cancel: AtomicBool,
    pub(crate) busy: AtomicBool,
}

impl Default for ConvertRuntime {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            cancel: AtomicBool::new(false),
            busy: AtomicBool::new(false),
        }
    }
}

fn resolve_tool_bin(app: &AppHandle, tool: ConvertTool) -> Result<PathBuf, String> {
    let name = tool.as_str();
    if let Ok(override_path) = std::env::var("MONOLITH_DOWNMARK_BIN") {
        let p = PathBuf::from(override_path);
        if p.is_file() {
            return Ok(p);
        }
        return Err(format!("MONOLITH_DOWNMARK_BIN is not a file: {}", p.display()));
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    let push_pair = |cands: &mut Vec<PathBuf>, base: PathBuf| {
        cands.push(base.join("run"));
        cands.push(base.join("downmark"));
        #[cfg(windows)]
        cands.push(base.join("downmark.exe"));
    };
    if let Ok(res) = app.path().resource_dir() {
        push_pair(&mut candidates, res.join("third-party").join(name));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_pair(&mut candidates, dir.join("third-party").join(name));
            push_pair(
                &mut candidates,
                dir.join("../Resources/third-party").join(name),
            );
        }
    }
    // Dev: workspace third-party next to src-tauri
    push_pair(
        &mut candidates,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("third-party")
            .join(name),
    );

    for c in &candidates {
        if c.is_file() {
            return Ok(c.canonicalize().unwrap_or_else(|_| c.clone()));
        }
    }
    Err(format!(
        "Converter '{name}' not found. Run scripts/install-converters.sh (checked {} candidates).",
        candidates.len()
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResultDto {
    pub output_path: String,
    pub tool: String,
    pub status: String,
    pub log_id: i64,
}

#[tauri::command]
pub fn convert_tool_for_path(path: String) -> Result<String, String> {
    Ok(tool_for_path(&path)?.as_str().to_string())
}

#[tauri::command]
pub fn convert_is_supported(path: String) -> Result<bool, String> {
    Ok(tool_for_path(&path).is_ok())
}

#[tauri::command]
pub fn convert_default_output(db: State<'_, AssetDb>, path: String) -> Result<String, String> {
    let dir = crate::settings::load_settings()
        .map(|s| s.inbox_dir)
        .unwrap_or_default();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    resolve_import_output(&conn, &path, &dir)
}

#[tauri::command]
pub fn conversion_latest_source(
    db: State<'_, AssetDb>,
    output_path: String,
) -> Result<Option<ConversionSourceDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    ensure_web_clip_log(&conn, &output_path)
}

#[tauri::command]
pub fn convert_cancel(rt: State<'_, ConvertRuntime>) -> Result<(), String> {
    rt.cancel.store(true, Ordering::SeqCst);
    if let Ok(mut g) = rt.child.lock() {
        if let Some(child) = g.as_mut() {
            let _ = child.kill();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn convert_run(
    app: AppHandle,
    db: State<'_, AssetDb>,
    rt: State<'_, ConvertRuntime>,
    input_path: String,
    output_path: String,
) -> Result<ConvertResultDto, String> {
    if rt
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A conversion is already running".into());
    }
    rt.cancel.store(false, Ordering::SeqCst);
    let result = run_convert_inner(&app, &db, &rt, &input_path, &output_path);
    {
        let mut g = rt.child.lock().map_err(|e| e.to_string())?;
        *g = None;
    }
    rt.busy.store(false, Ordering::SeqCst);
    result
}

pub(crate) fn run_convert_inner(
    app: &AppHandle,
    db: &AssetDb,
    rt: &ConvertRuntime,
    input_path: &str,
    output_path: &str,
) -> Result<ConvertResultDto, String> {
    let tool = tool_for_path(input_path)?;
    let input = Path::new(input_path);
    if !input.is_file() {
        return Err(format!("Input file not found: {input_path}"));
    }
    if !Path::new(output_path).is_absolute() {
        return Err("Output path must be absolute".into());
    }
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let bin = resolve_tool_bin(app, tool)?;
    let started = now_iso();
    let _ = app.emit(
        "convert-progress",
        serde_json::json!({ "phase": "start", "tool": tool.as_str(), "message": bin.display().to_string() }),
    );

    struct TmpHtml(PathBuf);
    impl Drop for TmpHtml {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let alt_html = crate::docx_altchunk::unwrap_docx_for_convert(input)?;
    let tmp_html = if let Some(html) = alt_html {
        let p = std::env::temp_dir().join(format!(
            "monolith-altchunk-{}-{}.html",
            std::process::id(),
            Local::now().timestamp_millis()
        ));
        fs::write(&p, html).map_err(|e| e.to_string())?;
        Some(TmpHtml(p))
    } else {
        None
    };
    let downmark_input = tmp_html
        .as_ref()
        .map(|t| t.0.to_string_lossy().into_owned())
        .unwrap_or_else(|| input_path.to_string());

    let mut cmd = Command::new(&bin);
    // Wrapper `run` takes <input> <output>; raw `downmark` uses -o <output> <input>.
    let is_wrapper = bin
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == "run");
    if is_wrapper {
        cmd.arg(&downmark_input).arg(output_path);
    } else {
        cmd.arg("-o").arg(output_path).arg(&downmark_input);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Explicit stub only — never implicit fallback.
    if std::env::var("MONOLITH_CONVERT_STUB").ok().as_deref() == Some("1") {
        cmd.env("MONOLITH_CONVERT_STUB", "1");
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn {}: {e}", bin.display()))?;
    if let Some(stdout) = child.stdout.take() {
        let app_c = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let _ = app_c.emit(
                    "convert-progress",
                    serde_json::json!({ "phase": "stdout", "message": line }),
                );
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app_c = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                let _ = app_c.emit(
                    "convert-progress",
                    serde_json::json!({ "phase": "stderr", "message": line }),
                );
            }
        });
    }

    {
        let mut g = rt.child.lock().map_err(|e| e.to_string())?;
        *g = Some(child);
    }

    let status = loop {
        if rt.cancel.load(Ordering::SeqCst) {
            if let Ok(mut g) = rt.child.lock() {
                if let Some(c) = g.as_mut() {
                    let _ = c.kill();
                }
            }
            break None;
        }
        let mut g = rt.child.lock().map_err(|e| e.to_string())?;
        let child = g.as_mut().ok_or_else(|| "Converter process lost".to_string())?;
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                drop(g);
                let _ = app.emit(
                    "convert-progress",
                    serde_json::json!({ "phase": "running", "tool": tool.as_str() }),
                );
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.to_string()),
        }
    };

    let finished = now_iso();
    let (log_status, message, ok) = if status.is_none() || rt.cancel.load(Ordering::SeqCst) {
        ("cancelled", Some("cancelled by user".to_string()), false)
    } else if let Some(st) = status {
        if st.success() {
            if !Path::new(output_path).is_file() {
                (
                    "failed",
                    Some("Converter exited 0 but output file missing".into()),
                    false,
                )
            } else if crate::docx_altchunk::markdown_is_blank(Path::new(output_path)) {
                let _ = fs::remove_file(output_path);
                (
                    "failed",
                    Some("Converter produced empty Markdown".into()),
                    false,
                )
            } else {
                ("success", None, true)
            }
        } else {
            (
                "failed",
                Some(format!("Converter exited with status {st}")),
                false,
            )
        }
    } else {
        ("failed", Some("unknown converter state".into()), false)
    };

    let input_id = file_identity(input_path).ok();
    let log_id = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        insert_log(
            &conn,
            input_path,
            output_path,
            tool.as_str(),
            log_status,
            &started,
            &finished,
            message.as_deref(),
            input_id.map(|i| i.mtime),
            input_id.map(|i| i.size),
        )?
    };

    if ok {
        let id = input_id.ok_or_else(|| format!("stat {input_path} after convert"))?;
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE asset_instance SET source_path = ?1, source_mtime = ?2, source_size = ?3
             WHERE id IN (
               SELECT asset_id FROM asset_version WHERE absolute_path = ?4
             )",
            params![input_path, id.mtime, id.size, output_path],
        )
        .map_err(|e| e.to_string())?;
    }

    let _ = app.emit(
        "convert-progress",
        serde_json::json!({
            "phase": "done",
            "status": log_status,
            "tool": tool.as_str(),
            "message": message.clone().unwrap_or_default()
        }),
    );

    if !ok {
        return Err(message.unwrap_or_else(|| log_status.to_string()));
    }

    Ok(ConvertResultDto {
        output_path: output_path.to_string(),
        tool: tool.as_str().to_string(),
        status: log_status.to_string(),
        log_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_conversion_log_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn parses_source_url_from_frontmatter() {
        let md = "---\ntitle: Hi\nsource: https://example.com/a\n---\n\nbody\n";
        assert_eq!(
            source_url_from_markdown(md).as_deref(),
            Some("https://example.com/a")
        );
    }

    #[test]
    fn materializes_web_clip_log_on_open() {
        let dir = tempfile_dir();
        let md_path = dir.join("clip.md");
        fs::write(
            &md_path,
            "---\nsource: \"https://example.com/clip\"\n---\n\nhello\n",
        )
        .unwrap();
        let conn = open_mem();
        let src = ensure_web_clip_log(&conn, md_path.to_str().unwrap())
            .unwrap()
            .expect("source");
        assert_eq!(src.path, "https://example.com/clip");
        assert!(src.exists);
        // Idempotent: second call does not fail / still returns URL.
        let again = ensure_web_clip_log(&conn, md_path.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(again.path, "https://example.com/clip");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversion_log WHERE tool = 'monolith-clipper'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    fn tempfile_dir() -> PathBuf {
        tempfile_dir_named("clip")
    }

    fn tempfile_dir_named(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "monolith-{}-{}-{}",
            tag,
            std::process::id(),
            Local::now().timestamp_millis()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn routes_all_convertible_to_downmark() {
        assert_eq!(tool_for_path("/tmp/a.pdf").unwrap(), ConvertTool::Downmark);
        assert_eq!(tool_for_path("/tmp/a.docx").unwrap(), ConvertTool::Downmark);
        assert_eq!(tool_for_path("/tmp/a.PPTX").unwrap(), ConvertTool::Downmark);
        assert_eq!(tool_for_path("/tmp/a.xlsx").unwrap(), ConvertTool::Downmark);
        assert_eq!(tool_for_path("/tmp/a.html").unwrap(), ConvertTool::Downmark);
    }

    #[test]
    fn rejects_unknown_and_image_without_ocr() {
        assert!(tool_for_path("/tmp/a.xyz").is_err());
        assert!(tool_for_path("/tmp/noext").is_err());
        // Images need OCR — out of scope for the small downmark binary.
        assert!(tool_for_path("/tmp/a.png").is_err());
        assert!(tool_for_path("/tmp/a.jpeg").is_err());
    }

    #[test]
    fn default_output_same_dir_stem_md() {
        assert_eq!(
            resolve_default_output("/data/report.pdf", "").unwrap(),
            "/data/report.md"
        );
    }

    #[test]
    fn default_output_uses_import_dir_when_set() {
        assert_eq!(
            resolve_default_output("/data/report.pdf", "/out/md").unwrap(),
            "/out/md/report.md"
        );
        assert_eq!(
            resolve_default_output("/data/report.pdf", "  ").unwrap(),
            "/data/report.md"
        );
    }

    #[test]
    fn import_same_source_overwrites_last_md() {
        let conn = open_mem();
        let dir = tempfile_dir_named("same-src");
        let inbox = dir.join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let src = dir.join("skill.pdf");
        fs::write(&src, b"pdf").unwrap();
        let md = inbox.join("skill.md");
        fs::write(&md, b"old").unwrap();
        insert_log(
            &conn,
            src.to_str().unwrap(),
            md.to_str().unwrap(),
            "downmark",
            "success",
            "t1",
            "t1",
            None,
            Some(1),
            Some(1),
        )
        .unwrap();
        let out = resolve_import_output(&conn, src.to_str().unwrap(), inbox.to_str().unwrap()).unwrap();
        assert_eq!(Path::new(&out), md.as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_other_source_same_stem_gets_dash_2() {
        let conn = open_mem();
        let dir = tempfile_dir_named("other-src");
        let inbox = dir.join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let a = dir.join("repo-a").join("skill.pdf");
        let b = dir.join("repo-b").join("skill.pdf");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let md = inbox.join("skill.md");
        fs::write(&md, b"from-a").unwrap();
        insert_log(
            &conn,
            a.to_str().unwrap(),
            md.to_str().unwrap(),
            "downmark",
            "success",
            "t1",
            "t1",
            None,
            Some(1),
            Some(1),
        )
        .unwrap();
        let out = resolve_import_output(&conn, b.to_str().unwrap(), inbox.to_str().unwrap()).unwrap();
        assert_eq!(Path::new(&out), inbox.join("skill-2.md").as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_follows_relocate_then_overwrites() {
        let conn = open_mem();
        let dir = tempfile_dir_named("reloc");
        let inbox = dir.join("inbox");
        let elsewhere = dir.join("kept");
        fs::create_dir_all(&inbox).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        let src = dir.join("doc.docx");
        fs::write(&src, b"x").unwrap();
        let orig = inbox.join("doc.md");
        let moved = elsewhere.join("doc.md");
        fs::write(&moved, b"body").unwrap();
        insert_log(
            &conn,
            src.to_str().unwrap(),
            orig.to_str().unwrap(),
            "downmark",
            "success",
            "t1",
            "t1",
            None,
            Some(1),
            Some(1),
        )
        .unwrap();
        log_output_relocated(&conn, orig.to_str().unwrap(), moved.to_str().unwrap()).unwrap();
        let out = resolve_import_output(&conn, src.to_str().unwrap(), inbox.to_str().unwrap()).unwrap();
        assert_eq!(Path::new(&out), moved.as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_recreates_deleted_last_md_at_same_path() {
        let conn = open_mem();
        let dir = tempfile_dir_named("deleted");
        let inbox = dir.join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let src = dir.join("gone.pdf");
        fs::write(&src, b"pdf").unwrap();
        let md = inbox.join("gone.md");
        insert_log(
            &conn,
            src.to_str().unwrap(),
            md.to_str().unwrap(),
            "downmark",
            "success",
            "t1",
            "t1",
            None,
            Some(1),
            Some(1),
        )
        .unwrap();
        assert!(!md.exists());
        let out = resolve_import_output(&conn, src.to_str().unwrap(), inbox.to_str().unwrap()).unwrap();
        assert_eq!(Path::new(&out), md.as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_unknown_occupant_uniquifies() {
        let conn = open_mem();
        let dir = tempfile_dir_named("unknown");
        let inbox = dir.join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let src = dir.join("skill.pdf");
        fs::write(&src, b"pdf").unwrap();
        fs::write(inbox.join("skill.md"), b"orphan").unwrap();
        fs::write(inbox.join("skill-2.md"), b"also").unwrap();
        let out = resolve_import_output(&conn, src.to_str().unwrap(), inbox.to_str().unwrap()).unwrap();
        assert_eq!(Path::new(&out), inbox.join("skill-3.md").as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_url_source_from_sidecar_counts_as_same() {
        let conn = open_mem();
        let dir = tempfile_dir_named("sidecar");
        let inbox = dir.join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let md = inbox.join("page.md");
        fs::write(&md, "---\nsource: https://example.com/x\n---\n\nbody\n").unwrap();
        fs::write(
            inbox.join("page.monolith-clip.json"),
            r#"{"source":"https://example.com/x"}"#,
        )
        .unwrap();
        let fake_src = dir.join("page.pdf");
        fs::write(&fake_src, b"x").unwrap();
        // PDF convert should not steal the clip's page.md
        let out = resolve_import_output(&conn, fake_src.to_str().unwrap(), inbox.to_str().unwrap())
            .unwrap();
        assert_eq!(Path::new(&out), inbox.join("page-2.md").as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_success_ignores_failed_and_cancelled() {
        let conn = open_mem();
        insert_log(
            &conn,
            "/in.pdf",
            "/out.md",
            "downmark",
            "failed",
            "2026-01-01T10:00:00",
            "2026-01-01T10:01:00",
            Some("boom"),
            Some(1),
            Some(10),
        )
        .unwrap();
        insert_log(
            &conn,
            "/old.pdf",
            "/out.md",
            "downmark",
            "success",
            "2026-01-01T11:00:00",
            "2026-01-01T11:01:00",
            None,
            Some(2),
            Some(20),
        )
        .unwrap();
        insert_log(
            &conn,
            "/new.pdf",
            "/out.md",
            "downmark",
            "success",
            "2026-01-01T12:00:00",
            "2026-01-01T12:01:00",
            None,
            Some(3),
            Some(30),
        )
        .unwrap();
        insert_log(
            &conn,
            "/cancel.pdf",
            "/out.md",
            "downmark",
            "cancelled",
            "2026-01-01T13:00:00",
            "2026-01-01T13:01:00",
            Some("cancelled"),
            Some(4),
            Some(40),
        )
        .unwrap();
        let src = latest_success_source(&conn, "/out.md").unwrap().unwrap();
        assert_eq!(src.path, "/new.pdf");
        assert_eq!(src.recorded_mtime, Some(3));
        assert_eq!(src.recorded_size, Some(30));
    }

    #[test]
    fn append_keeps_history_rows() {
        let conn = open_mem();
        insert_log(
            &conn,
            "/a.pdf",
            "/out.md",
            "downmark",
            "success",
            "t1",
            "t1",
            None,
            Some(1),
            Some(1),
        )
        .unwrap();
        insert_log(
            &conn,
            "/b.pdf",
            "/out.md",
            "downmark",
            "success",
            "t2",
            "t2",
            None,
            Some(2),
            Some(2),
        )
        .unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversion_log WHERE output_path = ?1",
                params!["/out.md"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn enrich_marks_stale_when_size_or_mtime_changes() {
        let dir = std::env::temp_dir().join(format!(
            "monolith-stale-{}",
            Local::now().timestamp_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("src.pdf");
        fs::write(&path, b"v1").unwrap();
        let id1 = file_identity(path.to_str().unwrap()).unwrap();
        let fresh = enrich_source(
            path.to_string_lossy().into_owned(),
            Some(id1.mtime),
            Some(id1.size),
        );
        assert!(fresh.exists);
        assert!(!fresh.stale);
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(&path, b"v2-longer").unwrap();
        let stale = enrich_source(
            path.to_string_lossy().into_owned(),
            Some(id1.mtime),
            Some(id1.size),
        );
        assert!(stale.stale);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enrich_md_to_md_never_stale() {
        let dir = std::env::temp_dir().join(format!(
            "monolith-md-stale-{}",
            Local::now().timestamp_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("SKILL.md");
        let out = dir.join("copy.md");
        fs::write(&src, b"# v1").unwrap();
        fs::write(&out, b"# copy").unwrap();
        let id1 = file_identity(src.to_str().unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(&src, b"# v2 changed").unwrap();
        let dto = enrich_source_for_output(
            src.to_string_lossy().into_owned(),
            Some(id1.mtime),
            Some(id1.size),
            Some(out.to_str().unwrap()),
        );
        assert!(dto.exists);
        assert!(!dto.stale);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relocate_appends_log_and_resolves_via_chain() {
        let conn = open_mem();
        insert_log(
            &conn,
            "/src.docx",
            "/orig.md",
            "downmark",
            "success",
            "t1",
            "t1",
            None,
            Some(10),
            Some(20),
        )
        .unwrap();
        log_output_relocated(&conn, "/orig.md", "/elsewhere.md").unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversion_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let hist = latest_success_source(&conn, "/orig.md").unwrap().unwrap();
        assert_eq!(hist.path, "/src.docx");
        let found = latest_success_source(&conn, "/elsewhere.md").unwrap().unwrap();
        assert_eq!(found.path, "/src.docx");
        assert_eq!(found.recorded_mtime, Some(10));
    }
}
