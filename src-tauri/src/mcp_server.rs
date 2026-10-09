//! Minimal MCP stdio server for Monolith assets (tools/list + tools/call).
//! Folder-first: omit folder = global (list/search) or root (register); with folder = scoped.

use crate::assets::{
    create_from_path_conn, default_assets_db_path, init_asset_db_at, list_assets,
    list_assets_in_folder, list_terms, resolve_browse_term, root_id, search_assets_by_name,
    term_display_path, AssetDto,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolDef {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

/// Name + description for Agent system prompt / UI (no full schemas).
pub fn tools_for_agent() -> Vec<(&'static str, &'static str)> {
    tools()
        .into_iter()
        .map(|t| (t.name, t.description))
        .collect()
}

fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "asset_list_tree",
            description: "Global overview: browse terms (folders) + all assets. Also returns assetsDir / inboxDir from Monolith settings (use assetsDir when creating new files before asset_register). Prefer asset_list_folder when the user names a folder.",
            input_schema: json!({"type":"object","properties":{}}),
        },
        ToolDef {
            name: "asset_list_folder",
            description: "List assets. Omit folder = global search/list. With folder (name, path like root/临床, or browse_term_id) = scoped. recursive default false.",
            input_schema: json!({
                "type":"object",
                "properties":{
                    "folder":{"type":"string","description":"Folder name, path (root/…), or numeric browse_term_id. Omit for global."},
                    "browse_term_id":{"type":"integer"},
                    "recursive":{"type":"boolean","description":"Include subfolders. Default false."}
                }
            }),
        },
        ToolDef {
            name: "asset_search",
            description: "Search assets. mode=name (default): substring on remark/displayName, file basename, and mount/ancestor folder names. mode=fulltext: not implemented (returns implemented:false; do not treat as name search). Optional folder scopes results.",
            input_schema: json!({
                "type":"object",
                "required":["query"],
                "properties":{
                    "query":{"type":"string","description":"Search string (required)"},
                    "mode":{"type":"string","description":"name | fulltext. Default name."},
                    "folder":{"type":"string"},
                    "browse_term_id":{"type":"integer"},
                    "recursive":{"type":"boolean","description":"When folder set, include subfolders. Default false."}
                }
            }),
        },
        ToolDef {
            name: "asset_get",
            description: "Get one asset by asset_id, absolute path, or display name. Optional folder scopes name lookup.",
            input_schema: json!({
                "type":"object",
                "properties":{
                    "asset_id":{"type":"integer"},
                    "path":{"type":"string"},
                    "name":{"type":"string","description":"display_name match"},
                    "folder":{"type":"string"},
                    "browse_term_id":{"type":"integer"},
                    "recursive":{"type":"boolean","description":"When resolving by name in a folder, include subfolders. Default true."}
                }
            }),
        },
        ToolDef {
            name: "asset_read_content",
            description: "Read current version file content (UTF-8). Resolve via asset_id / path / name (+ optional folder).",
            input_schema: json!({
                "type":"object",
                "properties":{
                    "asset_id":{"type":"integer"},
                    "path":{"type":"string"},
                    "name":{"type":"string"},
                    "folder":{"type":"string"},
                    "browse_term_id":{"type":"integer"},
                    "recursive":{"type":"boolean"},
                    "max_bytes":{"type":"integer"}
                }
            }),
        },
        ToolDef {
            name: "asset_register",
            description: "Register an existing local text file. Optional folder / browse_term_id; omit folder → root. Does not move the file. Optional source_path = local path or http(s) URL.",
            input_schema: json!({
                "type":"object",
                "required":["path"],
                "properties":{
                    "path":{"type":"string"},
                    "source_path":{"type":"string"},
                    "display_name":{"type":"string"},
                    "folder":{"type":"string"},
                    "browse_term_id":{"type":"integer"}
                }
            }),
        },
        ToolDef {
            name: "asset_write_content",
            description: "Overwrite current version file (Ctrl+S). Does NOT create a new version or move the asset's folder. Resolve via asset_id / path / name (+ optional folder).",
            input_schema: json!({
                "type":"object",
                "required":["content"],
                "properties":{
                    "asset_id":{"type":"integer"},
                    "path":{"type":"string"},
                    "name":{"type":"string"},
                    "folder":{"type":"string"},
                    "browse_term_id":{"type":"integer"},
                    "recursive":{"type":"boolean"},
                    "content":{"type":"string"},
                    "expected_mtime":{"type":"integer"}
                }
            }),
        },
        ToolDef {
            name: "asset_delete",
            description: "Delete asset INDEX only (L7). Does NOT delete the disk file or conversion_log.",
            input_schema: json!({
                "type":"object",
                "required":["asset_id"],
                "properties":{"asset_id":{"type":"integer"}}
            }),
        },
    ]
}

fn file_mtime_secs(path: &str) -> Option<i64> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    Some(
        modified
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs() as i64,
    )
}

