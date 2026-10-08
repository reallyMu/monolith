//! Bulk-register a disk folder into browse terms (mirror) + assets.

use crate::assets::{
    create_from_path_conn, ensure_child_term, file_type_of, mount_asset, root_id, AssetDb,
};
use crate::convert::{resolve_import_output, run_convert_inner, tool_for_path, ConvertRuntime};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

const MAX_TERM_DEPTH: usize = 5;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FolderImportResult {
    pub created: u32,
    pub mounted: u32,
    pub skipped_unsupported: u32,
    pub convert_failed: u32,
    pub depth_skipped: u32,
    pub messages: Vec<String>,
}

fn skip_name(name: &str) -> bool {
    let n = name.trim();
    if n.is_empty() || n.starts_with('.') {
        return true;
    }
    n.eq_ignore_ascii_case("node_modules")
}

fn classify_file(path: &Path) -> FileClass {
    let s = path.to_string_lossy();
    if file_type_of(&s).is_ok() {
        return FileClass::Text;
    }
    if tool_for_path(&s).is_ok() {
        return FileClass::Convert;
    }
    FileClass::Skip
}

enum FileClass {
    Text,
    Convert,
    Skip,
}

fn walk_files(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for ent in entries {
        let path = ent.path();
        let name = ent.file_name().to_string_lossy().into_owned();
        if skip_name(&name) {
            continue;
        }
        if path.is_dir() {
            if recursive {
                walk_files(&path, true, out);
            }
            continue;
        }
        if path.is_file() {
            out.push(path);
        }
    }
}

