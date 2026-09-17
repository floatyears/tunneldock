use std::path::Path;
use std::sync::Arc;
use tauri::State;
use crate::models::TunnelSettings;
use crate::state::AppState;
#[cfg(target_os = "windows")]
use crate::utils::cmd::execute_cmd;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::utils::cmd::execute_raw;

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

    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            execute_cmd("explorer.exe", &["/select,", &path], None).success
        } else {
            execute_cmd("explorer.exe", &[&path], None).success
        }
    }

    #[cfg(target_os = "macos")]
    {
        if p.is_file() {
            execute_raw("open", &["-R", &path], None).success
        } else {
            execute_raw("open", &[&path], None).success
        }
    }

    #[cfg(target_os = "linux")]
    {
        let target = if p.is_file() {
            p.parent().unwrap_or(p).to_string_lossy().to_string()
        } else {
            path
        };
        execute_raw("xdg-open", &[&target], None).success
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = p;
        false
    }
}

#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}