fn folder_term_id(conn: &Connection, args: &Value) -> Result<Option<i64>, String> {
    if let Some(id) = args.get("browse_term_id").and_then(|v| v.as_i64()) {
        return Ok(Some(id));
    }
    if let Some(folder) = args.get("folder").and_then(|v| v.as_str()) {
        let t = folder.trim();
        if t.is_empty() {
            return Ok(None);
        }
        return Ok(Some(resolve_browse_term(conn, t)?.id));
    }
    Ok(None)
}

fn scoped_assets(conn: &Connection, args: &Value) -> Result<Vec<AssetDto>, String> {
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    match folder_term_id(conn, args)? {
        Some(id) => list_assets_in_folder(conn, id, recursive),
        None => list_assets(conn),
    }
}

fn resolve_asset(conn: &Connection, args: &Value) -> Result<AssetDto, String> {
    if let Some(id) = args.get("asset_id").and_then(|v| v.as_i64()) {
        let asset = list_assets(conn)?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("NOT_FOUND: asset_id={id}"))?;
        if let Some(folder_id) = folder_term_id(conn, args)? {
            let recursive = args
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let in_folder = list_assets_in_folder(conn, folder_id, recursive)?;
            if !in_folder.iter().any(|a| a.id == id) {
                return Err(format!(
                    "NOT_FOUND: asset_id={id} not in folder browse_term_id={folder_id}"
                ));
            }
        }
        return Ok(asset);
    }
    if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
        let abs = Path::new(path)
            .canonicalize()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string());
        let asset_id: Option<i64> = conn
            .query_row(
                "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
                params![&abs],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .or_else(|| {
                conn.query_row(
                    "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
                    params![path],
                    |r| r.get(0),
                )
                .optional()
                .ok()
                .flatten()
            });
        let id = asset_id.ok_or_else(|| format!("NOT_FOUND: path={path}"))?;
        return list_assets(conn)?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("NOT_FOUND: asset_id={id}"));
    }
    if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
        let name = name.trim();
        if name.is_empty() {
            return Err("name is empty".into());
        }
        // Name lookup: folder scopes; default recursive true so nested titles resolve.
        let mut args_name = args.clone();
        if args_name.get("recursive").is_none() {
            args_name
                .as_object_mut()
                .map(|m| m.insert("recursive".into(), json!(true)));
        }
        let candidates: Vec<AssetDto> = scoped_assets(conn, &args_name)?
            .into_iter()
            .filter(|a| a.display_name.eq_ignore_ascii_case(name))
            .collect();
        return match candidates.len() {
            0 => Err(format!("NOT_FOUND: name='{name}'")),
            1 => Ok(candidates.into_iter().next().unwrap()),
            _ => {
                let detail = candidates
                    .iter()
                    .map(|a| {
                        format!(
                            "id={} folder={} path={}",
                            a.id,
                            a.browse_term_id,
                            a.absolute_path.as_deref().unwrap_or("?")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                Err(format!(
                    "AMBIGUOUS_ASSET: name='{name}' matches {}. Use asset_id or path.",
                    detail
                ))
            }
        };
    }
    Err("Provide asset_id, path, or name".into())
}

/// Shared by stdio MCP server and in-app Agent tool loop.
pub(crate) fn call_tool(conn: &Connection, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "asset_list_tree" => {
            let root = root_id(conn)?;
            let terms = list_terms(conn)?;
            let mut term_rows = Vec::new();
            for t in &terms {
                term_rows.push(json!({
                    "id": t.id,
                    "parentId": t.parent_id,
                    "code": t.code,
                    "displayName": t.display_name,
                    "sortOrder": t.sort_order,
                    "path": term_display_path(conn, t.id)?,
                }));
            }
            let assets_dir = crate::settings::resolve_assets_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let inbox_dir = crate::settings::resolve_inbox_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(json!({
                "rootTermId": root,
                "assetsDir": assets_dir,
                "inboxDir": inbox_dir,
                "terms": term_rows,
                "assets": list_assets(conn)?,
            }))
        }
        "asset_list_folder" => {
            let recursive = args
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match folder_term_id(conn, args)? {
                None => Ok(json!({
                    "scope": "global",
                    "recursive": recursive,
                    "assets": list_assets(conn)?,
                })),
                Some(id) => {
                    let path = term_display_path(conn, id)?;
                    Ok(json!({
                        "scope": "folder",
                        "browseTermId": id,
                        "folderPath": path,
                        "recursive": recursive,
                        "assets": list_assets_in_folder(conn, id, recursive)?,
                    }))
                }
            }
        }
        "asset_search" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "query required".to_string())?;
            let mode = args
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("name")
                .to_ascii_lowercase();
            if mode == "fulltext" {
                return Ok(json!({
                    "implemented": false,
                    "mode": "fulltext",
                    "message": "Full-text search is not implemented yet",
                    "assets": []
                }));
            }
            if mode != "name" {
                return Err(format!("unsupported search mode: {mode}"));
            }
            let recursive = args
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let folder = folder_term_id(conn, args)?;
            let assets = search_assets_by_name(conn, query, folder, recursive)?;
            Ok(json!({
                "implemented": true,
                "mode": "name",
                "query": query.trim(),
                "browseTermId": folder,
                "recursive": recursive,
                "assets": assets,
            }))
        }
        "asset_get" => Ok(serde_json::to_value(resolve_asset(conn, args)?).map_err(|e| e.to_string())?),
        "asset_read_content" => {
            let asset = resolve_asset(conn, args)?;
            let path = asset
                .absolute_path
                .ok_or_else(|| "INVALID_INDEX: no current path".to_string())?;
            let max = args
                .get("max_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(2_000_000) as usize;
            let mut bytes = fs::read(&path).map_err(|e| e.to_string())?;
            let truncated = bytes.len() > max;
            if truncated {
                bytes.truncate(max);
            }
            let content = String::from_utf8_lossy(&bytes).into_owned();
            Ok(json!({
                "asset_id": asset.id,
                "path": path,
                "browse_term_id": asset.browse_term_id,
                "content": content,
                "truncated": truncated,
                "mtime": file_mtime_secs(&path),
            }))
        }
        "asset_register" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "path required".to_string())?;
            let source_path = args
                .get("source_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let display_name = args
                .get("display_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // Omit folder → root.
            let term = folder_term_id(conn, args)?;
            let asset = create_from_path_conn(conn, path, source_path, display_name, term)?;
            Ok(serde_json::to_value(asset).map_err(|e| e.to_string())?)
        }
        "asset_write_content" => {
            let asset = resolve_asset(conn, args)?;
            let path = asset
                .absolute_path
                .ok_or_else(|| "INVALID_INDEX: no current path".to_string())?;
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "content required".to_string())?;
            if let Some(expected) = args.get("expected_mtime").and_then(|v| v.as_i64()) {
                let actual = file_mtime_secs(&path)
                    .ok_or_else(|| "MTIME_CONFLICT: cannot read current mtime".to_string())?;
                if actual != expected {
                    return Err(format!(
                        "MTIME_CONFLICT: expected {expected}, actual {actual}"
                    ));
                }
            }
            fs::write(&path, content).map_err(|e| e.to_string())?;
            Ok(json!({
                "asset_id": asset.id,
                "path": path,
                "browse_term_id": asset.browse_term_id,
                "mtime": file_mtime_secs(&path),
                "size": content.len(),
            }))
        }
        "asset_delete" => {
            let id = args
                .get("asset_id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "asset_id required".to_string())?;
            let n = conn
                .execute("DELETE FROM asset_instance WHERE id = ?1", params![id])
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err(format!("NOT_FOUND: asset_id={id}"));
            }
            Ok(json!({
                "deleted_asset_id": id,
                "note": "Index removed only; disk file and conversion_log kept (L7)."
            }))
        }
        other => Err(format!("Unknown tool: {other}")),
    }
}

