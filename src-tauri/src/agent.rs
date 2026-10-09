//! Agent helpers: enabled skill bodies, HTTP RAG retrieve, in-process MCP tool calls.

use crate::assets::AssetDb;
use crate::mcp_server;
use crate::settings::{load_settings, LlmEndpoint, RagStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillContent {
    pub id: String,
    pub name: String,
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagHit {
    pub text: String,
    pub score: Option<f64>,
    pub meta: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagRetrieveRequest {
    pub query: String,
    /// Optional store id; empty = all enabled http stores.
    #[serde(default)]
    pub store_id: String,
}

fn find_llm<'a>(llms: &'a [LlmEndpoint], id: &str) -> Option<&'a LlmEndpoint> {
    llms.iter().find(|l| l.id == id && l.enabled)
}

/// Load bodies of enabled skills that have a readable path.
#[tauri::command]
pub fn agent_skills_load() -> Result<Vec<AgentSkillContent>, String> {
    let s = load_settings()?;
    let mut out = Vec::new();
    for sk in s.agent_skills.iter().filter(|x| x.enabled) {
        let path = sk.path.trim();
        if path.is_empty() {
            continue;
        }
        let p = PathBuf::from(path);
        if !p.is_file() {
            continue;
        }
        let body = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        out.push(AgentSkillContent {
            id: sk.id.clone(),
            name: sk.name.clone(),
            path: path.to_string(),
            body,
        });
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolCallRequest {
    pub name: String,
    /// Stringly DSML params or JSON object; values are coerced (bool/number).
    #[serde(default)]
    pub args: Value,
}

/// Coerce DSML string params (`"true"`, `"12"`) into JSON types MCP tools expect.
fn coerce_tool_args(args: &Value) -> Value {
    let Some(obj) = args.as_object() else {
        return args.clone();
    };
    let mut out = Map::new();
    for (k, v) in obj {
        let next = match v {
            Value::String(s) => {
                let t = s.trim();
                if t.eq_ignore_ascii_case("true") {
                    Value::Bool(true)
                } else if t.eq_ignore_ascii_case("false") {
                    Value::Bool(false)
                } else if let Ok(i) = t.parse::<i64>() {
                    json!(i)
                } else if let Ok(f) = t.parse::<f64>() {
                    if t.contains('.') {
                        json!(f)
                    } else {
                        Value::String(s.clone())
                    }
                } else {
                    Value::String(s.clone())
                }
            }
            other => other.clone(),
        };
        out.insert(k.clone(), next);
    }
    Value::Object(out)
}

/// Run an MCP tool: monolith assets via in-process handlers; other servers via stdio client.
#[tauri::command]
pub fn agent_tool_call(
    db: State<'_, AssetDb>,
    req: AgentToolCallRequest,
) -> Result<Value, String> {
    let s = load_settings()?;
    let enabled: Vec<_> = s.agent_mcp_servers.iter().filter(|m| m.enabled).collect();
    if enabled.is_empty() {
        return Err("No enabled Agent MCP server in settings".into());
    }
    let name = req.name.trim();
    if name.is_empty() {
        return Err("tool name required".into());
    }
    let args = coerce_tool_args(&req.args);

    let monolith_on = enabled.iter().any(|m| crate::mcp_client::is_monolith_mcp(m));
    // Prefer in-process for asset_* when monolith MCP is enabled.
    if monolith_on && name.starts_with("asset_") {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        return mcp_server::call_tool(&conn, name, &args);
    }

    let mut last_err = String::new();
    for m in &enabled {
        if crate::mcp_client::is_monolith_mcp(m) {
            continue;
        }
        match crate::mcp_client::call_tool_external(m, name, &args) {
            Ok(v) => return Ok(v),
            Err(e) => last_err = format!("{}: {e}", m.name),
        }
    }

    if monolith_on {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        return mcp_server::call_tool(&conn, name, &args).map_err(|e| {
            if last_err.is_empty() {
                e
            } else {
                format!("{e} (also tried external: {last_err})")
            }
        });
    }
    Err(if last_err.is_empty() {
        format!("No MCP server could run tool «{name}»")
    } else {
        last_err
    })
}

/// List tools from every enabled MCP (monolith in-process + external stdio).
#[tauri::command]
pub fn agent_mcp_tools() -> Result<Value, String> {
    let s = load_settings()?;
    let mut out = Vec::new();
    for m in s.agent_mcp_servers.iter().filter(|m| m.enabled) {
        if crate::mcp_client::is_monolith_mcp(m) {
            let tools: Vec<Value> = mcp_server::tools_for_agent()
                .into_iter()
                .map(|(name, description)| {
                    json!({
                        "name": name,
                        "description": description,
                    })
                })
                .collect();
            out.push(json!({
                "id": m.id,
                "name": m.name,
                "kind": "monolith",
                "tools": tools,
            }));
            continue;
        }
        match crate::mcp_client::list_tools_external(m) {
            Ok(tools) => {
                let slim: Vec<Value> = tools
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.get("name").cloned().unwrap_or(json!("")),
                            "description": t.get("description").cloned().unwrap_or(json!("")),
                        })
                    })
                    .collect();
                out.push(json!({
                    "id": m.id,
                    "name": m.name,
                    "kind": "external",
                    "tools": slim,
                }));
            }
            Err(e) => {
                out.push(json!({
                    "id": m.id,
                    "name": m.name,
                    "kind": "external",
                    "error": e,
                    "tools": [],
                }));
            }
        }
    }
    Ok(json!({ "servers": out }))
}

