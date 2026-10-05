//! Discover Agent hosts on this machine and install monolith-mcp + Skill into selected ones.
//! Catalog + existence probe (not a full-disk search).

use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpFormat {
    /// `{ "mcpServers": { "monolith": { "command", "args" } } }`
    JsonMcpServers,
    Unsupported,
}

#[derive(Debug, Clone)]
struct AgentHostSpec {
    id: &'static str,
    name: &'static str,
    /// Any existing path counts as "present on this machine".
    evidence: Vec<PathBuf>,
    mcp_config: Option<PathBuf>,
    mcp_format: McpFormat,
    skill_dirs: Vec<PathBuf>,
    /// Shown when discovered but not auto-installable.
    skip_reason: Option<&'static str>,
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME not set".to_string())
}

fn catalog() -> Result<Vec<AgentHostSpec>, String> {
    let h = home()?;
    let apps = PathBuf::from("/Applications");
    let support = h.join("Library/Application Support");
    Ok(vec![
        AgentHostSpec {
            id: "cursor",
            name: "Cursor",
            evidence: vec![
                apps.join("Cursor.app"),
                h.join(".cursor/mcp.json"),
                h.join(".cursor"),
            ],
            mcp_config: Some(h.join(".cursor/mcp.json")),
            mcp_format: McpFormat::JsonMcpServers,
            skill_dirs: vec![
                h.join(".cursor/skills/monolith-assets"),
                h.join(".agents/skills/monolith-assets"),
            ],
            skip_reason: None,
        },
        AgentHostSpec {
            id: "claude-desktop",
            name: "Claude Desktop",
            evidence: vec![
                apps.join("Claude.app"),
                support.join("Claude/claude_desktop_config.json"),
            ],
            mcp_config: Some(support.join("Claude/claude_desktop_config.json")),
            mcp_format: McpFormat::JsonMcpServers,
            skill_dirs: vec![],
            skip_reason: None,
        },
        AgentHostSpec {
            id: "claude-cli",
            name: "Claude Code (CLI)",
            evidence: vec![h.join(".claude.json"), h.join(".claude")],
            mcp_config: None,
            mcp_format: McpFormat::Unsupported,
            skill_dirs: vec![h.join(".claude/skills/monolith-assets")],
            skip_reason: Some("暂不支持自动写入；请用 `claude mcp add` 注册 monolith-mcp"),
        },
        AgentHostSpec {
            id: "codex",
            name: "Codex",
            evidence: vec![h.join(".codex/config.toml"), h.join(".codex")],
            mcp_config: None,
            mcp_format: McpFormat::Unsupported,
            skill_dirs: vec![h.join(".codex/skills/monolith-assets")],
            skip_reason: Some("Codex 使用 TOML，一期不自动写入 MCP"),
        },
        AgentHostSpec {
            id: "qoder",
            name: "Qoder",
            evidence: vec![
                apps.join("Qoder.app"),
                support.join("Qoder"),
                support.join("Qoder/SharedClientCache/mcp.json"),
            ],
            mcp_config: Some(support.join("Qoder/SharedClientCache/mcp.json")),
            mcp_format: McpFormat::JsonMcpServers,
            skill_dirs: vec![],
            skip_reason: None,
        },
        AgentHostSpec {
            id: "vscode",
            name: "VS Code",
            evidence: vec![
                apps.join("Visual Studio Code.app"),
                support.join("Code/User/mcp.json"),
            ],
            mcp_config: Some(support.join("Code/User/mcp.json")),
            mcp_format: McpFormat::JsonMcpServers,
            skill_dirs: vec![],
            skip_reason: None,
        },
        AgentHostSpec {
            id: "windsurf",
            name: "Windsurf",
            evidence: vec![
                apps.join("Windsurf.app"),
                h.join(".codeium/windsurf/mcp_config.json"),
            ],
            mcp_config: Some(h.join(".codeium/windsurf/mcp_config.json")),
            mcp_format: McpFormat::JsonMcpServers,
            skill_dirs: vec![],
            skip_reason: None,
        },
    ])
}

fn any_exists(paths: &[PathBuf]) -> bool {
    paths.iter().any(|p| p.exists())
}

