mod commands;
mod state;

use commands::*;
use state::AgentState;
use aether_core::mcp::{McpClient, mcp_config::McpServerConfig};
use aether_core::tools::ToolRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[tokio::main]
pub async fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("async_openai=debug,aether_core=debug,reqwest=trace")
        .init();


    // Initialize MCP client (for now just to verify connection works)
    let mut mcp_client = McpClient::new();
    mcp_client.connect_server("mcp-mesh".to_string(), McpServerConfig::Http { url: "http://localhost:3000/mcp".to_string(), headers: std::collections::HashMap::new() }).await.unwrap();

    // Note: MCP client and tool discovery will be handled per-agent now
    // Tools will be registered when agents are created via Agent::set_mcp_client and Agent::register_mcp_tools

    let state = AgentState::default();

    // Generate TypeScript types in debug builds
    #[cfg(debug_assertions)]
    generate_types();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            send_message,
            get_chat_history,
            clear_chat_history,
            get_config,
            update_config,
            get_app_status,
            initialize_agent,
            test_provider_connection,
            start_mcp_server,
            stop_mcp_server,
            test_mcp_server_connection,
            refresh_mcp_server_status,
            get_tool_call_state_info,
            get_tool_call_info,
            get_stream_event_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(debug_assertions)]
fn generate_types() {
    use tauri_specta::{collect_commands, collect_events, Builder};

    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            send_message,
            get_chat_history,
            clear_chat_history,
            get_config,
            update_config,
            get_app_status,
            initialize_agent,
            test_provider_connection,
            start_mcp_server,
            stop_mcp_server,
            test_mcp_server_connection,
            refresh_mcp_server_status,
            get_tool_call_state_info,
            get_tool_call_info,
            get_stream_event_info
        ])
        .events(collect_events![ChatStreamEvent, ToolDiscoveryEventWrapper]);

    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/generated/bindings.ts",
        )
        .expect("Failed to export typescript types");
}
