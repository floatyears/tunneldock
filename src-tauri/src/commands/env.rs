use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use crate::models::{EnvCheckItem, InstallProgressEvent, McpMode, TunnelSettings};
use crate::state::AppState;
use crate::utils::cmd::{execute_cmd, find_executable, is_process_running, kill_process_tree, run_streaming};
use crate::utils::paths::{ensure_chappie_yaml_synced, get_chappie_yaml_path, sync_tunnel_profile};

struct ResetAtomicBool<'a>(&'a AtomicBool);

impl Drop for ResetAtomicBool<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn check_environment(state: State<'_, Arc<AppState>>) -> Result<Vec<EnvCheckItem>, String> {
    let locale = state.settings.lock().locale.clone();
    let is_en = locale.starts_with("en");
    let mut items = Vec::new();
    // System-level installers differ by OS. Windows has winget and macOS can
    // safely use Homebrew when present. Linux distributions vary too much to
    // silently choose a privileged package manager from a desktop GUI.
    let can_install_system_package = cfg!(target_os = "windows")
        || (cfg!(target_os = "macos") && find_executable("brew").is_some());
    let can_bootstrap_rust = cfg!(target_os = "windows") || find_executable("curl").is_some();

    // 1. Node.js
    let node_path = find_executable("node");
    let mut node_item = EnvCheckItem {
        id: "node".to_string(),
        name: crate::i18n::t(&locale, "env.name.node").to_string(),
        category: "runtime".to_string(),
        installed: false,
        version: None,
        required_version: Some(">= 26.0.0".to_string()),
        path: node_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "Node.js not detected. Chappie requires Node >= 26".to_string()
        } else {
            "未检测到 Node.js，Chappie 运行依赖 Node >= 26".to_string()
        },
        can_auto_install: can_install_system_package,
    };

    if node_path.is_some() {
        let out = execute_cmd("node", &["-v"], None);
        if out.success {
            let ver = out.stdout.trim().to_string();
            node_item.installed = true;
            node_item.version = Some(ver.clone());

            // Check if version >= 26
            let major_ver = ver
                .trim_start_matches('v')
                .split('.')
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);

            if major_ver >= 26 {
                node_item.status = "ready".to_string();
                node_item.message = if is_en {
                    format!("Node.js {} is ready (>= 26 satisfied)", ver)
                } else {
                    format!("Node.js {} 已就绪 (满足 >= 26 要求)", ver)
                };
            } else {
                node_item.status = "outdated".to_string();
                node_item.message = if is_en {
                    format!(
                        "Current version {} is too low. Chappie MCP requires Node >= 26. Please upgrade.",
                        ver
                    )
                } else {
                    format!(
                        "当前版本 {} 过低，Chappie MCP 要求 Node >= 26，请点击升级",
                        ver
                    )
                };
            }
        }
    }
    items.push(node_item);

    // 2. npm
    let npm_path = find_executable("npm");
    let mut npm_item = EnvCheckItem {
        id: "npm".to_string(),
        name: crate::i18n::t(&locale, "env.name.npm").to_string(),
        category: "runtime".to_string(),
        installed: false,
        version: None,
        required_version: None,
        path: npm_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "npm not detected".to_string()
        } else {
            "未检测到 npm".to_string()
        },
        can_auto_install: can_install_system_package,
    };
    if npm_path.is_some() {
        let out = execute_cmd("npm", &["-v"], None);
        if out.success {
            npm_item.installed = true;
            npm_item.version = Some(out.stdout.trim().to_string());
            npm_item.status = "ready".to_string();
            npm_item.message = if is_en {
                format!("npm {} is ready", out.stdout.trim())
            } else {
                format!("npm {} 正常可用", out.stdout.trim())
            };
        }
    }
    items.push(npm_item);

    // 3. Git
    let git_path = find_executable("git");
    let mut git_item = EnvCheckItem {
        id: "git".to_string(),
        name: crate::i18n::t(&locale, "env.name.git").to_string(),
        category: "tools".to_string(),
        installed: false,
        version: None,
        required_version: None,
        path: git_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "Git not installed. ChatGPT won't be able to run git branch/status operations".to_string()
        } else {
            "未安装 Git，ChatGPT 将无法执行 git 状态与分支操作".to_string()
        },
        can_auto_install: can_install_system_package,
    };
    if git_path.is_some() {
        let out = execute_cmd("git", &["--version"], None);
        if out.success {
            git_item.installed = true;
            git_item.version = Some(out.stdout.trim().to_string());
            git_item.status = "ready".to_string();
            git_item.message = out.stdout.trim().to_string();
        }
    }
    items.push(git_item);

    // 4. Rust / Cargo
    let cargo_path = find_executable("cargo");
    let mut cargo_item = EnvCheckItem {
        id: "cargo".to_string(),
        name: crate::i18n::t(&locale, "env.name.cargo").to_string(),
        category: "tools".to_string(),
        installed: false,
        version: None,
        required_version: None,
        path: cargo_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "Cargo not detected (used for building and updating otunnel)".to_string()
        } else {
            "未检测到 Cargo (用于构建与更新 otunnel)".to_string()
        },
        can_auto_install: can_bootstrap_rust,
    };
    if cargo_path.is_some() {
        let out = execute_cmd("cargo", &["-V"], None);
        if out.success {
            cargo_item.installed = true;
            cargo_item.version = Some(out.stdout.trim().to_string());
            cargo_item.status = "ready".to_string();
            cargo_item.message = out.stdout.trim().to_string();
        }
    }
    items.push(cargo_item);

    // 5. cargo-binstall
    let binstall_path = find_executable("cargo-binstall");
    let mut binstall_item = EnvCheckItem {
        id: "cargo_binstall".to_string(),
        name: crate::i18n::t(&locale, "env.name.cargo_binstall").to_string(),
        category: "tools".to_string(),
        installed: false,
        version: None,
        required_version: None,
        path: binstall_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "cargo-binstall not installed. Recommended for faster otunnel builds".to_string()
        } else {
            "未安装 cargo-binstall，建议安装以加速 otunnel 构建".to_string()
        },
        can_auto_install: true,
    };
    if binstall_path.is_some() {
        // cargo-binstall requires -V because --version is reserved for target crate version
        let mut out = execute_cmd("cargo-binstall", &["-V"], None);
        if !out.success || out.stdout.trim().is_empty() {
            out = execute_cmd("cargo", &["binstall", "-V"], None);
        }
        binstall_item.installed = true;
        binstall_item.status = "ready".to_string();
        if out.success && !out.stdout.trim().is_empty() {
            let ver = out.stdout.lines().next().unwrap_or("").trim().to_string();
            binstall_item.version = Some(ver.clone());
            binstall_item.message = if is_en {
                format!("cargo-binstall {} is ready", ver)
            } else {
                format!("cargo-binstall {} 已就绪", ver)
            };
        } else {
            binstall_item.message = if is_en {
                "cargo-binstall is installed".to_string()
            } else {
                "cargo-binstall 已安装".to_string()
            };
        }
    }
    items.push(binstall_item);

    // 6. otunnel
    let otunnel_path = find_executable("otunnel");
    let mut otunnel_item = EnvCheckItem {
        id: "otunnel".to_string(),
        name: crate::i18n::t(&locale, "env.name.otunnel").to_string(),
        category: "mcp".to_string(),
        installed: false,
        version: None,
        required_version: None,
        path: otunnel_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "otunnel binary not detected. Unable to establish OpenAI secure tunnel".to_string()
        } else {
            "未检测到 otunnel 二进制，无法建立 OpenAI 安全隧道".to_string()
        },
        can_auto_install: true,
    };
    if otunnel_path.is_some() {
        let out = execute_cmd("otunnel", &["--version"], None);
        if out.success {
            otunnel_item.installed = true;
            otunnel_item.version = Some(out.stdout.trim().to_string());
            otunnel_item.status = "ready".to_string();
            otunnel_item.message = if is_en {
                format!("otunnel {} installed", out.stdout.trim())
            } else {
                format!("otunnel {} 已安装", out.stdout.trim())
            };
        }
    }
    items.push(otunnel_item);

    // 7. Pi Coding Agent
    let pi_path = find_executable("pi");
    let mut pi_item = EnvCheckItem {
        id: "pi".to_string(),
        name: crate::i18n::t(&locale, "env.name.pi").to_string(),
        category: "mcp".to_string(),
        installed: false,
        version: None,
        required_version: None,
        path: pi_path.clone(),
        status: "missing".to_string(),
        message: if is_en {
            "pi not detected. Please install @earendil-works/pi-coding-agent".to_string()
        } else {
            "未检测到 pi，请安装 @earendil-works/pi-coding-agent".to_string()
        },
        can_auto_install: true,
    };
    if pi_path.is_some() {
        // execute_cmd resolves .cmd wrappers through PATHEXT on Windows and
        // invokes the executable directly on macOS/Linux.
        let out = execute_cmd("pi", &["--version"], None);

        if out.success && !out.stdout.trim().is_empty() {
            let ver = out.stdout.lines().next().unwrap_or("").trim().to_string();
            pi_item.installed = true;
            pi_item.version = Some(ver.clone());
            pi_item.status = "ready".to_string();
            pi_item.message = if is_en {
                format!("pi {} is ready", ver)
            } else {
                format!("pi {} 已就绪", ver)
            };
        }
    }
    items.push(pi_item);

    // 8. Chappie Extension
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let chappie_pkg_file = home
        .join(".pi")
        .join("agent")
        .join("npm")
        .join("node_modules")
        .join("@zetaloop")
        .join("chappie")
        .join("package.json");

    let mut chappie_ver = None;
    let mut chappie_path = None;
    if chappie_pkg_file.exists() {
        if let Ok(content) = fs::read_to_string(&chappie_pkg_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(v) = val.get("version").and_then(|v| v.as_str()) {
                    chappie_ver = Some(v.to_string());
                }
            }
        }
        chappie_path = chappie_pkg_file.parent().map(|p| p.to_string_lossy().to_string());
    }

    let mut chappie_item = EnvCheckItem {
        id: "chappie".to_string(),
        name: crate::i18n::t(&locale, "env.name.chappie").to_string(),
        category: "mcp".to_string(),
        installed: false,
        version: chappie_ver.clone(),
        required_version: None,
        path: chappie_path,
        status: "missing".to_string(),
        message: if is_en {
            "Chappie extension is not installed in Pi".to_string()
        } else {
            "Pi 中未安装 @zetaloop/chappie 扩展".to_string()
        },
        can_auto_install: true,
    };

    if pi_path.is_some() || chappie_pkg_file.exists() {
        let list_out = execute_cmd("pi", &["list"], None);
        let help_out = execute_cmd("pi", &["--help"], None);

        let has_pkg_on_disk = chappie_pkg_file.exists();
        let has_pkg_in_list = list_out.stdout.contains("@zetaloop/chappie");
        let has_flag = help_out.stdout.contains("--chappie");

        if has_pkg_on_disk || has_pkg_in_list || has_flag {
            chappie_item.installed = true;
            chappie_item.status = "ready".to_string();
            let ver_text = chappie_ver
                .map(|v| format!("v{} ", v))
                .unwrap_or_default();
            chappie_item.message = if is_en {
                format!("Chappie MCP Broker {}integrated into Pi", ver_text)
            } else {
                format!("Chappie MCP Broker {}已正常集成至 Pi", ver_text)
            };
        }
    }
    items.push(chappie_item);

    // 9. OpenAI Tunnel API Key
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let key_file = home.join(".chappie").join("tunnelkey.txt");
    let key_exists = key_file.exists();
    let mut key_content = String::new();
    if key_exists {
        if let Ok(c) = fs::read_to_string(&key_file) {
            key_content = c.trim().to_string();
        }
    }
    let has_valid_key = key_content.starts_with("sk-");

    let key_item = EnvCheckItem {
        id: "tunnel_key".to_string(),
        name: crate::i18n::t(&locale, "env.name.tunnel_key").to_string(),
        category: "credentials".to_string(),
        installed: has_valid_key,
        version: if has_valid_key {
            Some(format!("{}...", &key_content[..std::cmp::min(10, key_content.len())]))
        } else {
            None
        },
        required_version: None,
        path: Some(key_file.to_string_lossy().to_string()),
        status: if has_valid_key {
            "ready".to_string()
        } else {
            "config_needed".to_string()
        },
        message: if has_valid_key {
            if is_en {
                format!("API Key configured ({})", key_file.display())
            } else {
                format!("API Key 已配置 ({})", key_file.display())
            }
        } else if is_en {
            "Missing API Key. Please enter OpenAI Tunnel Restricted Key in Settings".to_string()
        } else {
            "缺少 API Key，请在设置中输入 OpenAI Tunnel Restricted Key".to_string()
        },
        can_auto_install: false,
    };
    items.push(key_item);

    // 10. Tunnel Profile (chappie.yaml)
    ensure_chappie_yaml_synced();
    let yaml_opt = get_chappie_yaml_path();
    let yaml_exists = yaml_opt.is_some();
    let mut has_tunnel_id = false;
    let mut tunnel_id_str = String::new();
    let resolved_path = yaml_opt.as_ref().map(|p| p.to_string_lossy().to_string());

    if let Some(ref yaml_file) = yaml_opt {
        if let Ok(content) = fs::read_to_string(yaml_file) {
            for line in content.lines() {
                if line.trim().starts_with("tunnel_id:") {
                    let tid = line.trim().trim_start_matches("tunnel_id:").trim();
                    if tid.starts_with("tunnel_") {
                        has_tunnel_id = true;
                        tunnel_id_str = tid.to_string();
                    }
                }
            }
        }
    }

    let config_item = EnvCheckItem {
        id: "tunnel_config".to_string(),
        name: crate::i18n::t(&locale, "env.name.tunnel_config").to_string(),
        category: "credentials".to_string(),
        installed: yaml_exists && has_tunnel_id,
        version: if has_tunnel_id { Some(tunnel_id_str) } else { None },
        required_version: None,
        path: resolved_path,
        status: if yaml_exists && has_tunnel_id {
            "ready".to_string()
        } else {
            "config_needed".to_string()
        },
        message: if yaml_exists && has_tunnel_id {
            if is_en {
                "Otunnel profile configuration initialized".to_string()
            } else {
                "Otunnel Profile 配置文件已初始化".to_string()
            }
        } else if is_en {
            "Profile not initialized or missing valid Tunnel ID".to_string()
        } else {
            "尚未初始化 Profile 或缺少有效 Tunnel ID".to_string()
        },
        can_auto_install: false,
    };
    items.push(config_item);

    Ok(items)
}

