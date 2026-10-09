//! App-level settings. Stored as JSON under App Support.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";

/// Default Pi Bridge listen port (loopback). Single source for settings + agent_bridge.
pub const DEFAULT_BRIDGE_PORT: u16 = 8096;

fn default_bridge_port() -> u16 {
    DEFAULT_BRIDGE_PORT
}

/// Resolve bridge port from settings (0 is invalid → default).
pub fn bridge_port(s: &AppSettings) -> u16 {
    if s.agent_bridge_port == 0 {
        DEFAULT_BRIDGE_PORT
    } else {
        s.agent_bridge_port
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmEndpoint {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    /// `chat` | `embed` | `both`
    #[serde(default = "default_chat_role")]
    pub role: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_chat_role() -> String {
    "chat".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagStore {
    pub id: String,
    pub name: String,
    /// Engine id: `http` | `pgvector` | `qdrant` | `milvus` | `chroma` | `weaviate` | `lancedb` | `elasticsearch`
    #[serde(default = "default_rag_kind")]
    pub kind: String,
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    /// `text` | `vector`
    #[serde(default = "default_query_mode")]
    pub query_mode: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_top_k")]
    pub top_k: u32,
}

fn default_rag_kind() -> String {
    "http".into()
}
fn default_query_mode() -> String {
    "text".into()
}
fn default_top_k() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMcpServer {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillEntry {
    pub id: String,
    pub name: String,
    /// `builtin` | `local` | `remote`
    pub source: String,
    #[serde(default)]
    pub path: String,
    /// URL or git remote (P1 pull)
    #[serde(default)]
    pub remote_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub inbox_dir: String,
    /// Default disk folder for Agent/MCP **new** asset files (register after write).
    #[serde(default)]
    pub assets_dir: String,
    pub default_open_dir: String,
    pub default_save_dir: String,
    #[serde(default)]
    pub llms: Vec<LlmEndpoint>,
    #[serde(default)]
    pub active_chat_llm_id: String,
    #[serde(default)]
    pub active_embed_llm_id: String,
    #[serde(default)]
    pub rag_stores: Vec<RagStore>,
    #[serde(default)]
    pub agent_mcp_servers: Vec<AgentMcpServer>,
    #[serde(default)]
    pub agent_skills: Vec<AgentSkillEntry>,
    #[serde(default = "default_bridge_port")]
    pub agent_bridge_port: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let downloads = home.join("Downloads");
        let documents = home.join("Documents");
        let mut s = Self {
            inbox_dir: downloads
                .join("MonolithInbox")
                .to_string_lossy()
                .into_owned(),
            assets_dir: documents
                .join("MonolithAssets")
                .to_string_lossy()
                .into_owned(),
            default_open_dir: String::new(),
            default_save_dir: String::new(),
            llms: Vec::new(),
            active_chat_llm_id: String::new(),
            active_embed_llm_id: String::new(),
            rag_stores: Vec::new(),
            agent_mcp_servers: Vec::new(),
            agent_skills: Vec::new(),
            agent_bridge_port: default_bridge_port(),
        };
        ensure_agent_defaults(&mut s);
        s
    }
}

/// Same kind + endpoint (trimmed) → keep the first row only. Multiple distinct
/// RAG libraries remain allowed.
fn dedupe_rag_stores(stores: &mut Vec<RagStore>) {
    let mut seen = std::collections::HashSet::<String>::new();
    stores.retain(|r| {
        let key = format!(
            "{}|{}",
            r.kind.trim().to_ascii_lowercase(),
            r.endpoint.trim()
        );
        if key.ends_with('|') {
            return true; // empty endpoint: let UI validate, don't collapse blanks together
        }
        seen.insert(key)
    });
}

/// Fill empty MCP / Skill lists. LLMs are never auto-injected — user adds any
/// OpenAI-compatible endpoint (oMLX, Ollama, cloud, etc.).
pub fn ensure_agent_defaults(s: &mut AppSettings) {
    dedupe_rag_stores(&mut s.rag_stores);
    // Drop stale active ids that no longer exist (e.g. after user removes a row).
    if !s.active_chat_llm_id.is_empty()
        && !s.llms.iter().any(|l| l.id == s.active_chat_llm_id)
    {
        s.active_chat_llm_id.clear();
    }
    if !s.active_embed_llm_id.is_empty()
        && !s.llms.iter().any(|l| l.id == s.active_embed_llm_id)
    {
        s.active_embed_llm_id.clear();
    }
    if s.active_chat_llm_id.trim().is_empty() {
        s.active_chat_llm_id = s
            .llms
            .iter()
            .find(|l| l.enabled && (l.role == "chat" || l.role == "both"))
            .map(|l| l.id.clone())
            .unwrap_or_else(|| s.llms.first().map(|l| l.id.clone()).unwrap_or_default());
    }
    if s.active_embed_llm_id.trim().is_empty() {
        s.active_embed_llm_id = s
            .llms
            .iter()
            .find(|l| l.enabled && (l.role == "embed" || l.role == "both"))
            .map(|l| l.id.clone())
            .unwrap_or_else(|| s.active_chat_llm_id.clone());
    }
    if s.agent_mcp_servers.is_empty() {
        let mcp = app_data_dir()
            .map(|d| d.join("bin/monolith-mcp").to_string_lossy().into_owned())
            .unwrap_or_else(|_| "monolith-mcp".into());
        s.agent_mcp_servers = vec![AgentMcpServer {
            id: "mcp-monolith".into(),
            name: "Monolith assets".into(),
            command: mcp,
            args: vec![],
            enabled: true,
        }];
    }
    if s.agent_skills.is_empty() {
        let builtin = app_data_dir()
            .map(|d| {
                d.join("skills/monolith-assets/SKILL.md")
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default();
        s.agent_skills = vec![AgentSkillEntry {
            id: "skill-monolith-assets".into(),
            name: "monolith-assets".into(),
            source: "builtin".into(),
            path: builtin,
            remote_url: String::new(),
            enabled: true,
        }];
    }
    if s.agent_bridge_port == 0 {
        s.agent_bridge_port = default_bridge_port();
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub settings: AppSettings,
    pub assets_db_path: String,
    pub app_data_dir: String,
    pub mcp_binary_path: String,
    pub clipper_release_dir: String,
    pub skills_dir: String,
}

pub(crate) fn app_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME not set".to_string())?;
    Ok(PathBuf::from(home).join("Library/Application Support/com.muqiang.monolith"))
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join(SETTINGS_FILE))
}

pub fn load_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.is_file() {
        return Ok(AppSettings::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut s: AppSettings = serde_json::from_str(&raw).unwrap_or_default();
    let def = AppSettings::default();
    if s.inbox_dir.trim().is_empty() {
        s.inbox_dir = def.inbox_dir;
    }
    if s.assets_dir.trim().is_empty() {
        s.assets_dir = def.assets_dir;
    }
    ensure_agent_defaults(&mut s);
    Ok(s)
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let dir = app_data_dir()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(SETTINGS_FILE);
    let pretty = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, pretty + "\n").map_err(|e| e.to_string())
}

pub fn resolve_inbox_dir() -> Result<PathBuf, String> {
    let s = load_settings()?;
    let dir = PathBuf::from(s.inbox_dir.trim());
    if dir.as_os_str().is_empty() {
        return Err("inbox_dir is empty".into());
    }
    if !dir.is_absolute() {
        return Err("inbox_dir must be an absolute path".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Agent/MCP default folder for newly created asset files.
pub fn resolve_assets_dir() -> Result<PathBuf, String> {
    let s = load_settings()?;
    let dir = PathBuf::from(s.assets_dir.trim());
    if dir.as_os_str().is_empty() {
        return Err("assets_dir is empty".into());
    }
    if !dir.is_absolute() {
        return Err("assets_dir must be an absolute path".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn validate_optional_dir(label: &str, value: &str) -> Result<(), String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(());
    }
    let p = Path::new(t);
    if !p.is_absolute() {
        return Err(format!("{label} must be an absolute path"));
    }
    Ok(())
}

#[tauri::command]
pub fn settings_get() -> Result<SettingsView, String> {
    let settings = load_settings()?;
    let data = app_data_dir()?;
    let assets = crate::assets::default_assets_db_path()?;
    let mcp = data.join("bin/monolith-mcp");
    let clipper = crate::clipper::clipper_release_dir().unwrap_or_else(|_| data.join("MonolithClipper"));
    let skills = data.join("skills");
    Ok(SettingsView {
        settings,
        assets_db_path: assets.to_string_lossy().into_owned(),
        app_data_dir: data.to_string_lossy().into_owned(),
        mcp_binary_path: mcp.to_string_lossy().into_owned(),
        clipper_release_dir: clipper.to_string_lossy().into_owned(),
        skills_dir: skills.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn settings_save(settings: AppSettings) -> Result<SettingsView, String> {
    let mut s = settings;
    s.inbox_dir = s.inbox_dir.trim().to_string();
    s.assets_dir = s.assets_dir.trim().to_string();
    s.default_open_dir = s.default_open_dir.trim().to_string();
    s.default_save_dir = s.default_save_dir.trim().to_string();
    if s.inbox_dir.is_empty() {
        return Err("导入目录不能为空".into());
    }
    if s.assets_dir.is_empty() {
        return Err("资产目录不能为空".into());
    }
    let inbox = PathBuf::from(&s.inbox_dir);
    if !inbox.is_absolute() {
        return Err("导入目录必须是绝对路径".into());
    }
    let assets = PathBuf::from(&s.assets_dir);
    if !assets.is_absolute() {
        return Err("资产目录必须是绝对路径".into());
    }
    validate_optional_dir("default_open_dir", &s.default_open_dir)?;
    validate_optional_dir("default_save_dir", &s.default_save_dir)?;
    ensure_agent_defaults(&mut s);
    fs::create_dir_all(&inbox).map_err(|e| format!("无法创建导入目录: {e}"))?;
    fs::create_dir_all(&assets).map_err(|e| format!("无法创建资产目录: {e}"))?;
    let _ = fs::create_dir_all(app_data_dir()?.join("skills"));
    save_settings(&s)?;
    crate::agent_bridge::sync_after_settings_save(&s);
    settings_get()
}
