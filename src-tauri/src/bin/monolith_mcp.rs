//! Monolith MCP stdio server — see docs/superpowers/specs/2026-10-05-monolith-mcp-assets-design.md

fn main() {
    if let Err(e) = monolith_lib::mcp_server::run_stdio() {
        eprintln!("monolith-mcp error: {e}");
        std::process::exit(1);
    }
}
