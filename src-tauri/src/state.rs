use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use crate::models::{McpCallRecord, TunnelSettings, WorkspaceItem};
use crate::utils::paths::{ensure_chappie_yaml_synced, get_chappie_yaml_path};

pub struct AppState {
    pub workspaces: Arc<Mutex<Vec<WorkspaceItem>>>,
    pub running_workspace_pids: Arc<Mutex<HashMap<String, u32>>>,
    pub otunnel_pid: Arc<Mutex<Option<u32>>>,
    pub history: Arc<Mutex<Vec<McpCallRecord>>>,
    pub settings: Arc<Mutex<TunnelSettings>>,
    pub app_data_dir: PathBuf,
}

impl AppState {
    pub fn new() -> Self {
        let app_data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chappie-desktop");
        let _ = fs::create_dir_all(&app_data_dir);

        let settings = Self::load_settings(&app_data_dir);
        let workspaces = Self::load_workspaces(&app_data_dir);
        let history = Self::load_history(&app_data_dir);

        Self {
            workspaces: Arc::new(Mutex::new(workspaces)),
            running_workspace_pids: Arc::new(Mutex::new(HashMap::new())),
            otunnel_pid: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(history)),
            settings: Arc::new(Mutex::new(settings)),
            app_data_dir,
        }
    }

    fn load_settings(data_dir: &PathBuf) -> TunnelSettings {
        let file = data_dir.join("settings.json");
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(settings) = serde_json::from_str::<TunnelSettings>(&content) {
                return settings;
            }
        }

        // Try reading existing from ~/.chappie/tunnelkey.txt or chappie.yaml if present
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let key_file = home.join(".chappie").join("tunnelkey.txt");
        let key = if key_file.exists() {
            fs::read_to_string(&key_file).unwrap_or_default().trim().to_string()
        } else {
            String::new()
        };

        ensure_chappie_yaml_synced();

        // Try reading tunnel_id from chappie.yaml across all standard locations
        let mut tunnel_id = String::new();
        if let Some(yaml_file) = get_chappie_yaml_path() {
            if let Ok(yaml_content) = fs::read_to_string(yaml_file) {
                for line in yaml_content.lines() {
                    if line.trim().starts_with("tunnel_id:") {
                        tunnel_id = line.trim().trim_start_matches("tunnel_id:").trim().to_string();
                        break;
                    }
                }
            }
        }

        TunnelSettings {
            tunnel_id,
            api_key: key,
            key_file_path: key_file.to_string_lossy().to_string(),
            health_port: 8080,
            profile_name: "chappie".to_string(),
        }
    }

    pub fn save_settings(&self) {
        let settings = self.settings.lock().clone();
        let file = self.app_data_dir.join("settings.json");
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = fs::write(file, json);
        }
    }

    fn load_workspaces(data_dir: &PathBuf) -> Vec<WorkspaceItem> {
        let file = data_dir.join("workspaces.json");
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(list) = serde_json::from_str::<Vec<WorkspaceItem>>(&content) {
                // When app starts, reset running state to stopped
                return list.into_iter().map(|mut w| {
                    w.status = "stopped".to_string();
                    w.pid = None;
                    w
                }).collect();
            }
        }
        Vec::new()
    }

    pub fn save_workspaces(&self) {
        let list = self.workspaces.lock().clone();
        let file = self.app_data_dir.join("workspaces.json");
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = fs::write(file, json);
        }
    }

    fn load_history(data_dir: &PathBuf) -> Vec<McpCallRecord> {
        let file = data_dir.join("history.json");
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(list) = serde_json::from_str::<Vec<McpCallRecord>>(&content) {
                return list;
            }
        }
        Vec::new()
    }

    pub fn save_history(&self) {
        let list = self.history.lock().clone();
        let file = self.app_data_dir.join("history.json");
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = fs::write(file, json);
        }
    }
}