pub(crate) fn read_message(stdin: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = stdin.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                rest.trim()
                    .parse()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            );
        }
    }
    let len = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length")
    })?;
    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf)?;
    let v: Value = serde_json::from_slice(&buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(v))
}

pub(crate) fn write_message(stdout: &mut impl Write, msg: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(msg)?;
    write!(stdout, "Content-Length: {}\r\n\r\n", body.len())?;
    stdout.write_all(&body)?;
    stdout.flush()?;
    Ok(())
}

fn handle(req: &Value, conn: &Connection) -> Option<Value> {
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = req.get("id").cloned();
    if id.is_none() {
        return None;
    }
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "monolith-mcp", "version": "0.1.1" }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let params = req.get("params").cloned().unwrap_or(json!({}));
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(conn, name, &args) {
                Ok(v) => Ok(json!({
                    "content": [{ "type": "text", "text": v.to_string() }],
                    "structuredContent": v,
                    "isError": false
                })),
                Err(e) => Ok(json!({
                    "content": [{ "type": "text", "text": e }],
                    "isError": true
                })),
            }
        }
        _ => Err(format!("Method not found: {method}")),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc":"2.0","id":id,"result":r}),
        Err(e) => json!({
            "jsonrpc":"2.0",
            "id": id,
            "error": { "code": -32601, "message": e }
        }),
    })
}

/// Run MCP stdio loop until stdin EOF.
pub fn run_stdio() -> Result<(), String> {
    let path = default_assets_db_path()?;
    let conn = init_asset_db_at(&path)?;
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut stdout = io::stdout().lock();
    eprintln!(
        "monolith-mcp ready db={} tools={}",
        path.display(),
        tools().len()
    );
    while let Some(msg) = read_message(&mut stdin).map_err(|e| e.to_string())? {
        if let Some(resp) = handle(&msg, &conn) {
            write_message(&mut stdout, &resp).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