/// Snapshot of what the embedded Agent will actually use (skills + MCP flags).
#[tauri::command]
pub fn agent_runtime_status() -> Result<Value, String> {
    let s = load_settings()?;
    let skills = agent_skills_load()?;
    let mcp: Vec<Value> = s
        .agent_mcp_servers
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "name": m.name,
                "enabled": m.enabled,
                "command": m.command,
            })
        })
        .collect();
    Ok(json!({
        "skillsLoaded": skills.iter().map(|sk| json!({
            "id": sk.id,
            "name": sk.name,
            "path": sk.path,
            "bytes": sk.body.len(),
        })).collect::<Vec<_>>(),
        "skillsConfigured": s.agent_skills.len(),
        "mcpServers": mcp,
        "bridgePort": crate::settings::bridge_port(&s),
        "piAgentDir": crate::agent_bridge::pi_agent_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }))
}

/// HTTP RAG retrieve. `queryMode=text` POSTs `{query,topK}`; `vector` embeds via active embed LLM then POSTs `{vector,topK}`.
#[tauri::command]
pub fn agent_rag_retrieve(req: RagRetrieveRequest) -> Result<Vec<RagHit>, String> {
    let q = req.query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let s = load_settings()?;
    let stores: Vec<&RagStore> = s
        .rag_stores
        .iter()
        .filter(|r| {
            r.enabled
                && rag_kind_known(&r.kind)
                && (req.store_id.is_empty() || r.id == req.store_id)
        })
        .collect();
    if stores.is_empty() {
        return Ok(vec![]);
    }

    let mut hits = Vec::new();
    for store in stores {
        let batch = if rag_kind_http_retrieve(&store.kind) {
            retrieve_one(store, q, &s.llms, &s.active_embed_llm_id)?
        } else {
            // Native drivers (pgvector / LanceDB path): retrieve lands with dedicated adapters.
            Vec::new()
        };
        hits.extend(batch);
    }
    Ok(hits)
}

fn rag_kind_known(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "http"
            | "pgvector"
            | "qdrant"
            | "milvus"
            | "chroma"
            | "weaviate"
            | "lancedb"
            | "elasticsearch"
    )
}

/// Engines that speak an HTTP search API (generic `{query|vector,topK}` adapter).
fn rag_kind_http_retrieve(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "http" | "qdrant" | "milvus" | "chroma" | "weaviate" | "elasticsearch"
    )
}

