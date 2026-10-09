//! Pi Agent Bridge sidecar: materialize App Support config, sync settings, spawn Node.

use crate::settings::{self, bridge_port, load_settings, AppSettings, LlmEndpoint};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const MIN_NODE_MAJOR: u32 = 22;
const MIN_NODE_MINOR: u32 = 19;
/// Pi models.json / settings provider id written by Monolith sync.
const PI_PROVIDER_ID: &str = "monolith";
const BRIDGE_HOST: &str = "127.0.0.1";

pub struct BridgeState {
    pub child: Mutex<Option<Child>>,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub up: bool,
    pub port: u16,
    pub bridge_url: String,
    pub health_url: String,
    pub pid: Option<u32>,
    pub node_ok: bool,
    pub node_version: Option<String>,
    pub node_error: Option<String>,
    pub agent_dir: String,
    pub workdir: String,
    pub bridge_script: String,
    pub health: Option<Value>,
    pub hint: Option<String>,
}

pub fn pi_agent_runtime_dir() -> Result<PathBuf, String> {
    Ok(settings::app_data_dir()?.join("pi-agent"))
}

pub fn pi_agent_dir() -> Result<PathBuf, String> {
    Ok(pi_agent_runtime_dir()?.join(".pi-agent"))
}

fn logs_dir() -> Result<PathBuf, String> {
    Ok(pi_agent_runtime_dir()?.join("logs"))
}

