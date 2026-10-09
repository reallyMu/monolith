//! Minimal MCP stdio **client** for Agent → external MCP servers (Content-Length framing).

use crate::mcp_server::{read_message, write_message};
use crate::settings::AgentMcpServer;
use serde_json::{json, Value};
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const RPC_TIMEOUT: Duration = Duration::from_secs(25);

pub fn is_monolith_mcp(m: &AgentMcpServer) -> bool {
    let cmd = m.command.to_ascii_lowercase();
    let name = m.name.to_ascii_lowercase();
    let id = m.id.to_ascii_lowercase();
    cmd.contains("monolith-mcp")
        || name.contains("monolith")
        || id.contains("monolith")
}

pub struct McpSession {
    child: Child,
    stdin: ChildStdin,
    /// stdout reader kept in Option so we can move it into a timeout thread.
    stdout: Option<BufReader<std::process::ChildStdout>>,
    next_id: u64,
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl McpSession {
    pub fn start(command: &str, args: &[String]) -> Result<Self, String> {
        if command.trim().is_empty() {
            return Err("MCP command is empty".into());
        }
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn MCP «{command}»: {e}"))?;
        let stdin = child.stdin.take().ok_or("MCP stdin missing")?;
        let stdout = child.stdout.take().ok_or("MCP stdout missing")?;
        let mut session = Self {
            child,
            stdin,
            stdout: Some(BufReader::new(stdout)),
            next_id: 1,
        };
        session.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "monolith-agent", "version": "0.3.0" }
            }),
        )?;
        session.notify("notifications/initialized", json!({}))?;
        Ok(session)
    }

    pub fn start_from(m: &AgentMcpServer) -> Result<Self, String> {
        Self::start(&m.command, &m.args)
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        write_message(&mut self.stdin, &msg).map_err(|e| e.to_string())?;
        let _ = self.stdin.flush();
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        write_message(&mut self.stdin, &msg).map_err(|e| format!("write {method}: {e}"))?;
        let _ = self.stdin.flush();

        let mut stdout = self
            .stdout
            .take()
            .ok_or_else(|| "MCP stdout missing".to_string())?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let r = read_message(&mut stdout).map_err(|e| e.to_string());
            let _ = tx.send((r, stdout));
        });

        match rx.recv_timeout(RPC_TIMEOUT) {
            Ok((Ok(Some(resp)), stdout)) => {
                self.stdout = Some(stdout);
                if let Some(err) = resp.get("error") {
                    return Err(format!(
                        "MCP {method} error: {}",
                        err.get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or(&err.to_string())
                    ));
                }
                resp.get("result")
                    .cloned()
                    .ok_or_else(|| format!("MCP {method}: missing result"))
            }
            Ok((Ok(None), stdout)) => {
                self.stdout = Some(stdout);
                Err(format!("MCP closed during {method}"))
            }
            Ok((Err(e), stdout)) => {
                self.stdout = Some(stdout);
                Err(format!("MCP read {method}: {e}"))
            }
            Err(_) => {
                let _ = self.child.kill();
                Err(format!("MCP RPC timeout ({method}, {}s)", RPC_TIMEOUT.as_secs()))
            }
        }
    }

    pub fn list_tools(&mut self) -> Result<Vec<Value>, String> {
        let result = self.request("tools/list", json!({}))?;
        Ok(result
            .get("tools")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default())
    }

    pub fn call_tool(&mut self, name: &str, arguments: &Value) -> Result<Value, String> {
        let result = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )?;
        if result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false) {
            let text = result
                .pointer("/content/0/text")
                .and_then(|v| v.as_str())
                .unwrap_or("tool error");
            return Err(text.to_string());
        }
        if let Some(sc) = result.get("structuredContent") {
            return Ok(sc.clone());
        }
        if let Some(text) = result.pointer("/content/0/text") {
            if let Some(s) = text.as_str() {
                if let Ok(v) = serde_json::from_str::<Value>(s) {
                    return Ok(v);
                }
                return Ok(json!({ "text": s }));
            }
        }
        Ok(result)
    }
}

pub fn list_tools_external(m: &AgentMcpServer) -> Result<Vec<Value>, String> {
    let mut s = McpSession::start_from(m)?;
    s.list_tools()
}

pub fn call_tool_external(m: &AgentMcpServer, name: &str, args: &Value) -> Result<Value, String> {
    let mut s = McpSession::start_from(m)?;
    s.call_tool(name, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn monolith_mcp_bin() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        let p = PathBuf::from(home)
            .join("Library/Application Support/com.muqiang.monolith/bin/monolith-mcp");
        p.is_file().then_some(p)
    }

    #[test]
    fn list_tools_from_monolith_mcp_stdio() {
        let Some(bin) = monolith_mcp_bin() else {
            eprintln!("skip: monolith-mcp binary missing");
            return;
        };
        let m = AgentMcpServer {
            id: "ext-test".into(),
            name: "external-monolith".into(),
            command: bin.to_string_lossy().into_owned(),
            args: vec![],
            enabled: true,
        };
        // Not classified as in-process monolith by command path ending in monolith-mcp —
        // is_monolith_mcp returns true; still fine for stdio list smoke.
        let tools = list_tools_external(&m).expect("list_tools");
        assert!(
            tools.iter().any(|t| t.get("name").and_then(|n| n.as_str()) == Some("asset_search")),
            "expected asset_search in {tools:?}"
        );
    }
}