/// Probe one RAG store by engine (used by settings「测试连接」).
#[tauri::command]
pub fn agent_rag_test(store_id: String) -> Result<String, String> {
    let s = load_settings()?;
    let store = s
        .rag_stores
        .iter()
        .find(|r| r.id == store_id)
        .ok_or_else(|| format!("RAG store not found: {store_id}"))?;
    probe_rag_store(store, &s.llms, &s.active_embed_llm_id)
}

/// Engine-specific connectivity check. Prefer product health paths — never treat
/// a generic `/` 200 from an unrelated service as success.
pub(crate) fn probe_rag_store(
    store: &RagStore,
    llms: &[LlmEndpoint],
    embed_id: &str,
) -> Result<String, String> {
    match store.kind.trim() {
        "pgvector" => test_pgvector(store),
        "lancedb" => test_lancedb(store),
        "qdrant" => test_http_ready(store, &["/collections"]),
        "chroma" => test_http_ready(store, &["/api/v2/heartbeat", "/api/v1/heartbeat"]),
        "weaviate" => test_http_ready(store, &["/v1/.well-known/ready", "/v1/meta"]),
        "elasticsearch" => test_http_ready(store, &["/_cluster/health"]),
        "milvus" => test_http_ready(store, &["/healthz", "/v1/vector/collections"]),
        "http" => {
            let hits = retrieve_one(store, "ping", llms, embed_id)?;
            Ok(if hits.is_empty() {
                "ok · HTTP Search reachable (0 hits)".into()
            } else {
                format!("ok · HTTP Search {} hit(s)", hits.len())
            })
        }
        other => Err(format!("unsupported RAG kind: {other}")),
    }
}

fn test_pgvector(store: &RagStore) -> Result<String, String> {
    let conn = store.endpoint.trim();
    if conn.is_empty() {
        return Err("pgvector connection string empty".into());
    }
    let out = std::process::Command::new("psql")
        .arg(conn)
        .args([
            "-v",
            "ON_ERROR_STOP=1",
            "-tAc",
            "SELECT COALESCE((SELECT extversion FROM pg_extension WHERE extname = 'vector'), '')",
        ])
        .output()
        .map_err(|e| format!("psql not runnable: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("pgvector probe failed: {}", err.trim()));
    }
    let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if ver.is_empty() {
        return Err("connected, but extension «vector» is not installed in this database".into());
    }
    Ok(format!("ok · pgvector {ver}"))
}

fn test_lancedb(store: &RagStore) -> Result<String, String> {
    let p = store.endpoint.trim();
    if p.is_empty() {
        return Err("LanceDB path empty".into());
    }
    let path = std::path::Path::new(p);
    if !path.exists() {
        return Err(format!("LanceDB path not found: {p}"));
    }
    Ok(format!(
        "ok · LanceDB path ({})",
        if path.is_dir() { "dir" } else { "file" }
    ))
}

fn test_http_ready(store: &RagStore, paths: &[&str]) -> Result<String, String> {
    let base = store.endpoint.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("endpoint empty".into());
    }
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err("endpoint must be an http(s) URL for this engine".into());
    }
    let mut last_err = String::new();
    for path in paths {
        let url = if *path == "/" {
            base.to_string()
        } else {
            format!("{base}{path}")
        };
        let mut req = ureq::get(&url);
        if !store.api_key.trim().is_empty() {
            req = req.set(
                "Authorization",
                &format!("Bearer {}", store.api_key.trim()),
            );
        }
        match req.call() {
            Ok(resp) => {
                let code = resp.status();
                // Only 2xx counts — a random service answering 404/405 on `/` is not "ok".
                if (200..300).contains(&code) {
                    return Ok(format!("ok · {} HTTP {code}", store.kind));
                }
                last_err = format!("HTTP {code} at {url}");
            }
            Err(e) => last_err = format!("{url}: {e}"),
        }
    }
    Err(format!("{} probe failed: {last_err}", store.kind))
}

