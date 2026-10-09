mod agent;
mod agent_bridge;
mod assets;
mod clipper;
mod convert;
mod docx_altchunk;
mod folder_import;
mod i18n;
mod mcp_client;
mod mcp_install;
pub mod mcp_server;
mod settings;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager, RunEvent, WindowEvent,
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
fn path_is_dir(path: String) -> bool {
    Path::new(&path).is_dir()
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
    use i18n::{menu as m, MenuMsg as K};

    let new_item = MenuItem::with_id(app, "file-new", m(K::New), true, Some("CmdOrCtrl+N"))
        .map_err(|e| e.to_string())?;
    let open_item = MenuItem::with_id(app, "file-open", m(K::Open), true, Some("CmdOrCtrl+O"))
        .map_err(|e| e.to_string())?;
    let save_item = MenuItem::with_id(app, "file-save", m(K::Save), true, Some("CmdOrCtrl+S"))
        .map_err(|e| e.to_string())?;
    let save_as_item = MenuItem::with_id(
        app,
        "file-save-as",
        m(K::SaveAs),
        true,
        Some("Shift+CmdOrCtrl+S"),
    )
    .map_err(|e| e.to_string())?;
    let convert_item =
        MenuItem::with_id(app, "file-convert-md", m(K::ConvertMd), true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let inbox_item =
        MenuItem::with_id(app, "tools-browse-inbox", m(K::BrowseInbox), true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let folder_import_item = MenuItem::with_id(
        app,
        "assets-folder-import",
        m(K::FolderImport),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let asset_register_item = MenuItem::with_id(
        app,
        "assets-generate",
        m(K::RegisterAsset),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let asset_version_item = MenuItem::with_id(
        app,
        "assets-new-version",
        m(K::SaveNewVersion),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let settings_item = MenuItem::with_id(
        app,
        "app-settings",
        m(K::Settings),
        true,
        Some("CmdOrCtrl+,"),
    )
    .map_err(|e| e.to_string())?;
    let close_item =
        MenuItem::with_id(app, "file-close", m(K::CloseTab), true, Some("CmdOrCtrl+W"))
            .map_err(|e| e.to_string())?;
    let clear_recent_item =
        MenuItem::with_id(app, "file-clear-recent", m(K::ClearRecent), true, None::<&str>)
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
        m(K::OpenRecent),
        true,
        &[
            &r0, &r1, &r2, &r3, &r4, &r5, &r6, &r7, &r8, &r9, &sep_r, &clear_recent_item,
        ],
    )
    .map_err(|e| e.to_string())?;

    let sep1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep3 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep4 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep5 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit =
        PredefinedMenuItem::quit(app, Some(m(K::QuitApp))).map_err(|e| e.to_string())?;

    // Groups: file ops + inbox | asset register/version | settings | close/quit
    let file_menu = Submenu::with_id_and_items(
        app,
        "file",
        m(K::File),
        true,
        &[
            &new_item,
            &open_item,
            &inbox_item,
            &folder_import_item,
            &convert_item,
            &save_item,
            &save_as_item,
            &sep1,
            &recent_menu,
            &sep2,
            &asset_register_item,
            &asset_version_item,
            &sep3,
            &settings_item,
            &sep4,
            &close_item,
            &sep5,
            &quit,
        ],
    )
    .map_err(|e| e.to_string())?;

    let undo = MenuItem::with_id(app, "edit-undo", m(K::Undo), true, Some("CmdOrCtrl+Z"))
        .map_err(|e| e.to_string())?;
    let redo = MenuItem::with_id(
        app,
        "edit-redo",
        m(K::Redo),
        true,
        Some("Shift+CmdOrCtrl+Z"),
    )
    .map_err(|e| e.to_string())?;
    let find = MenuItem::with_id(app, "edit-find", m(K::Find), true, Some("CmdOrCtrl+F"))
        .map_err(|e| e.to_string())?;
    // Predefined labels follow the OS language automatically.
    let cut = PredefinedMenuItem::cut(app, None).map_err(|e| e.to_string())?;
    let copy = PredefinedMenuItem::copy(app, None).map_err(|e| e.to_string())?;
    let paste = PredefinedMenuItem::paste(app, None).map_err(|e| e.to_string())?;
    let select_all = PredefinedMenuItem::select_all(app, None).map_err(|e| e.to_string())?;
    let sep_e1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep_e2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    let edit_menu = Submenu::with_id_and_items(
        app,
        "edit",
        m(K::Edit),
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
        MenuItem::with_id(app, "view-edit", m(K::EditOnly), true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let view_preview =
        MenuItem::with_id(app, "view-preview", m(K::PreviewOnly), true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let view_split =
        MenuItem::with_id(app, "view-split", m(K::CompareRender), true, None::<&str>)
            .map_err(|e| e.to_string())?;
    // Claim ⌘E at the menu layer so macOS/WebKit cannot treat it as "Use Selection for Find".
    let view_toggle = MenuItem::with_id(
        app,
        "view-cycle-mode",
        m(K::CycleView),
        true,
        Some("CmdOrCtrl+E"),
    )
    .map_err(|e| e.to_string())?;
    let sep_v = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    let view_menu = Submenu::with_id_and_items(
        app,
        "view",
        m(K::View),
        true,
        &[&view_edit, &view_preview, &view_split, &sep_v, &view_toggle],
    )
    .map_err(|e| e.to_string())?;

    let minimize = PredefinedMenuItem::minimize(app, None).map_err(|e| e.to_string())?;
    let maximize = PredefinedMenuItem::maximize(app, None).map_err(|e| e.to_string())?;
    let sep_w = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let close_win = PredefinedMenuItem::close_window(app, None).map_err(|e| e.to_string())?;

    let window_menu = Submenu::with_id_and_items(
        app,
        "window",
        m(K::Window),
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
            i18n::menu(i18n::MenuMsg::RecentEmpty)
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
            let asset_db = assets::init_asset_db(&app.handle())?;
            app.manage(asset_db);
            app.manage(convert::ConvertRuntime::default());
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
        // macOS ❌ = hide to Dock (keep process). Fullscreen must exit first or
        // macOS leaves a black Space; see tauri#10580.
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.is_fullscreen().unwrap_or(false) {
                    let win = window.clone();
                    let _ = window.set_fullscreen(false);
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(700));
                        let win2 = win.clone();
                        let _ = win.run_on_main_thread(move || {
                            let _ = win2.hide();
                        });
                    });
                } else {
                    let _ = window.hide();
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (window, event);
            }
        })
        .manage(agent_bridge::BridgeState::default())
        .invoke_handler(tauri::generate_handler![
            take_pending_opens,
            path_is_dir,
            read_text_file,
            write_text_file,
            list_recent,
            push_recent,
            clear_recent,
            assets::asset_list_tree,
            assets::asset_create_from_path,
            assets::asset_delete,
            assets::asset_move,
            assets::asset_unmount,
            assets::asset_rename,
            assets::asset_relocate,
            assets::asset_find_by_path,
            assets::asset_search_by_name,
            assets::asset_list_versions,
            assets::asset_set_current_version,
            assets::asset_save_new_version,
            assets::reveal_in_os,
            assets::open_in_os,
            assets::term_create,
            assets::term_rename,
            assets::term_move,
            assets::term_delete,
            convert::convert_tool_for_path,
            convert::convert_is_supported,
            convert::convert_default_output,
            convert::conversion_latest_source,
            convert::convert_run,
            convert::convert_cancel,
            folder_import::folder_import,
            clipper::clipper_inbox_dir,
            clipper::clipper_list_inbox,
            clipper::clipper_ensure_extension,
            clipper::clipper_extension_dir,
            clipper::clipper_open_install,
            mcp_install::mcp_discover_agents,
            mcp_install::mcp_install_for_agents,
            mcp_install::mcp_skill_get,
            mcp_install::mcp_skill_save,
            mcp_install::mcp_skill_reload,
            settings::settings_get,
            settings::settings_save,
            agent::agent_skills_load,
            agent::agent_rag_retrieve,
            agent::agent_rag_test,
            agent::agent_tool_call,
            agent::agent_mcp_tools,
            agent::agent_runtime_status,
            agent::agent_active_chat_llm,
            agent_bridge::agent_bridge_status,
            agent_bridge::agent_bridge_start,
            agent_bridge::agent_bridge_stop
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            match event {
                RunEvent::Opened { urls } => handle_opened(app, urls),
                RunEvent::Reopen {
                    has_visible_windows,
                    ..
                } => {
                    if !has_visible_windows {
                        bring_to_front(app);
                    }
                }
                // Hide-on-close already prevented teardown; if something still
                // requests exit without a code, stay alive. ⌘Q uses terminate.
                RunEvent::ExitRequested { api, code, .. } => {
                    if code.is_none() {
                        api.prevent_exit();
                    } else if let Some(state) = app.try_state::<agent_bridge::BridgeState>() {
                        if let Ok(mut guard) = state.child.lock() {
                            if let Some(mut child) = guard.take() {
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                        }
                    }
                }
                _ => {}
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app, event);
            }
        });
}
