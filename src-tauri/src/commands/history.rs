use std::sync::Arc;
use tauri::State;
use crate::models::McpCallRecord;
use crate::state::AppState;

#[tauri::command]
pub fn list_history(state: State<'_, Arc<AppState>>) -> Vec<McpCallRecord> {
    let mut history = state.history.lock().clone();
    if history.is_empty() {
        // Populate initial guidance records to demonstrate MCP flow
        let samples = vec![
            McpCallRecord {
                id: "init_sample_1".to_string(),
                timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                session_id: Some("session_main".to_string()),
                workspace_name: Some("系统初始化".to_string()),
                tool_name: "init".to_string(),
                args_json: "{}".to_string(),
                result_summary: "绑定默认 Session 成功".to_string(),
                status: "success".to_string(),
                duration_ms: 45,
            },
            McpCallRecord {
                id: "sessions_sample_2".to_string(),
                timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                session_id: None,
                workspace_name: Some("MCP Broker".to_string()),
                tool_name: "sessions".to_string(),
                args_json: "{}".to_string(),
                result_summary: "发现 1 个在线工作区 Pi Session".to_string(),
                status: "success".to_string(),
                duration_ms: 18,
            },
        ];
        *state.history.lock() = samples.clone();
        state.save_history();
        history = samples;
    }
    history
}

#[tauri::command]
pub fn clear_history(state: State<'_, Arc<AppState>>) -> bool {
    state.history.lock().clear();
    state.save_history();
    true
}

#[tauri::command]
pub fn export_history_json(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let history = state.history.lock().clone();
    serde_json::to_string_pretty(&history).map_err(|e| e.to_string())
}