fn retrieve_one(
    store: &RagStore,
    query: &str,
    llms: &[LlmEndpoint],
    embed_id: &str,
) -> Result<Vec<RagHit>, String> {
    let endpoint = store.endpoint.trim();
    if endpoint.is_empty() {
        return Err(format!("RAG store {} has empty endpoint", store.id));
    }
    let body = if store.query_mode == "vector" {
        let embed_llm = find_llm(llms, embed_id)
            .or_else(|| llms.iter().find(|l| l.enabled && (l.role == "embed" || l.role == "both")))
            .ok_or_else(|| "No enabled embedding LLM configured".to_string())?;
        let vector = embed_query(embed_llm, query)?;
        json!({ "vector": vector, "topK": store.top_k, "query": query })
    } else {
        json!({ "query": query, "topK": store.top_k })
    };

    let mut request = ureq::post(endpoint).set("Content-Type", "application/json");
    if !store.api_key.trim().is_empty() {
        request = request.set("Authorization", &format!("Bearer {}", store.api_key.trim()));
    }
    let resp = request
        .send_json(body)
        .map_err(|e| format!("RAG HTTP {}: {e}", store.name))?;
    let val: Value = resp.into_json().map_err(|e| e.to_string())?;
    Ok(parse_rag_hits(&val))
}

fn embed_query(llm: &LlmEndpoint, text: &str) -> Result<Vec<f32>, String> {
    let base = llm.base_url.trim().trim_end_matches('/');
    let url = format!("{base}/embeddings");
    let mut request = ureq::post(&url).set("Content-Type", "application/json");
    let key = llm.api_key.trim();
    let local = llm.base_url.contains("127.0.0.1")
        || llm.base_url.contains("localhost")
        || llm.base_url.contains("[::1]");
    let bearer = if !key.is_empty() {
        key
    } else if local {
        "local"
    } else {
        ""
    };
    if !bearer.is_empty() {
        request = request.set("Authorization", &format!("Bearer {bearer}"));
    }
    let model = if llm.model.trim().is_empty() {
        "text-embedding"
    } else {
        llm.model.trim()
    };
    let resp = request
        .send_json(json!({ "model": model, "input": text }))
        .map_err(|e| format!("embed failed: {e}"))?;
    let val: Value = resp.into_json().map_err(|e| e.to_string())?;
    let arr = val
        .pointer("/data/0/embedding")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "embed response missing data[0].embedding".to_string())?;
    arr.iter()
        .map(|x| {
            x.as_f64()
                .map(|f| f as f32)
                .ok_or_else(|| "non-numeric embedding".to_string())
        })
        .collect()
}

