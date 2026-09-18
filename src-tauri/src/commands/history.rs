use crate::models::McpCallRecord;
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_history(state: State<'_, Arc<AppState>>) -> Vec<McpCallRecord> {
    current_history(&state.history.lock())
}

pub(crate) fn current_history(history: &[McpCallRecord]) -> Vec<McpCallRecord> {
    history.to_vec()
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

#[cfg(test)]
mod tests {
    use super::current_history;

    #[test]
    fn empty_history_stays_empty_instead_of_creating_demo_records() {
        assert!(current_history(&[]).is_empty());
    }
}
