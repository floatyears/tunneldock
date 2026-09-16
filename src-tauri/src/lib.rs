pub mod models;
pub mod state;
pub mod utils;
pub mod commands;

use std::sync::Arc;
use state::AppState;
use tauri::Manager;

pub fn run() {
    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Environment
            commands::env::check_environment,
            commands::env::install_component,
            commands::env::save_tunnel_credentials,
            // Otunnel & Health & Doctor
            commands::otunnel::get_otunnel_status,
            commands::otunnel::start_otunnel,
            commands::otunnel::stop_otunnel,
            commands::otunnel::restart_otunnel,
            commands::otunnel::run_otunnel_doctor,
            commands::otunnel::probe_network_latency,
            // Workspaces
            commands::workspace::list_workspaces,
            commands::workspace::add_workspace,
            commands::workspace::remove_workspace,
            commands::workspace::start_workspace_session,
            commands::workspace::stop_workspace_session,
            commands::workspace::restart_workspace_session,
            commands::workspace::generate_chatgpt_prompt,
            // History
            commands::history::list_history,
            commands::history::clear_history,
            commands::history::export_history_json,
            // Settings
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::open_path_in_explorer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
