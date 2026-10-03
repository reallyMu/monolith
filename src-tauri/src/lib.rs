use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager, RunEvent,
};

const MAX_RECENT: usize = 20;
const RECENT_FILE: &str = "recent.json";

/// Paths from Finder “Open With” / double-click. Opened can arrive before setup, so use a static.
static PENDING_OPENS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RecentStore {
    paths: Vec<String>,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn recent_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(RECENT_FILE))
}

fn load_recent(app: &AppHandle) -> Result<RecentStore, String> {
    let path = recent_file_path(app)?;
    if !path.exists() {
        return Ok(RecentStore::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_recent(app: &AppHandle, store: &RecentStore) -> Result<(), String> {
    let path = recent_file_path(app)?;
    let raw = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

fn is_binary_ish(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

/// Drop Finder/download quarantine so Gatekeeper does not warn on the document name.
#[cfg(target_os = "macos")]
fn clear_quarantine(path: &Path) {
    let _ = std::process::Command::new("/usr/bin/xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(path)
        .output();
}

#[cfg(not(target_os = "macos"))]
fn clear_quarantine(_path: &Path) {}

fn stash_open_paths(paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    if let Ok(mut g) = PENDING_OPENS.lock() {
        g.extend(paths);
    }
}

fn paths_from_urls(urls: Vec<url::Url>) -> Vec<String> {
    urls.into_iter()
        .filter_map(|u| u.to_file_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Finder double-click / Open With often delivers Opened while Monolith stays behind.
/// Show the window, activate NSApp, then focus so it becomes the frontmost app.
fn bring_to_front(app: &AppHandle) {
    let win = app.get_webview_window("main").or_else(|| {
        app.webview_windows()
            .into_iter()
            .next()
            .map(|(_, w)| w)
    });
    if let Some(ref win) = win {
        let _ = win.unminimize();
        let _ = win.show();
    }
    #[cfg(target_os = "macos")]
    activate_macos_app();
    if let Some(win) = win {
        let _ = win.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn activate_macos_app() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let ns_app = NSApplication::sharedApplication(mtm);
    // macOS 14+: activate(); keep IgnoringOtherApps for reliable Finder→app raise.
    ns_app.activate();
    #[allow(deprecated)]
    ns_app.activateIgnoringOtherApps(true);
}

fn handle_opened(app: &AppHandle, urls: Vec<url::Url>) {
    let paths = paths_from_urls(urls);
    if paths.is_empty() {
        return;
    }
    for p in &paths {
        clear_quarantine(Path::new(p));
    }
    stash_open_paths(paths.clone());
    bring_to_front(app);
    let _ = app.emit("open-files", paths);
}

#[tauri::command]
fn take_pending_opens() -> Vec<String> {
    PENDING_OPENS
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {path}"));
    }
    clear_quarantine(p);
    let bytes = fs::read(p).map_err(|e| e.to_string())?;
    if is_binary_ish(&bytes) {
        return Err("Rejected: file contains null bytes (binary or non-text)".into());
    }
    String::from_utf8(bytes).map_err(|_| "Rejected: file is not valid UTF-8".into())
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_recent(app: AppHandle) -> Result<Vec<String>, String> {
    Ok(load_recent(&app)?.paths)
}

#[tauri::command]
fn push_recent(app: AppHandle, path: String) -> Result<Vec<String>, String> {
    let mut store = load_recent(&app)?;
    store.paths.retain(|p| p != &path);
    store.paths.insert(0, path);
    store.paths.truncate(MAX_RECENT);
    save_recent(&app, &store)?;
    let _ = rebuild_menu(&app, &store.paths);
    Ok(store.paths)
}

#[tauri::command]
fn clear_recent(app: AppHandle) -> Result<(), String> {
    save_recent(&app, &RecentStore::default())?;
    let _ = rebuild_menu(&app, &[]);
    Ok(())
}

fn rebuild_menu(app: &AppHandle, recent: &[String]) -> Result<(), String> {
    let menu = build_menu(app, recent)?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(())
}

fn build_menu(app: &AppHandle, recent: &[String]) -> Result<Menu<tauri::Wry>, String> {
    let new_item = MenuItem::with_id(app, "file-new", "New", true, Some("CmdOrCtrl+N"))
        .map_err(|e| e.to_string())?;
    let open_item = MenuItem::with_id(app, "file-open", "Open…", true, Some("CmdOrCtrl+O"))
        .map_err(|e| e.to_string())?;
    let save_item = MenuItem::with_id(app, "file-save", "Save", true, Some("CmdOrCtrl+S"))
        .map_err(|e| e.to_string())?;
    let save_as_item =
        MenuItem::with_id(app, "file-save-as", "Save As…", true, Some("Shift+CmdOrCtrl+S"))
            .map_err(|e| e.to_string())?;
    let close_item = MenuItem::with_id(app, "file-close", "Close Tab", true, Some("CmdOrCtrl+W"))
        .map_err(|e| e.to_string())?;
    let clear_recent_item =
        MenuItem::with_id(app, "file-clear-recent", "Clear Recent", true, None::<&str>)
            .map_err(|e| e.to_string())?;

    let r0 = recent_item(app, recent, 0)?;
    let r1 = recent_item(app, recent, 1)?;
    let r2 = recent_item(app, recent, 2)?;
    let r3 = recent_item(app, recent, 3)?;
    let r4 = recent_item(app, recent, 4)?;
    let r5 = recent_item(app, recent, 5)?;
    let r6 = recent_item(app, recent, 6)?;
    let r7 = recent_item(app, recent, 7)?;
    let r8 = recent_item(app, recent, 8)?;
    let r9 = recent_item(app, recent, 9)?;

    let sep_r = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let recent_menu = Submenu::with_id_and_items(
        app,
        "recent-menu",
        "Open Recent",
        true,
        &[
            &r0, &r1, &r2, &r3, &r4, &r5, &r6, &r7, &r8, &r9, &sep_r, &clear_recent_item,
        ],
    )
    .map_err(|e| e.to_string())?;

    let sep1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit = PredefinedMenuItem::quit(app, Some("Quit Monolith")).map_err(|e| e.to_string())?;

    let file_menu = Submenu::with_id_and_items(
        app,
        "file",
        "File",
        true,
        &[
            &new_item,
            &open_item,
            &save_item,
            &save_as_item,
            &sep1,
            &recent_menu,
            &close_item,
            &sep2,
            &quit,
        ],
    )
    .map_err(|e| e.to_string())?;

    let undo = MenuItem::with_id(app, "edit-undo", "Undo", true, Some("CmdOrCtrl+Z"))
        .map_err(|e| e.to_string())?;
    let redo = MenuItem::with_id(app, "edit-redo", "Redo", true, Some("Shift+CmdOrCtrl+Z"))
        .map_err(|e| e.to_string())?;
    let find = MenuItem::with_id(app, "edit-find", "Find…", true, Some("CmdOrCtrl+F"))
        .map_err(|e| e.to_string())?;
    let cut = PredefinedMenuItem::cut(app, None).map_err(|e| e.to_string())?;
    let copy = PredefinedMenuItem::copy(app, None).map_err(|e| e.to_string())?;
    let paste = PredefinedMenuItem::paste(app, None).map_err(|e| e.to_string())?;
    let select_all = PredefinedMenuItem::select_all(app, None).map_err(|e| e.to_string())?;
    let sep_e1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep_e2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    let edit_menu = Submenu::with_id_and_items(
        app,
        "edit",
        "Edit",
        true,
        &[
            &undo,
            &redo,
            &sep_e1,
            &cut,
            &copy,
            &paste,
            &select_all,
            &sep_e2,
            &find,
        ],
    )
    .map_err(|e| e.to_string())?;

    let view_edit =
        MenuItem::with_id(app, "view-edit", "Edit Only", true, None::<&str>).map_err(|e| e.to_string())?;
    let view_preview =
        MenuItem::with_id(app, "view-preview", "Preview Only", true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let view_split =
        MenuItem::with_id(app, "view-split", "Split", true, None::<&str>).map_err(|e| e.to_string())?;

    let view_menu = Submenu::with_id_and_items(
        app,
        "view",
        "View",
        true,
        &[&view_edit, &view_preview, &view_split],
    )
    .map_err(|e| e.to_string())?;

    let minimize = PredefinedMenuItem::minimize(app, None).map_err(|e| e.to_string())?;
    let maximize = PredefinedMenuItem::maximize(app, None).map_err(|e| e.to_string())?;
    let sep_w = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let close_win = PredefinedMenuItem::close_window(app, None).map_err(|e| e.to_string())?;

    let window_menu = Submenu::with_id_and_items(
        app,
        "window",
        "Window",
        true,
        &[&minimize, &maximize, &sep_w, &close_win],
    )
    .map_err(|e| e.to_string())?;

    Menu::with_items(app, &[&file_menu, &edit_menu, &view_menu, &window_menu])
        .map_err(|e| e.to_string())
}

fn recent_item(
    app: &AppHandle,
    recent: &[String],
    index: usize,
) -> Result<MenuItem<tauri::Wry>, String> {
    let id = format!("recent:{index}");
    if let Some(path) = recent.get(index) {
        let label = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);
        MenuItem::with_id(app, &id, label, true, None::<&str>).map_err(|e| e.to_string())
    } else {
        let label = if index == 0 && recent.is_empty() {
            "(Empty)"
        } else {
            "—"
        };
        MenuItem::with_id(app, &id, label, false, None::<&str>).map_err(|e| e.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Non-macOS / CLI: path as argv (macOS Finder uses RunEvent::Opened instead).
    let argv_paths: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter(|a| Path::new(a).is_file())
        .collect();
    stash_open_paths(argv_paths);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let recent = load_recent(&app.handle()).unwrap_or_default();
            rebuild_menu(&app.handle(), &recent.paths)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            if let Some(rest) = id.strip_prefix("recent:") {
                if let Ok(idx) = rest.parse::<usize>() {
                    if let Ok(store) = load_recent(&app) {
                        if let Some(path) = store.paths.get(idx) {
                            let _ = app.emit("menu-action", format!("open-recent:{path}"));
                            return;
                        }
                    }
                }
            }
            let _ = app.emit("menu-action", id);
        })
        .invoke_handler(tauri::generate_handler![
            take_pending_opens,
            read_text_file,
            write_text_file,
            list_recent,
            push_recent,
            clear_recent
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Opened { urls } = event {
                handle_opened(app, urls);
            }
        });
}