/// Bundled Resources first; otherwise repo `third-party/pi-agent` for `cargo tauri dev`.
pub fn resolve_bundled_pi_root(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(res) = app.path().resource_dir() {
        let cand = res.join("third-party/pi-agent");
        if cand.join("bridge/server.mjs").is_file() {
            return Ok(cand);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../third-party/pi-agent");
    let dev = fs::canonicalize(&dev).map_err(|e| {
        format!(
            "Pi Agent runtime not found under Resources or third-party/pi-agent ({e}). Run scripts/pi-agent-install.sh."
        )
    })?;
    if dev.join("bridge/server.mjs").is_file() {
        return Ok(dev);
    }
    Err(
        "Pi Agent runtime not found (bundled Resources or third-party/pi-agent). Run scripts/pi-agent-install.sh."
            .into(),
    )
}

fn bridge_script(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_bundled_pi_root(app)?.join("bridge/server.mjs"))
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let to = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else if ty.is_file() && !to.exists() {
            fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Ensure App Support `pi-agent/.pi-agent` exists (seed once; never wipe sessions).
pub fn materialize_pi_runtime(app: &AppHandle) -> Result<PathBuf, String> {
    let runtime = pi_agent_runtime_dir()?;
    fs::create_dir_all(runtime.join("logs")).map_err(|e| e.to_string())?;
    let agent = pi_agent_dir()?;
    let seed = resolve_bundled_pi_root(app)?.join("seed/.pi-agent");
    if !seed.is_dir() {
        return Err(format!("Pi Agent seed missing: {}", seed.display()));
    }
    if !agent.exists() {
        copy_dir_recursive(&seed, &agent)?;
    } else {
        for name in ["settings.json", "models.json", "auth.json"] {
            let dest = agent.join(name);
            let src = seed.join(name);
            if !dest.exists() && src.is_file() {
                fs::copy(&src, &dest).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(agent)
}

fn active_chat_llm(s: &AppSettings) -> Option<&LlmEndpoint> {
    s.llms
        .iter()
        .find(|l| l.id == s.active_chat_llm_id && l.enabled)
        .or_else(|| {
            s.llms
                .iter()
                .find(|l| l.enabled && (l.role == "chat" || l.role == "both"))
        })
}

/// Require an enabled chat LLM with baseUrl + model (no invented defaults).
pub fn require_active_chat_llm(s: &AppSettings) -> Result<&LlmEndpoint, String> {
    let llm = active_chat_llm(s).ok_or_else(|| {
        "No enabled chat LLM. Configure Settings → LLM (baseUrl, model, apiKey as required by the provider)."
            .to_string()
    })?;
    let label = if llm.name.trim().is_empty() {
        llm.id.as_str()
    } else {
        llm.name.as_str()
    };
    if llm.base_url.trim().is_empty() {
        return Err(format!("LLM «{label}» has empty baseUrl."));
    }
    if llm.model.trim().is_empty() {
        return Err(format!("LLM «{label}» has empty model."));
    }
    Ok(llm)
}

/// Map `…/name/SKILL.md` → skill package dir `…/name`; otherwise use the path as-is.
pub fn skill_package_dir(path: &str) -> Option<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let p = PathBuf::from(path);
    let is_skill_md = p
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("SKILL.md"));
    let dir = if is_skill_md {
        p.parent()?.to_path_buf()
    } else {
        p
    };
    if dir.as_os_str().is_empty() {
        return None;
    }
    Some(dir)
}

pub fn skill_dirs(s: &AppSettings) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for sk in s.agent_skills.iter().filter(|x| x.enabled) {
        if let Some(dir) = skill_package_dir(&sk.path) {
            let key = dir.to_string_lossy().into_owned();
            if seen.insert(key.clone()) {
                dirs.push(key);
            }
        }
    }
    dirs
}

pub fn mcp_server_key(name: &str, id: &str) -> String {
    let raw = if name.trim().is_empty() { id } else { name };
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn build_mcp_servers_object(s: &AppSettings) -> Map<String, Value> {
    let mut mcp_servers = Map::new();
    for m in s.agent_mcp_servers.iter().filter(|x| x.enabled) {
        let cmd = m.command.trim();
        if cmd.is_empty() {
            continue;
        }
        let key = mcp_server_key(&m.name, &m.id);
        mcp_servers.insert(
            key,
            json!({
                "command": cmd,
                "args": m.args,
                "enabled": true
            }),
        );
    }
    mcp_servers
}

/// Build Pi `models.json` / `auth.json` / `settings.json` payloads from a validated LLM.
pub fn build_pi_llm_files(llm: &LlmEndpoint, skill_dirs: &[String]) -> (Value, Value, Value) {
    let base_url = llm.base_url.trim().trim_end_matches('/').to_string();
    let api_key = llm.api_key.clone();
    let model_id = llm.model.trim().to_string();
    let model_name = if llm.name.trim().is_empty() {
        model_id.clone()
    } else {
        llm.name.clone()
    };

    // `compat` is the pi-coding-agent models.json field name (provider capability flags).
    let models = json!({
        "providers": {
            PI_PROVIDER_ID: {
                "baseUrl": base_url,
                "api": "openai-completions",
                "apiKey": api_key,
                "compat": {
                    "supportsDeveloperRole": false,
                    "supportsReasoningEffort": false
                },
                "models": [{
                    "id": model_id,
                    "name": model_name,
                    "reasoning": false,
                    "input": ["text"],
                    "contextWindow": 128000,
                    "maxTokens": 8192,
                    "cost": {
                        "input": 0,
                        "output": 0,
                        "cacheRead": 0,
                        "cacheWrite": 0
                    }
                }]
            }
        }
    });
    let auth = json!({
        PI_PROVIDER_ID: { "type": "api_key", "key": api_key }
    });
    let settings_json = json!({
        "defaultProvider": PI_PROVIDER_ID,
        "defaultModel": model_id,
        "defaultThinkingLevel": "off",
        "defaultProjectTrust": "always",
        "enabledModels": [format!("{PI_PROVIDER_ID}/{model_id}")],
        "skills": skill_dirs
    });
    (models, auth, settings_json)
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, body.as_bytes()).map_err(|e| e.to_string())
}

/// Sync Monolith LLM / Skill / MCP settings into App Support `.pi-agent`.
/// Requires an enabled chat LLM — does not invent endpoints or keys.
pub fn sync_pi_config_from_settings(s: &AppSettings) -> Result<(), String> {
    let agent = pi_agent_dir()?;
    fs::create_dir_all(&agent).map_err(|e| e.to_string())?;

    let llm = require_active_chat_llm(s)?;
    let skills = skill_dirs(s);
    let (models, auth, settings_json) = build_pi_llm_files(llm, &skills);
    write_json_pretty(&agent.join("models.json"), &models)?;
    write_json_pretty(&agent.join("auth.json"), &auth)?;
    write_json_pretty(&agent.join("settings.json"), &settings_json)?;
    write_json_pretty(
        &agent.join("mcp.json"),
        &json!({ "mcpServers": Value::Object(build_mcp_servers_object(s)) }),
    )?;
    Ok(())
}

/// Parse `node -v` output (`v22.19.0`). Rejects unparseable versions.
pub fn parse_node_version(ver: &str) -> Result<(u32, u32, u32), String> {
    let trimmed = ver.trim().trim_start_matches('v');
    let mut parts = trimmed.split('.');
    let major: u32 = parts
        .next()
        .ok_or_else(|| format!("unparseable node version: {ver}"))?
        .parse()
        .map_err(|_| format!("unparseable node version: {ver}"))?;
    let minor: u32 = parts
        .next()
        .ok_or_else(|| format!("unparseable node version: {ver}"))?
        .parse()
        .map_err(|_| format!("unparseable node version: {ver}"))?;
    let patch: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Ok((major, minor, patch))
}

pub fn node_version_meets_min(ver: &str) -> Result<(), String> {
    let (major, minor, _) = parse_node_version(ver)?;
    if major < MIN_NODE_MAJOR || (major == MIN_NODE_MAJOR && minor < MIN_NODE_MINOR) {
        return Err(format!(
            "Node {ver} is too old; need ≥ {MIN_NODE_MAJOR}.{MIN_NODE_MINOR}.0"
        ));
    }
    Ok(())
}

/// Locate `node` via login shell PATH (macOS GUI apps often have a thin process PATH).
fn which_node() -> Result<PathBuf, String> {
    let out = Command::new("/bin/bash")
        .args(["-lc", "command -v node"])
        .output()
        .map_err(|e| format!("failed to locate node: {e}"))?;
    if !out.status.success() {
        return Err(
            "Node.js not found. Install Node ≥ 22.19 (https://nodejs.org/) and restart Monolith."
                .into(),
        );
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        return Err("Node.js not found on PATH.".into());
    }
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("node path from shell is not a file: {path}"));
    }
    Ok(p)
}

fn login_shell_path() -> Result<String, String> {
    let out = Command::new("/bin/bash")
        .args(["-lc", "printf %s \"$PATH\""])
        .output()
        .map_err(|e| format!("failed to read login PATH: {e}"))?;
    if !out.status.success() {
        return Err("failed to read login shell PATH".into());
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        return Err("login shell PATH is empty".into());
    }
    Ok(path)
}

fn check_node() -> Result<(PathBuf, String), String> {
    let node = which_node()?;
    let out = Command::new(&node)
        .arg("-v")
        .output()
        .map_err(|e| format!("node -v failed: {e}"))?;
    if !out.status.success() {
        return Err("node -v failed".into());
    }
    let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
    node_version_meets_min(&ver)?;
    Ok((node, ver))
}

/// Agent cwd = configured `assetsDir` (must be absolute). Created if missing.
pub fn resolve_workdir(s: &AppSettings) -> Result<PathBuf, String> {
    let d = s.assets_dir.trim();
    if d.is_empty() {
        return Err("assetsDir is empty; set Settings → 资产目录 before starting Pi Agent.".into());
    }
    let p = PathBuf::from(d);
    if !p.is_absolute() {
        return Err(format!("assetsDir must be absolute: {d}"));
    }
    fs::create_dir_all(&p).map_err(|e| format!("cannot create assetsDir {d}: {e}"))?;
    Ok(p)
}

pub fn bridge_base_url(port: u16) -> String {
    format!("http://{BRIDGE_HOST}:{port}")
}

fn health_url(port: u16) -> String {
    format!("{}/health", bridge_base_url(port))
}

fn probe_health(port: u16) -> Option<Value> {
    let url = health_url(port);
    ureq::get(&url)
        .timeout(Duration::from_secs(2))
        .call()
        .ok()
        .and_then(|r| r.into_json::<Value>().ok())
}

fn health_reports_ok(health: &Value) -> bool {
    health
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn wait_healthy(port: u16, attempts: u32) -> bool {
    for _ in 0..attempts {
        if probe_health(port).is_some_and(|h| health_reports_ok(&h)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn child_pid(state: &BridgeState) -> Option<u32> {
    let mut guard = state.child.lock().ok()?;
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                None
            }
            Ok(None) => Some(child.id()),
            Err(_) => None,
        }
    } else {
        None
    }
}

fn kill_listeners_on_port(port: u16) {
    let _ = Command::new("/bin/bash")
        .args([
            "-lc",
            &format!(
                "pids=$(lsof -tiTCP:{port} -sTCP:LISTEN 2>/dev/null); [ -n \"$pids\" ] && kill $pids 2>/dev/null; true"
            ),
        ])
        .status();
}

fn stop_managed_child(state: &BridgeState) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command]
pub fn agent_bridge_status(app: AppHandle, state: State<'_, BridgeState>) -> Result<BridgeStatus, String> {
    let s = load_settings()?;
    let port = bridge_port(&s);
    let agent_dir = pi_agent_dir()?.to_string_lossy().into_owned();
    let workdir = match resolve_workdir(&s) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(e) => e,
    };
    let bridge_script = match bridge_script(&app) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(e) => e,
    };

    let (node_ok, node_version, node_error) = match check_node() {
        Ok((_, v)) => (true, Some(v), None),
        Err(e) => (false, None, Some(e)),
    };

    let health = probe_health(port);
    let up = health.as_ref().is_some_and(health_reports_ok);
    let pid = child_pid(&state);

    let hint = if !node_ok {
        node_error.clone()
    } else if !up {
        Some("Pi Bridge is not running. Open the Agent float or use Start Bridge in Settings → LLM.".into())
    } else {
        None
    };

    Ok(BridgeStatus {
        up,
        port,
        bridge_url: bridge_base_url(port),
        health_url: health_url(port),
        pid,
        node_ok,
        node_version,
        node_error,
        agent_dir,
        workdir,
        bridge_script,
        health,
        hint,
    })
}

