use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::models::{McpCallRecord, TunnelSettings, WorkspaceItem};
use crate::utils::cmd::kill_process_tree;
use crate::utils::paths::{ensure_chappie_yaml_synced, get_chappie_yaml_path};

const APP_DATA_DIR_NAME: &str = "TunnelDock";
const LEGACY_APP_DATA_DIR_NAMES: [&str; 2] = ["local-mcp-console", "chappie-desktop"];
const PERSISTED_FILES: [&str; 3] = ["settings.json", "workspaces.json", "history.json"];

pub struct AppState {
    pub workspaces: Arc<Mutex<Vec<WorkspaceItem>>>,
    pub running_workspace_pids: Arc<Mutex<HashMap<String, u32>>>,
    pub running_workspace_stdins: Arc<Mutex<HashMap<String, std::process::ChildStdin>>>,
    pub otunnel_pid: Arc<Mutex<Option<u32>>>,
    pub history: Arc<Mutex<Vec<McpCallRecord>>>,
    pub settings: Arc<Mutex<TunnelSettings>>,
    pub app_data_dir: PathBuf,
    cleanup_started: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        let app_data_dir = Self::resolve_app_data_dir();

        let settings = Self::load_settings(&app_data_dir);
        let workspaces = Self::load_workspaces(&app_data_dir);
        let history = Self::load_history(&app_data_dir);

        Self {
            workspaces: Arc::new(Mutex::new(workspaces)),
            running_workspace_pids: Arc::new(Mutex::new(HashMap::new())),
            running_workspace_stdins: Arc::new(Mutex::new(HashMap::new())),
            otunnel_pid: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(history)),
            settings: Arc::new(Mutex::new(settings)),
            app_data_dir,
            cleanup_started: AtomicBool::new(false),
        }
    }

    fn resolve_app_data_dir() -> PathBuf {
        let base_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::resolve_app_data_dir_in(&base_dir)
    }

    fn resolve_app_data_dir_in(base_dir: &Path) -> PathBuf {
        let app_data_dir = base_dir.join(APP_DATA_DIR_NAME);

        if !app_data_dir.exists() {
            for legacy_name in LEGACY_APP_DATA_DIR_NAMES {
                let legacy_app_data_dir = base_dir.join(legacy_name);
                if legacy_app_data_dir.exists() {
                    if fs::rename(&legacy_app_data_dir, &app_data_dir).is_err() {
                        let _ = fs::create_dir_all(&app_data_dir);
                        Self::copy_legacy_data(&legacy_app_data_dir, &app_data_dir);
                    }
                    break;
                }
            }
        }

        let _ = fs::create_dir_all(&app_data_dir);
        app_data_dir
    }

    fn copy_legacy_data(source_dir: &Path, target_dir: &Path) {
        for file_name in PERSISTED_FILES {
            let source = source_dir.join(file_name);
            let target = target_dir.join(file_name);
            if source.exists() && !target.exists() {
                let _ = fs::copy(source, target);
            }
        }
    }

    pub fn cleanup_all_processes(&self) {
        // Shutdown can be observed through ExitRequested, Exit and Drop. Only the
        // first caller performs cleanup so we never wait on the same children more
        // than once during application teardown.
        if self.cleanup_started.swap(true, Ordering::AcqRel) {
            return;
        }

        // 1. Close RPC stdin first so well-behaved Pi children can observe EOF.
        self.running_workspace_stdins.lock().clear();

        // 2. Terminate only processes owned/tracked by this application. The
        // platform-neutral sysinfo implementation avoids blocking shell commands.
        if let Some(pid) = self.otunnel_pid.lock().take() {
            let _ = kill_process_tree(pid);
        }

        let pids: Vec<u32> = self.running_workspace_pids.lock().values().copied().collect();
        self.running_workspace_pids.lock().clear();
        for pid in pids {
            let _ = kill_process_tree(pid);
        }

        // 3. Persist a clean stopped state for the next launch.
        let mut list = self.workspaces.lock();
        for w in list.iter_mut() {
            w.status = "stopped".to_string();
            w.pid = None;
        }
        drop(list);
        self.save_workspaces();
    }

    fn load_settings(data_dir: &PathBuf) -> TunnelSettings {
        let file = data_dir.join("settings.json");
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(settings) = serde_json::from_str::<TunnelSettings>(&content) {
                return settings;
            }
        }

        // Try reading existing from ~/.chappie/tunnelkey.txt or chappie.yaml if present.
        // These names belong to the Chappie/otunnel integration and are intentionally
        // retained independently from the TunnelDock product brand.
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let key_file = home.join(".chappie").join("tunnelkey.txt");
        let key = if key_file.exists() {
            fs::read_to_string(&key_file).unwrap_or_default().trim().to_string()
        } else {
            String::new()
        };

        ensure_chappie_yaml_synced();

        // Try reading tunnel_id from chappie.yaml across all standard locations.
        let mut tunnel_id = String::new();
        if let Some(yaml_file) = get_chappie_yaml_path() {
            if let Ok(yaml_content) = fs::read_to_string(yaml_file) {
                for line in yaml_content.lines() {
                    if line.trim().starts_with("tunnel_id:") {
                        tunnel_id = line
                            .trim()
                            .trim_start_matches("tunnel_id:")
                            .trim()
                            .to_string();
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
                // When app starts, reset running state to stopped.
                return list
                    .into_iter()
                    .map(|mut w| {
                        w.status = "stopped".to_string();
                        w.pid = None;
                        w
                    })
                    .collect();
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

impl Drop for AppState {
    fn drop(&mut self) {
        self.cleanup_all_processes();
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_data_root(case: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tunneldock-state-test-{}-{}-{}",
            std::process::id(),
            nonce,
            case
        ))
    }

    #[test]
    fn migrates_data_from_all_pre_tunneldock_directories() {
        for legacy_name in ["local-mcp-console", "chappie-desktop"] {
            let base_dir = temporary_data_root(legacy_name);
            let legacy_dir = base_dir.join(legacy_name);
            fs::create_dir_all(&legacy_dir).expect("legacy directory should be created");
            fs::write(legacy_dir.join("settings.json"), legacy_name)
                .expect("legacy settings should be written");

            let resolved = AppState::resolve_app_data_dir_in(&base_dir);

            assert_eq!(resolved, base_dir.join("TunnelDock"));
            assert_eq!(
                fs::read_to_string(resolved.join("settings.json"))
                    .expect("migrated settings should exist"),
                legacy_name
            );
            fs::remove_dir_all(base_dir).expect("test data should be removed");
        }
    }
}
