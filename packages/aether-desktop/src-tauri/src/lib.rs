mod state;
mod commands;

use state::AgentState;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("async_openai=debug,aether_core=debug,reqwest=trace")
        .init();
        
    // Generate TypeScript types in debug builds
    #[cfg(debug_assertions)]
    generate_types();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AgentState::default())
        .invoke_handler(tauri::generate_handler![
            send_message,
            get_chat_history,
            clear_chat_history,
            execute_tool_call,
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
            execute_tool_call,
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