#[tauri::command]
pub fn agent_bridge_start(app: AppHandle, state: State<'_, BridgeState>) -> Result<BridgeStatus, String> {
    materialize_pi_runtime(&app)?;
    let s = load_settings()?;
    sync_pi_config_from_settings(&s)?;

    let port = bridge_port(&s);

    if probe_health(port).is_some() {
        stop_managed_child(&state)?;
        kill_listeners_on_port(port);
        std::thread::sleep(Duration::from_millis(300));
    }

    let (node, _) = check_node()?;
    let script = bridge_script(&app)?;
    if !script.is_file() {
        return Err(format!("bridge script missing: {}", script.display()));
    }
    let bridge_dir = script
        .parent()
        .ok_or_else(|| format!("bridge script has no parent: {}", script.display()))?;
    let nm = bridge_dir.join("node_modules/@earendil-works/pi-coding-agent");
    if !nm.exists() {
        return Err(
            "Pi Bridge node_modules missing. Run: npm run pi-agent:install (or scripts/pi-agent-install.sh)"
                .into(),
        );
    }

    let agent_dir = pi_agent_dir()?;
    let workdir = resolve_workdir(&s)?;
    let log_path = logs_dir()?.join("pi-bridge.log");
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("open log: {e}"))?;
    let log_err = log_file.try_clone().map_err(|e| e.to_string())?;
    let shell_path = login_shell_path()?;

    {
        stop_managed_child(&state)?;
        let mut guard = state.child.lock().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&node);
        cmd.arg(&script)
            .env("PI_BRIDGE_PORT", port.to_string())
            .env("PI_AGENT_WORKDIR", workdir.as_os_str())
            .env("PI_AGENT_DIR", agent_dir.as_os_str())
            .env("PATH", &shell_path)
            .current_dir(bridge_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err));
        let child = cmd.spawn().map_err(|e| format!("spawn bridge: {e}"))?;
        *guard = Some(child);
    }

    if !wait_healthy(port, 40) {
        let mut hint = format!(
            "Pi Bridge did not become healthy on port {port}. See {}",
            log_path.display()
        );
        if let Ok(tail) = fs::read_to_string(&log_path) {
            let lines: Vec<&str> = tail.lines().rev().take(12).collect();
            let chron: Vec<&str> = lines.into_iter().rev().collect();
            if !chron.is_empty() {
                hint.push_str("\n--- log ---\n");
                hint.push_str(&chron.join("\n"));
            }
        }
        return Err(hint);
    }

    agent_bridge_status(app, state)
}

