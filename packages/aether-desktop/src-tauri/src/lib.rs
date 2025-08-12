mod state;
mod commands;

use state::AgentState;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            get_config,
            update_config
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
            update_config
        ])
        .events(collect_events![StreamEvent]);

    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/generated/bindings.ts",
        )
        .expect("Failed to export typescript types");
}