fn walk_dirs(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    if !recursive {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for ent in entries {
        let path = ent.path();
        let name = ent.file_name().to_string_lossy().into_owned();
        if skip_name(&name) || !path.is_dir() {
            continue;
        }
        out.push(path.clone());
        walk_dirs(&path, true, out);
    }
}

fn rel_segments(root: &Path, path: &Path) -> Vec<String> {
    path.strip_prefix(root)
        .ok()
        .map(|p| {
            p.iter()
                .filter_map(|s| s.to_str().map(|x| x.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn ensure_mirror_term(
    conn: &rusqlite::Connection,
    import_root: i64,
    disk_root: &Path,
    file_or_dir: &Path,
    is_dir: bool,
    stats: &mut FolderImportResult,
) -> Option<i64> {
    let rel = if is_dir {
        rel_segments(disk_root, file_or_dir)
    } else {
        let parent = file_or_dir.parent().unwrap_or(file_or_dir);
        rel_segments(disk_root, parent)
    };
    if rel.len() + 1 > MAX_TERM_DEPTH {
        stats.depth_skipped += 1;
        return None;
    }
    let mut cur = import_root;
    for seg in rel {
        match ensure_child_term(conn, cur, &seg) {
            Ok(t) => cur = t.id,
            Err(e) => {
                stats.messages.push(e);
                stats.depth_skipped += 1;
                return None;
            }
        }
    }
    Some(cur)
}

fn existing_asset_id(conn: &rusqlite::Connection, abs: &str) -> Result<Option<i64>, String> {
    use rusqlite::{params, OptionalExtension};
    Ok(conn
        .query_row(
            "SELECT asset_id FROM asset_version WHERE absolute_path = ?1",
            params![abs],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn folder_import(
    app: AppHandle,
    db: State<'_, AssetDb>,
    rt: State<'_, ConvertRuntime>,
    dir: String,
    include_subdirs: bool,
) -> Result<FolderImportResult, String> {
    let disk_root = PathBuf::from(&dir);
    if !disk_root.is_dir() {
        return Err(format!("Not a directory: {dir}"));
    }
    let folder_name = disk_root
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid directory name".to_string())?;

    if rt
        .busy
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return Err("A conversion is already running".into());
    }
    rt.cancel.store(false, std::sync::atomic::Ordering::SeqCst);

    let result = (|| {
        let mut stats = FolderImportResult::default();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let root = root_id(&conn)?;
        let import_term = ensure_child_term(&conn, root, folder_name)?;

        let mut dirs = Vec::new();
        walk_dirs(&disk_root, include_subdirs, &mut dirs);
        for d in &dirs {
            let _ = ensure_mirror_term(&conn, import_term.id, &disk_root, d, true, &mut stats);
        }

        let mut files = Vec::new();
        walk_files(&disk_root, include_subdirs, &mut files);
        drop(conn);

        for path in files {
            if rt.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                stats.messages.push("cancelled".into());
                break;
            }
            let abs = path.to_string_lossy().into_owned();
            let class = classify_file(&path);
            match class {
                FileClass::Skip => {
                    stats.skipped_unsupported += 1;
                    continue;
                }
                FileClass::Text => {
                    let conn = db.0.lock().map_err(|e| e.to_string())?;
                    let Some(term) =
                        ensure_mirror_term(&conn, import_term.id, &disk_root, &path, false, &mut stats)
                    else {
                        continue;
                    };
                    match existing_asset_id(&conn, &abs)? {
                        Some(id) => {
                            if mount_asset(&conn, id, term)? {
                                stats.mounted += 1;
                            }
                        }
                        None => {
                            create_from_path_conn(&conn, &abs, None, None, Some(term))?;
                            stats.created += 1;
                        }
                    }
                }
                FileClass::Convert => {
                    let output = {
                        let conn = db.0.lock().map_err(|e| e.to_string())?;
                        let inbox = crate::settings::load_settings()
                            .map(|s| s.inbox_dir)
                            .unwrap_or_default();
                        resolve_import_output(&conn, &abs, &inbox)?
                    };
                    match run_convert_inner(&app, &db, &rt, &abs, &output) {
                        Ok(dto) => {
                            let conn = db.0.lock().map_err(|e| e.to_string())?;
                            let Some(term) = ensure_mirror_term(
                                &conn,
                                import_term.id,
                                &disk_root,
                                &path,
                                false,
                                &mut stats,
                            ) else {
                                continue;
                            };
                            match existing_asset_id(&conn, &dto.output_path)? {
                                Some(id) => {
                                    if mount_asset(&conn, id, term)? {
                                        stats.mounted += 1;
                                    }
                                }
                                None => {
                                    create_from_path_conn(
                                        &conn,
                                        &dto.output_path,
                                        Some(abs),
                                        None,
                                        Some(term),
                                    )?;
                                    stats.created += 1;
                                }
                            }
                        }
                        Err(e) => {
                            stats.convert_failed += 1;
                            stats.messages.push(format!("{}: {e}", path.display()));
                        }
                    }
                }
            }
        }
        Ok(stats)
    })();

    {
        let mut g = rt.child.lock().map_err(|e| e.to_string())?;
        *g = None;
    }
    rt.busy.store(false, std::sync::atomic::Ordering::SeqCst);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_dot_and_node_modules() {
        assert!(skip_name(".git"));
        assert!(skip_name(".DS_Store"));
        assert!(skip_name("node_modules"));
        assert!(!skip_name("docs"));
    }

    #[test]
    fn walk_skips_hidden_and_empty_dirs_listed() {
        let dir = std::env::temp_dir().join(format!(
            "ml-import-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("keep")).unwrap();
        fs::create_dir_all(dir.join("empty")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::write(dir.join("keep/a.md"), "# a").unwrap();
        fs::write(dir.join(".git/hidden.md"), "# h").unwrap();
        fs::write(dir.join("node_modules/x.md"), "# x").unwrap();
        fs::write(dir.join("skip.png"), [1, 2, 3]).unwrap();
        let mut files = Vec::new();
        walk_files(&dir, true, &mut files);
        let names: Vec<_> = files
            .iter()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
            .collect();
        assert!(names.contains(&"a.md".into()), "{names:?}");
        assert!(!names.iter().any(|n| n == "hidden.md" || n == "x.md"));
        assert!(names.contains(&"skip.png".into()));
        assert!(matches!(classify_file(&dir.join("keep/a.md")), FileClass::Text));
        assert!(matches!(classify_file(&dir.join("skip.png")), FileClass::Skip));
        let mut dirs = Vec::new();
        walk_dirs(&dir, true, &mut dirs);
        let dnames: Vec<_> = dirs
            .iter()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
            .collect();
        assert!(dnames.contains(&"keep".into()) && dnames.contains(&"empty".into()), "{dnames:?}");
        assert!(!dnames.iter().any(|n| n == ".git" || n == "node_modules"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn already_registered_path_counts_as_mount() {
        use crate::assets::{apply_asset_schema, create_from_path_conn, ensure_child_term, root_id};
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        apply_asset_schema(&conn).unwrap();
        let ts = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO asset_browse_term (id, parent_id, code, display_name, sort_order, status, created_at, updated_at)
             VALUES (1, NULL, 'root', 'Root', 0, 'ACTIVE', ?1, ?1)",
            rusqlite::params![ts],
        )
        .unwrap();
        let root = root_id(&conn).unwrap();
        let dir = std::env::temp_dir().join(format!("ml-imp2-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.md");
        fs::write(&f, "hi").unwrap();
        let abs = f.to_string_lossy().into_owned();
        let created = create_from_path_conn(&conn, &abs, None, None, Some(root)).unwrap();
        let other = ensure_child_term(&conn, root, "skills").unwrap();
        let id = existing_asset_id(&conn, &abs).unwrap();
        assert_eq!(id, Some(created.id));
        assert!(mount_asset(&conn, created.id, other.id).unwrap());
        assert!(!mount_asset(&conn, created.id, other.id).unwrap());
        let _ = fs::remove_file(&f);
        let _ = fs::remove_dir(&dir);
    }
}