#[tauri::command]
pub fn agent_bridge_stop(app: AppHandle, state: State<'_, BridgeState>) -> Result<BridgeStatus, String> {
    stop_managed_child(&state)?;
    let s = load_settings()?;
    kill_listeners_on_port(bridge_port(&s));
    agent_bridge_status(app, state)
}

/// Best-effort sync after settings save. Missing LLM is expected until the user configures one.
pub fn sync_after_settings_save(s: &AppSettings) {
    match sync_pi_config_from_settings(s) {
        Ok(()) => {}
        Err(e) if e.starts_with("No enabled chat LLM") => {}
        Err(e) => eprintln!("[monolith] pi config sync skipped: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{AgentMcpServer, AgentSkillEntry, LlmEndpoint};

    fn llm(base: &str, model: &str, key: &str) -> LlmEndpoint {
        LlmEndpoint {
            id: "llm-1".into(),
            name: "Test".into(),
            base_url: base.into(),
            api_key: key.into(),
            model: model.into(),
            role: "chat".into(),
            enabled: true,
        }
    }

    fn settings_with_llm(l: LlmEndpoint) -> AppSettings {
        let id = l.id.clone();
        AppSettings {
            active_chat_llm_id: id,
            llms: vec![l],
            assets_dir: "/tmp/monolith-assets-test".into(),
            ..AppSettings::default()
        }
    }

    #[test]
    fn skill_package_dir_from_skill_md() {
        assert_eq!(
            skill_package_dir("/tmp/skills/monolith-assets/SKILL.md").unwrap(),
            PathBuf::from("/tmp/skills/monolith-assets")
        );
        assert_eq!(
            skill_package_dir("/tmp/skills/monolith-assets").unwrap(),
            PathBuf::from("/tmp/skills/monolith-assets")
        );
        assert!(skill_package_dir("").is_none());
        assert!(skill_package_dir("   ").is_none());
    }

    #[test]
    fn skill_dirs_dedupe_and_skip_disabled() {
        let s = AppSettings {
            agent_skills: vec![
                AgentSkillEntry {
                    id: "a".into(),
                    name: "monolith-assets".into(),
                    source: "builtin".into(),
                    path: "/tmp/skills/monolith-assets/SKILL.md".into(),
                    remote_url: String::new(),
                    enabled: true,
                },
                AgentSkillEntry {
                    id: "b".into(),
                    name: "dup".into(),
                    source: "local".into(),
                    path: "/tmp/skills/monolith-assets".into(),
                    remote_url: String::new(),
                    enabled: true,
                },
                AgentSkillEntry {
                    id: "c".into(),
                    name: "off".into(),
                    source: "local".into(),
                    path: "/tmp/skills/other/SKILL.md".into(),
                    remote_url: String::new(),
                    enabled: false,
                },
            ],
            ..AppSettings::default()
        };
        assert_eq!(skill_dirs(&s), vec!["/tmp/skills/monolith-assets".to_string()]);
    }

    #[test]
    fn mcp_servers_object_sanitizes_keys_and_skips_empty_command() {
        let s = AppSettings {
            agent_mcp_servers: vec![
                AgentMcpServer {
                    id: "mcp-1".into(),
                    name: "Monolith assets".into(),
                    command: "/bin/monolith-mcp".into(),
                    args: vec!["--stdio".into()],
                    enabled: true,
                },
                AgentMcpServer {
                    id: "mcp-2".into(),
                    name: "empty".into(),
                    command: "  ".into(),
                    args: vec![],
                    enabled: true,
                },
                AgentMcpServer {
                    id: "mcp-3".into(),
                    name: "off".into(),
                    command: "/bin/x".into(),
                    args: vec![],
                    enabled: false,
                },
            ],
            ..AppSettings::default()
        };
        let map = build_mcp_servers_object(&s);
        assert_eq!(map.len(), 1);
        let entry = map.get("Monolith_assets").expect("key");
        assert_eq!(entry["command"], "/bin/monolith-mcp");
        assert_eq!(entry["args"], json!(["--stdio"]));
        assert_eq!(entry["enabled"], true);
    }

    #[test]
    fn require_active_chat_llm_rejects_missing_and_empty_fields() {
        let empty = AppSettings::default();
        assert!(require_active_chat_llm(&empty)
            .unwrap_err()
            .starts_with("No enabled chat LLM"));

        let mut s = settings_with_llm(llm("http://127.0.0.1:8000/v1", "m1", "k"));
        s.llms[0].base_url.clear();
        assert!(require_active_chat_llm(&s)
            .unwrap_err()
            .contains("empty baseUrl"));

        let mut s = settings_with_llm(llm("http://127.0.0.1:8000/v1", "m1", "k"));
        s.llms[0].model.clear();
        assert!(require_active_chat_llm(&s)
            .unwrap_err()
            .contains("empty model"));
    }

    #[test]
    fn build_pi_llm_files_uses_exact_llm_fields_no_invented_key() {
        let llm = llm("http://127.0.0.1:8000/v1/", "gemma-x", "");
        let (models, auth, settings) = build_pi_llm_files(&llm, &["/skills/a".into()]);
        assert_eq!(
            models["providers"]["monolith"]["baseUrl"],
            "http://127.0.0.1:8000/v1"
        );
        assert_eq!(models["providers"]["monolith"]["apiKey"], "");
        assert_eq!(models["providers"]["monolith"]["models"][0]["id"], "gemma-x");
        assert_eq!(auth["monolith"]["key"], "");
        assert_eq!(settings["defaultProvider"], "monolith");
        assert_eq!(settings["defaultModel"], "gemma-x");
        assert_eq!(settings["enabledModels"][0], "monolith/gemma-x");
        assert_eq!(settings["skills"][0], "/skills/a");
    }

    #[test]
    fn parse_and_check_node_version() {
        assert_eq!(parse_node_version("v22.19.0").unwrap(), (22, 19, 0));
        assert_eq!(parse_node_version("22.20.1").unwrap(), (22, 20, 1));
        assert!(parse_node_version("nope").is_err());
        assert!(node_version_meets_min("v22.19.0").is_ok());
        assert!(node_version_meets_min("v22.18.0").is_err());
        assert!(node_version_meets_min("v21.0.0").is_err());
        assert!(node_version_meets_min("v23.0.0").is_ok());
    }

    #[test]
    fn bridge_urls() {
        assert_eq!(bridge_base_url(8096), "http://127.0.0.1:8096");
        assert_eq!(health_url(8096), "http://127.0.0.1:8096/health");
    }

    #[test]
    fn health_reports_ok_requires_true() {
        assert!(health_reports_ok(&json!({"ok": true})));
        assert!(!health_reports_ok(&json!({"ok": false})));
        assert!(!health_reports_ok(&json!({})));
    }

    #[test]
    fn resolve_workdir_requires_absolute_assets_dir() {
        let mut s = AppSettings::default();
        s.assets_dir.clear();
        assert!(resolve_workdir(&s).unwrap_err().contains("empty"));

        s.assets_dir = "relative/path".into();
        assert!(resolve_workdir(&s).unwrap_err().contains("absolute"));

        let dir = std::env::temp_dir().join(format!(
            "monolith-pi-workdir-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        s.assets_dir = dir.to_string_lossy().into_owned();
        let got = resolve_workdir(&s).unwrap();
        assert_eq!(got, dir);
        assert!(dir.is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_pi_config_writes_files_when_llm_present() {
        let dir = std::env::temp_dir().join(format!(
            "monolith-pi-sync-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        // Override app data by writing into a temp agent dir via env is hard;
        // instead unit-test builders + write helpers in isolation:
        let agent = dir.join(".pi-agent");
        fs::create_dir_all(&agent).unwrap();
        let llm = llm("http://127.0.0.1:9000/v1", "m", "secret");
        let skills = vec!["/s/a".to_string()];
        let (models, auth, settings) = build_pi_llm_files(&llm, &skills);
        write_json_pretty(&agent.join("models.json"), &models).unwrap();
        write_json_pretty(&agent.join("auth.json"), &auth).unwrap();
        write_json_pretty(&agent.join("settings.json"), &settings).unwrap();
        let mcp = json!({ "mcpServers": Value::Object(Map::new()) });
        write_json_pretty(&agent.join("mcp.json"), &mcp).unwrap();

        let models_raw = fs::read_to_string(agent.join("models.json")).unwrap();
        assert!(models_raw.contains("9000"));
        assert!(models_raw.contains("secret"));
        assert!(!models_raw.contains("omlx"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bridge_port_zero_uses_default() {
        let mut s = AppSettings::default();
        s.agent_bridge_port = 0;
        assert_eq!(bridge_port(&s), settings::DEFAULT_BRIDGE_PORT);
        s.agent_bridge_port = 9099;
        assert_eq!(bridge_port(&s), 9099);
    }
}
