use std::path::Path;
use std::sync::Arc;
use tauri::State;
use crate::models::TunnelSettings;
use crate::state::AppState;
use crate::utils::cmd::execute_cmd;

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> TunnelSettings {
    state.settings.lock().clone()
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<AppState>>,
    new_settings: TunnelSettings,
) -> Result<TunnelSettings, String> {
    *state.settings.lock() = new_settings.clone();
    state.save_settings();
    Ok(new_settings)
}

#[tauri::command]
pub fn open_path_in_explorer(path: String) -> bool {
    let p = Path::new(&path);
    if p.is_file() {
        execute_cmd("explorer.exe", &["/select,", &path], None).success
    } else {
        execute_cmd("explorer.exe", &[&path], None).success
    }
}

#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}