#[tauri::command]
pub async fn install_component(app: AppHandle, item_id: String) -> Result<bool, String> {
    let app_handle = app.clone();
    let id_for_closure = item_id.clone();
    let emit_log = move |line: String, is_err: bool| {
        let _ = app_handle.emit(
            "install-log",
            InstallProgressEvent {
                item_id: id_for_closure.clone(),
                stage: if is_err { "error".to_string() } else { "installing".to_string() },
                log_line: line,
                is_error: is_err,
            },
        );
    };

    let log_fn = emit_log.clone();

    match item_id.as_str() {
        "node" | "npm" => {
            #[cfg(target_os = "windows")]
            {
                log_fn("正在使用 winget 安装最新版 Node.js (包含 npm)...".to_string(), false);
                return Ok(run_streaming(
                    "winget",
                    &["install", "--id", "OpenJS.NodeJS", "-e", "--accept-source-agreements", "--accept-package-agreements"],
                    None,
                    emit_log,
                ));
            }
            #[cfg(target_os = "macos")]
            {
                if find_executable("brew").is_none() {
                    return Err("macOS 自动安装 Node.js 需要 Homebrew；请先安装 Homebrew 或手动安装 Node.js >= 26".to_string());
                }
                log_fn("正在使用 Homebrew 安装最新版 Node.js (包含 npm)...".to_string(), false);
                return Ok(run_streaming("brew", &["install", "node"], None, emit_log));
            }
            #[cfg(target_os = "linux")]
            {
                Err("Linux 发行版的软件源差异较大，为避免自动安装到低于 26 的 Node.js，请使用你的发行版或 Node 官方方式安装 Node.js >= 26".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("当前平台不支持自动安装 Node.js".to_string())
            }
        }
        "git" => {
            #[cfg(target_os = "windows")]
            {
                log_fn("正在使用 winget 安装 Git...".to_string(), false);
                return Ok(run_streaming(
                    "winget",
                    &["install", "--id", "Git.Git", "-e", "--accept-source-agreements", "--accept-package-agreements"],
                    None,
                    emit_log,
                ));
            }
            #[cfg(target_os = "macos")]
            {
                if find_executable("brew").is_none() {
                    return Err("macOS 自动安装 Git 需要 Homebrew；也可以运行 xcode-select --install".to_string());
                }
                log_fn("正在使用 Homebrew 安装 Git...".to_string(), false);
                return Ok(run_streaming("brew", &["install", "git"], None, emit_log));
            }
            #[cfg(target_os = "linux")]
            {
                Err("请使用当前 Linux 发行版的包管理器安装 Git（例如 apt/dnf/pacman）；应用不会在后台静默请求 root 权限".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("当前平台不支持自动安装 Git".to_string())
            }
        }
        "cargo" => {
            #[cfg(target_os = "windows")]
            {
                log_fn("正在使用 winget 安装 Rustup / Cargo...".to_string(), false);
                return Ok(run_streaming(
                    "winget",
                    &["install", "--id", "Rustlang.Rustup", "-e", "--accept-source-agreements", "--accept-package-agreements"],
                    None,
                    emit_log,
                ));
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                if find_executable("curl").is_none() {
                    return Err("自动安装 Rust 需要 curl，请先安装 curl 或手动安装 rustup".to_string());
                }
                log_fn("正在通过 rustup 官方安装脚本安装 Rust / Cargo...".to_string(), false);
                return Ok(run_streaming(
                    "sh",
                    &["-c", "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"],
                    None,
                    emit_log,
                ));
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("当前平台不支持自动安装 Rust".to_string())
            }
        }
        "cargo_binstall" => {
            log_fn("正在执行 cargo install cargo-binstall --locked...".to_string(), false);
            Ok(run_streaming(
                "cargo",
                &["install", "cargo-binstall", "--locked"],
                None,
                emit_log,
            ))
        }
        "otunnel" => {
            log_fn("正在安装 otunnel 客户端...".to_string(), false);
            let ok = if find_executable("cargo-binstall").is_some() {
                log_fn("使用 cargo-binstall 安装 otunnel...".to_string(), false);
                run_streaming("cargo-binstall", &["otunnel", "-y"], None, emit_log)
            } else {
                log_fn("使用 cargo install otunnel 编译安装...".to_string(), false);
                run_streaming("cargo", &["install", "otunnel", "--locked"], None, emit_log)
            };
            Ok(ok)
        }
        "pi" => {
            log_fn("正在全局安装 @earendil-works/pi-coding-agent...".to_string(), false);
            Ok(run_streaming(
                "npm",
                &["install", "-g", "--ignore-scripts", "@earendil-works/pi-coding-agent"],
                None,
                emit_log,
            ))
        }
        "chappie" => {
            log_fn("正在为 Pi 安装 Chappie 扩展: pi install npm:@zetaloop/chappie...".to_string(), false);
            Ok(run_streaming(
                "pi",
                &["install", "npm:@zetaloop/chappie"],
                None,
                emit_log,
            ))
        }
        _ => Err(format!("未知的安装组件: {}", item_id)),
    }
}

#[tauri::command]
pub async fn save_tunnel_credentials(
    state: State<'_, Arc<AppState>>,
    tunnel_id: String,
    api_key: String,
    health_port: Option<u16>,
    mcp_mode: Option<McpMode>,
    safe_tunnel_id: Option<String>,
    full_tunnel_id: Option<String>,
) -> Result<TunnelSettings, String> {
    let clean_key = api_key.trim().to_string();
    let clean_id = tunnel_id.trim().to_string();

    let locale = state.settings.lock().locale.clone();
    if clean_key.is_empty() || clean_id.is_empty() {
        return Err(crate::i18n::t(&locale, "error.tunnel_id_and_key_required").to_string());
    }
    if state.tunnel_config_busy.swap(true, Ordering::AcqRel) {
        return Err("A Tunnel configuration change is already in progress.".to_string());
    }
    let _config_guard = ResetAtomicBool(&state.tunnel_config_busy);

    let current_settings = state.settings.lock().clone();
    let port = health_port.unwrap_or(current_settings.health_port);
    let selected_mode = mcp_mode.unwrap_or(current_settings.mcp_mode);
    let mode_changed = selected_mode != current_settings.mcp_mode;
    let mut safe_id = safe_tunnel_id.clone().unwrap_or_else(|| current_settings.safe_tunnel_id.clone());
    let mut full_id = full_tunnel_id.clone().unwrap_or_else(|| current_settings.full_tunnel_id.clone());
    if safe_id.is_empty() && full_id.is_empty() {
        match current_settings.mcp_mode {
            McpMode::ReadOnly => safe_id = current_settings.tunnel_id.clone(),
            McpMode::Full => full_id = current_settings.tunnel_id.clone(),
        }
    }
    // Older callers (the first-run environment form) update the active identity
    // without sending both mode-specific IDs.
    if safe_tunnel_id.is_none() && full_tunnel_id.is_none() {
        match selected_mode {
            McpMode::ReadOnly => safe_id = clean_id.clone(),
            McpMode::Full => full_id = clean_id.clone(),
        }
    }
    safe_id = safe_id.trim().to_string();
    full_id = full_id.trim().to_string();
    if !safe_id.is_empty() && safe_id == full_id {
        return Err("Read Only and Full MCP must use separate Tunnel IDs so ChatGPT keeps distinct tool snapshots.".to_string());
    }
    let selected_tunnel_id = match selected_mode {
        McpMode::ReadOnly => safe_id.trim().to_string(),
        McpMode::Full => full_id.trim().to_string(),
    };
    if selected_tunnel_id.is_empty() {
        return Err(match selected_mode {
            McpMode::ReadOnly => "Configure a Read Only Tunnel ID before switching to Read Only.".to_string(),
            McpMode::Full => "Configure a Full MCP Tunnel ID before switching to Full MCP.".to_string(),
        });
    }

    let backend_changed = mode_changed
        || selected_tunnel_id != current_settings.tunnel_id
        || clean_key != current_settings.api_key
        || port != current_settings.health_port;
    if backend_changed {
        if !wait_for_workspace_starts(&state).await {
            return Err("Pi Session 启动尚未结束，无法安全更改 Tunnel 配置；本次更改未应用。".to_string());
        }
    }

    let mut tunnel_was_running = false;
    if backend_changed {
        let daemon = crate::commands::otunnel::get_otunnel_status(state.clone()).await?;
        if daemon.running {
            tunnel_was_running = true;
            let stopped = crate::commands::otunnel::stop_otunnel_during_config_update(state.clone()).await?;
            if !stopped {
                return Err("The running MCP tunnel could not be stopped safely. No configuration change was applied.".to_string());
            }
        }
        // Stop the tunnel before draining so it cannot accept more MCP work
        // while we wait for calls already running in TunnelDock-managed Pi sessions.
        wait_for_active_workspace_calls(&state).await;
    }
    let home = match dirs::home_dir() {
        Some(home) => home,
        None => {
            if tunnel_was_running {
                let _ = crate::commands::otunnel::start_otunnel_during_config_update(state.clone()).await;
            }
            return Err(if locale.starts_with("en") {
                "Unable to get user home directory".to_string()
            } else {
                "无法获取用户主目录".to_string()
            });
        }
    };
    let chappie_dir = home.join(".chappie");
    if let Err(error) = fs::create_dir_all(&chappie_dir) {
        if tunnel_was_running {
            let _ = crate::commands::otunnel::start_otunnel_during_config_update(state.clone()).await;
        }
        return Err(error.to_string());
    }

    let key_file = chappie_dir.join("tunnelkey.txt");
    let previous_key = fs::read(&key_file).ok();
    if let Err(error) = fs::write(&key_file, &clean_key) {
        if let Some(previous) = previous_key.as_ref() {
            let _ = fs::write(&key_file, previous);
        } else {
            let _ = fs::remove_file(&key_file);
        }
        if tunnel_was_running {
            let _ = crate::commands::otunnel::start_otunnel_during_config_update(state.clone()).await;
        }
        return Err(format!("写入密钥文件失败: {}", error));
    }

    let key_file_str = key_file.to_string_lossy().to_string();

    let new_settings = TunnelSettings {
        tunnel_id: selected_tunnel_id,
        safe_tunnel_id: safe_id,
        full_tunnel_id: full_id,
        api_key: clean_key,
        key_file_path: key_file_str,
        health_port: port,
        profile_name: "chappie".to_string(),
        mcp_mode: selected_mode,
        locale,
    };

    if let Err(error) = sync_tunnel_profile(&new_settings) {
        let _ = sync_tunnel_profile(&current_settings);
        match previous_key {
            Some(previous) => {
                let _ = fs::write(&key_file, previous);
            }
            None => {
                let _ = fs::remove_file(&key_file);
            }
        }
        if tunnel_was_running {
            let _ = crate::commands::otunnel::start_otunnel_during_config_update(state.clone()).await;
        }
        return Err(error);
    }

    if mode_changed && selected_mode == McpMode::ReadOnly {
        if let Err(error) = stop_workspace_sessions(&state).await {
            if let Some(previous) = previous_key.as_ref() {
                let _ = fs::write(&key_file, previous);
            } else {
                let _ = fs::remove_file(&key_file);
            }
            let _ = sync_tunnel_profile(&current_settings);
            if tunnel_was_running {
                if let Err(restart_error) = crate::commands::otunnel::start_otunnel_during_config_update(state.clone()).await {
                    return Err(format!("{error} The previous tunnel could not be restored: {restart_error}"));
                }
            }
            return Err(error);
        }
    }
    *state.settings.lock() = new_settings.clone();
    state.save_settings();

    if tunnel_was_running {
        crate::commands::otunnel::start_otunnel_during_config_update(state.clone())
            .await
            .map_err(|error| format!("Mode saved, but the new MCP backend did not become ready. The tunnel remains stopped: {error}"))?;
    }

    Ok(new_settings)
}

async fn wait_for_active_workspace_calls(state: &AppState) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut idle_since = None;
    loop {
        let calls_running = state
            .history
            .lock()
            .iter()
            .any(|record| record.status == "executing");
        let now = tokio::time::Instant::now();
        if calls_running {
            idle_since = None;
        } else if let Some(since) = idle_since {
            if now.duration_since(since) >= std::time::Duration::from_millis(250) {
                break;
            }
        } else {
            idle_since = Some(now);
        }
        if now >= deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn wait_for_workspace_starts(state: &AppState) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.workspace_start_count.load(Ordering::Acquire) > 0 {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    true
}

async fn stop_workspace_sessions(state: &AppState) -> Result<(), String> {
    let pids = state.running_workspace_pids.lock().values().copied().collect::<Vec<_>>();
    let mut every_process_tree_stopped = true;
    for pid in &pids {
        if !kill_process_tree(*pid) {
            every_process_tree_stopped = false;
        }
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    while pids.iter().any(|pid| is_process_running(*pid)) {
        if tokio::time::Instant::now() >= deadline {
            return Err("Could not stop every TunnelDock-managed Pi Session. Read Only was not enabled.".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    if !every_process_tree_stopped {
        return Err("Could not confirm that every TunnelDock-managed Pi process was stopped. Read Only was not enabled.".to_string());
    }
    state.running_workspace_pids.lock().clear();
    state.running_workspace_stdins.lock().clear();
    {
        let mut workspaces = state.workspaces.lock();
        for workspace in workspaces.iter_mut() {
            workspace.status = "stopped".to_string();
            workspace.session_id = None;
            workspace.pid = None;
            workspace.binding_count = 0;
        }
    }
    {
        let mut history = state.history.lock();
        for record in history.iter_mut().filter(|record| record.status == "executing") {
            record.status = "error".to_string();
            record.result_summary = "Cancelled while switching to Read Only.".to_string();
        }
    }
    state.save_workspaces();
    state.save_history();
    Ok(())
}
