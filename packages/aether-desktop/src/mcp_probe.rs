use crate::state::McpServerStatus;
use aether::mcp::{RawMcpConfig, RawMcpServerConfig};
use rmcp::transport::auth::OAuthState;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Probe all MCP servers from the mcp.json config file.
///
/// For HTTP servers, attempts to determine connection status:
/// - Connected: Has valid cached credentials
/// - NeedsOAuth: Server requires OAuth and we don't have valid credentials
/// - Failed: Connection error
///
/// For Stdio/InMemory servers, returns a placeholder status since
/// we can't probe them without running them.
pub async fn probe_mcp_servers(project_path: &Path) -> HashMap<String, McpServerStatus> {
    let config_path = project_path.join("mcp.json");

    let config = match RawMcpConfig::from_json_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            debug!("Failed to parse mcp.json: {}", e);
            return HashMap::new();
        }
    };

    info!("Probing {} MCP servers from mcp.json", config.servers.len());

    let mut results = HashMap::new();

    for (name, server_config) in config.servers {
        let status = match &server_config {
            RawMcpServerConfig::Http { url, .. } | RawMcpServerConfig::Sse { url, .. } => {
                probe_http_server(&name, url).await
            }
            RawMcpServerConfig::InMemory { .. } => McpServerStatus::Connected,
            RawMcpServerConfig::Stdio { .. } => {
                // Stdio servers spawn a subprocess - we can't probe without running
                // Mark as connected since they don't require OAuth setup
                McpServerStatus::Connected
            }
        };

        info!("MCP server '{}' status: {:?}", name, status);
        results.insert(name, status);
    }

    results
}

/// Probe a single HTTP MCP server to determine its connection status.
async fn probe_http_server(server_id: &str, base_url: &str) -> McpServerStatus {
    match OAuthState::new(base_url, None).await {
        Ok(OAuthState::Unauthorized(_)) => {
            debug!("Server '{}' requires OAuth", server_id);
            McpServerStatus::NeedsOAuth {
                server_id: server_id.to_string(),
                base_url: base_url.to_string(),
            }
        }
        Ok(_) => {
            debug!("Server '{}' is authorized without OAuth", server_id);
            McpServerStatus::Connected
        }
        Err(e) => {
            warn!("Failed to probe server '{}': {}", server_id, e);
            McpServerStatus::Failed {
                error: e.to_string(),
            }
        }
    }
}