fn json_has_monolith_server(path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    v.get("mcpServers")
        .and_then(|s| s.get("monolith"))
        .is_some()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredAgent {
    pub id: String,
    pub name: String,
    pub mcp_config_path: Option<String>,
    pub skill_dirs: Vec<String>,
    pub installable: bool,
    pub already_has_monolith: bool,
    pub skip_reason: Option<String>,
}

/// Probe the known-host catalog; return only agents present on this machine.
#[tauri::command]
pub fn mcp_discover_agents() -> Result<Vec<DiscoveredAgent>, String> {
    let mut out = Vec::new();
    for spec in catalog()? {
        if !any_exists(&spec.evidence) {
            continue;
        }
        let installable = spec.mcp_config.is_some()
            && spec.mcp_format == McpFormat::JsonMcpServers
            && spec.skip_reason.is_none();
        let already = spec
            .mcp_config
            .as_ref()
            .map(|p| p.is_file() && json_has_monolith_server(p))
            .unwrap_or(false);
        out.push(DiscoveredAgent {
            id: spec.id.into(),
            name: spec.name.into(),
            mcp_config_path: spec
                .mcp_config
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            skill_dirs: spec
                .skill_dirs
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            installable,
            already_has_monolith: already,
            skip_reason: spec.skip_reason.map(|s| s.to_string()),
        });
    }
    Ok(out)
}

fn app_support_bin_dir() -> Result<PathBuf, String> {
    let dir = home()?.join("Library/Application Support/com.muqiang.monolith/bin");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn bundled_mcp_candidates(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        v.push(res.join("monolith-mcp"));
        v.push(res.join("bin/monolith-mcp"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join("monolith-mcp"));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    v.push(manifest.join("target/release/monolith-mcp"));
    v.push(manifest.join("target/debug/monolith-mcp"));
    v
}

fn copy_mcp_binary(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dest_dir = app_support_bin_dir()?;
    let dest = dest_dir.join("monolith-mcp");
    let mut last_err = "monolith-mcp binary not found".to_string();
    for src in bundled_mcp_candidates(app) {
        if src.is_file() {
            fs::copy(&src, &dest).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest).map_err(|e| e.to_string())?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dest, perms).map_err(|e| e.to_string())?;
            }
            return Ok(dest);
        }
        last_err = format!("missing {}", src.display());
    }
    Err(format!(
        "Could not locate monolith-mcp ({last_err}). Build with: cargo build --bin monolith-mcp"
    ))
}

/// Bundled / repo template (read-only seed).
fn bundled_skill_src(app: &tauri::AppHandle) -> Option<PathBuf> {
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("skills/monolith-assets/SKILL.md");
        if p.is_file() {
            return Some(p);
        }
    }
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../skills/monolith-assets/SKILL.md");
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Editable canonical skill under App Support (seeded once from bundle).
fn editable_skill_path() -> Result<PathBuf, String> {
    Ok(crate::settings::app_data_dir()?.join("skills/monolith-assets/SKILL.md"))
}

