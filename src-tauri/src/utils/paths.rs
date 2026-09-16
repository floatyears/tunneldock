use std::fs;
use std::path::PathBuf;

/// Find existing chappie.yaml across known Windows & POSIX/XDG locations
pub fn get_chappie_yaml_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // 1. ~/.config/tunnel-client/chappie.yaml (XDG / default for otunnel)
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".config").join("tunnel-client").join("chappie.yaml"));
    }

    // 2. AppData/Roaming/tunnel-client/chappie.yaml (Windows standard config_dir)
    if let Some(roaming) = dirs::config_dir() {
        candidates.push(roaming.join("tunnel-client").join("chappie.yaml"));
    }

    // 3. ~/.chappie/chappie.yaml (Chappie custom directory)
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".chappie").join("chappie.yaml"));
    }

    for p in candidates {
        if p.exists() {
            if let Ok(meta) = fs::metadata(&p) {
                if meta.len() > 0 {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Save chappie.yaml to all standard locations so otunnel and any CLI variant can always find it
pub fn sync_chappie_yaml(content: &str) -> std::io::Result<PathBuf> {
    let mut target_paths = Vec::new();

    // 1. ~/.config/tunnel-client/chappie.yaml
    if let Some(home) = dirs::home_dir() {
        target_paths.push(home.join(".config").join("tunnel-client").join("chappie.yaml"));
    }

    // 2. AppData/Roaming/tunnel-client/chappie.yaml
    if let Some(roaming) = dirs::config_dir() {
        target_paths.push(roaming.join("tunnel-client").join("chappie.yaml"));
    }

    // 3. ~/.chappie/chappie.yaml
    if let Some(home) = dirs::home_dir() {
        target_paths.push(home.join(".chappie").join("chappie.yaml"));
    }

    let mut primary = None;
    for path in &target_paths {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let res = fs::write(path, content);
        if res.is_ok() && primary.is_none() {
            primary = Some(path.clone());
        }
    }

    primary.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "未能成功写入任何配置路径"))
}

/// Ensure all standard config locations have the chappie.yaml if one already exists
pub fn ensure_chappie_yaml_synced() {
    if let Some(existing) = get_chappie_yaml_path() {
        if let Ok(content) = fs::read_to_string(&existing) {
            let _ = sync_chappie_yaml(&content);
        }
    }
}