fn parse_rag_hits(val: &Value) -> Vec<RagHit> {
    let items = val
        .get("hits")
        .or_else(|| val.get("results"))
        .or_else(|| val.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in items {
        let text = item
            .get("text")
            .or_else(|| item.get("content"))
            .or_else(|| item.get("document"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if text.trim().is_empty() {
            continue;
        }
        let score = item.get("score").and_then(|v| v.as_f64());
        out.push(RagHit {
            text,
            score,
            meta: item.get("meta").cloned(),
        });
    }
    out
}

/// Active chat LLM snapshot for the UI (no secrets beyond what settings already store).
#[tauri::command]
pub fn agent_active_chat_llm() -> Result<Option<LlmEndpoint>, String> {
    let s = load_settings()?;
    Ok(find_llm(&s.llms, &s.active_chat_llm_id).cloned().or_else(|| {
        s.llms
            .into_iter()
            .find(|l| l.enabled && (l.role == "chat" || l.role == "both"))
    }))
}

#[cfg(test)]
mod rag_probe_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    /// Serialize probes that bind ephemeral listeners (avoid cross-talk under parallel tests).
    static PROBE_LOCK: Mutex<()> = Mutex::new(());

    fn store(kind: &str, endpoint: &str) -> RagStore {
        RagStore {
            id: format!("rag-{kind}"),
            name: kind.into(),
            kind: kind.into(),
            endpoint: endpoint.into(),
            api_key: String::new(),
            query_mode: "text".into(),
            enabled: true,
            top_k: 5,
        }
    }

    /// Tiny multi-route mock covering health paths + HTTP Search POST.
    fn spawn_mock() -> (String, Arc<AtomicBool>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("nonblocking");
            while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                        let mut buf = [0u8; 8192];
                        let n = stream.read(&mut buf).unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let (code, body) = if req.starts_with("POST") {
                            (
                                200,
                                r#"{"hits":[{"text":"mock-hit","score":0.91}]}"#,
                            )
                        } else if req.contains("/collections") {
                            (200, r#"{"result":[]}"#)
                        } else if req.contains("heartbeat") {
                            (200, r#"{"nanosecond heartbeat":1}"#)
                        } else if req.contains(".well-known/ready") || req.contains("/v1/meta") {
                            (200, r#"{}"#)
                        } else if req.contains("/_cluster/health") {
                            (200, r#"{"status":"green"}"#)
                        } else if req.contains("/healthz") || req.contains("/v1/vector/collections")
                        {
                            (200, "OK")
                        } else {
                            (404, "missing")
                        };
                        let resp = format!(
                            "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        });
        (format!("http://{addr}"), stop, handle)
    }

    #[test]
    fn probe_each_http_engine_against_mock() {
        let _guard = PROBE_LOCK.lock().unwrap();
        let (base, stop, handle) = spawn_mock();
        thread::sleep(Duration::from_millis(50));
        let cases = [
            ("qdrant", base.clone()),
            ("chroma", base.clone()),
            ("weaviate", base.clone()),
            ("elasticsearch", base.clone()),
            ("milvus", base.clone()),
            ("http", format!("{base}/search")),
        ];
        let mut errors = Vec::new();
        for (kind, ep) in cases {
            match probe_rag_store(&store(kind, &ep), &[], "") {
                Ok(msg) => {
                    assert!(
                        msg.starts_with("ok"),
                        "{kind} expected ok, got {msg}"
                    );
                }
                Err(e) => errors.push(format!("{kind}: {e}")),
            }
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
        assert!(errors.is_empty(), "probe failures: {errors:?}");
    }

    #[test]
    fn probe_lancedb_path() {
        let dir = std::env::temp_dir().join(format!("monolith-lancedb-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let msg = probe_rag_store(
            &store("lancedb", dir.to_str().unwrap()),
            &[],
            "",
        )
        .expect("lancedb path");
        assert!(msg.contains("LanceDB"), "{msg}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_pgvector_local() {
        let ep = std::env::var("MONOLITH_TEST_PG")
            .unwrap_or_else(|_| "postgresql://muqiang@127.0.0.1:5432/postgres".into());
        let msg = probe_rag_store(&store("pgvector", &ep), &[], "").expect("pgvector");
        assert!(msg.starts_with("ok · pgvector"), "{msg}");
    }

    #[test]
    fn probe_rejects_unrelated_root_200() {
        let _guard = PROBE_LOCK.lock().unwrap();
        // Server that only answers `/` with 200 — chroma must NOT accept this.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = thread::spawn(move || {
            listener.set_nonblocking(true).ok();
            while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 2048];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first = req.lines().next().unwrap_or("");
                    // Only exact GET / — health paths must 404 (don't confuse with oMLX etc.).
                    let (code, body) = if first.starts_with("GET / HTTP/") || first == "GET /" {
                        (200, "ok")
                    } else {
                        (404, "no")
                    };
                    let resp = format!(
                        "HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                } else {
                    thread::sleep(Duration::from_millis(15));
                }
            }
        });
        thread::sleep(Duration::from_millis(40));
        let base = format!("http://{addr}");
        let err = probe_rag_store(&store("chroma", &base), &[], "").unwrap_err();
        assert!(err.contains("probe failed"), "{err}");
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
    }
}