fn ensure_editable_skill(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dest = editable_skill_path()?;
    if dest.is_file() {
        return Ok(dest);
    }
    let Some(src) = bundled_skill_src(app) else {
        return Err("SKILL.md not found (bundle or repo)".into());
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

fn write_skill_body_to_dirs(body: &str, dirs: &[PathBuf]) -> Result<Vec<String>, String> {
    if dirs.is_empty() {
        return Ok(vec!["skill skipped (no skill root for selected agents)".into()]);
    }
    let mut notes = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let key = dir.to_string_lossy().into_owned();
        if !seen.insert(key) {
            continue;
        }
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let dest = dir.join("SKILL.md");
        fs::write(&dest, body).map_err(|e| e.to_string())?;
        notes.push(format!("skill → {}", dest.display()));
    }
    Ok(notes)
}

fn install_skills_to(app: &tauri::AppHandle, dirs: &[PathBuf]) -> Result<Vec<String>, String> {
    let src = ensure_editable_skill(app)?;
    let body = fs::read_to_string(&src).map_err(|e| e.to_string())?;
    write_skill_body_to_dirs(&body, dirs)
}

fn skill_dirs_for_agents(agents: &[String]) -> Result<Vec<PathBuf>, String> {
    let specs = catalog()?;
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in agents {
        let Some(spec) = specs.iter().find(|s| s.id == id) else {
            continue;
        };
        if !any_exists(&spec.evidence) {
            continue;
        }
        for d in &spec.skill_dirs {
            let key = d.to_string_lossy().into_owned();
            if seen.insert(key) {
                dirs.push(d.clone());
            }
        }
    }
    Ok(dirs)
}

fn all_discovered_skill_dirs() -> Result<Vec<PathBuf>, String> {
    let specs = catalog()?;
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for spec in specs {
        if !any_exists(&spec.evidence) || spec.skill_dirs.is_empty() {
            continue;
        }
        for d in spec.skill_dirs {
            let key = d.to_string_lossy().into_owned();
            if seen.insert(key) {
                dirs.push(d);
            }
        }
    }
    Ok(dirs)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSkillView {
    pub path: String,
    pub content: String,
    /// Destinations that would receive a reload (discovered hosts with skill roots).
    pub reload_targets: Vec<String>,
}

#[tauri::command]
pub fn mcp_skill_get(app: tauri::AppHandle) -> Result<McpSkillView, String> {
    let path = ensure_editable_skill(&app)?;
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let reload_targets = all_discovered_skill_dirs()?
        .into_iter()
        .map(|p| p.join("SKILL.md").to_string_lossy().into_owned())
        .collect();
    Ok(McpSkillView {
        path: path.to_string_lossy().into_owned(),
        content,
        reload_targets,
    })
}

#[tauri::command]
pub fn mcp_skill_save(app: tauri::AppHandle, content: String) -> Result<McpSkillView, String> {
    let path = ensure_editable_skill(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())?;
    mcp_skill_get(app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSkillReloadResult {
    pub notes: Vec<String>,
}

/// Push editable skill to Agent skill dirs.
/// Empty `agents` → all discovered hosts that have a skill root.
#[tauri::command]
pub fn mcp_skill_reload(
    app: tauri::AppHandle,
    agents: Vec<String>,
) -> Result<McpSkillReloadResult, String> {
    let src = ensure_editable_skill(&app)?;
    let body = fs::read_to_string(&src).map_err(|e| e.to_string())?;
    let dirs = if agents.is_empty() {
        all_discovered_skill_dirs()?
    } else {
        skill_dirs_for_agents(&agents)?
    };
    Ok(McpSkillReloadResult {
        notes: write_skill_body_to_dirs(&body, &dirs)?,
    })
}

/// MCP host configs must avoid spaces in `command` (e.g. `Application Support`).
/// Keep the real binary under App Support; expose a no-space symlink for agents.
#[cfg(unix)]
fn mcp_command_path(binary: &Path) -> Result<(PathBuf, Vec<String>), String> {
    let link_dir = home()?.join(".local/bin");
    fs::create_dir_all(&link_dir).map_err(|e| e.to_string())?;
    let link = link_dir.join("monolith-mcp");
    match fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() || meta.is_file() => {
            let _ = fs::remove_file(&link);
        }
        _ => {}
    }
    std::os::unix::fs::symlink(binary, &link).map_err(|e| {
        format!(
            "symlink {} → {}: {e}",
            link.display(),
            binary.display()
        )
    })?;
    Ok((
        link.clone(),
        vec![format!(
            "MCP command (no spaces): {} → {}",
            link.display(),
            binary.display()
        )],
    ))
}

#[cfg(not(unix))]
fn mcp_command_path(binary: &Path) -> Result<(PathBuf, Vec<String>), String> {
    Ok((binary.to_path_buf(), Vec::new()))
}

fn merge_mcp_server(config_path: &Path, command: &str) -> Result<(), String> {
    let mut root: Value = if config_path.is_file() {
        let raw = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if config_path.is_file() {
        let bak = config_path.with_extension("json.monolith-bak");
        let _ = fs::copy(config_path, bak);
    }
    let servers = root
        .as_object_mut()
        .ok_or_else(|| "MCP config root must be an object".to_string())?
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    let map = servers
        .as_object_mut()
        .ok_or_else(|| "mcpServers must be an object".to_string())?;
    map.insert(
        "monolith".into(),
        json!({
            "command": command,
            "args": []
        }),
    );
    let pretty = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    fs::write(config_path, pretty + "\n").map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstallResult {
    pub binary_path: String,
    pub notes: Vec<String>,
}

/// `agents`: discovered host ids, e.g. ["cursor", "claude-desktop"]
#[tauri::command]
pub fn mcp_install_for_agents(
    app: tauri::AppHandle,
    agents: Vec<String>,
) -> Result<McpInstallResult, String> {
    if agents.is_empty() {
        return Err("No agents selected".into());
    }
    let specs = catalog()?;
    let binary = copy_mcp_binary(&app)?;
    let (cmd_path, mut notes) = mcp_command_path(&binary)?;
    let cmd = cmd_path.to_string_lossy().into_owned();
    notes.push(format!("binary → {}", binary.display()));
    let mut skill_dirs: Vec<PathBuf> = Vec::new();

    for id in &agents {
        let Some(spec) = specs.iter().find(|s| s.id == id) else {
            notes.push(format!("skipped unknown agent: {id}"));
            continue;
        };
        if !any_exists(&spec.evidence) {
            notes.push(format!("skipped {id}: not found on this machine"));
            continue;
        }
        if spec.mcp_format != McpFormat::JsonMcpServers || spec.mcp_config.is_none() {
            let why = spec.skip_reason.unwrap_or("MCP auto-install unsupported");
            notes.push(format!("skipped {}: {why}", spec.name));
            // Still drop skill if we know a skill root (e.g. Codex skills folder).
            skill_dirs.extend(spec.skill_dirs.iter().cloned());
            continue;
        }
        let path = spec.mcp_config.as_ref().unwrap();
        merge_mcp_server(path, &cmd)?;
        notes.push(format!("{} mcp → {} (command={cmd})", spec.name, path.display()));
        skill_dirs.extend(spec.skill_dirs.iter().cloned());
    }

    notes.extend(install_skills_to(&app, &skill_dirs)?);

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg("-R").arg(&binary).status();
    }

    Ok(McpInstallResult {
        binary_path: cmd,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_finds_local_hosts() {
        let found = mcp_discover_agents().expect("discover");
        let ids: Vec<_> = found.iter().map(|a| a.id.as_str()).collect();
        assert!(
            ids.contains(&"cursor") || ids.contains(&"claude-desktop"),
            "expected cursor or claude-desktop, got {ids:?}"
        );
        for a in &found {
            if a.installable {
                assert!(a.mcp_config_path.is_some(), "{} installable without path", a.id);
            }
        }
    }
}
