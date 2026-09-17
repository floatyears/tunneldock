use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::models::InstallProgressEvent;
use crate::utils::cmd::{execute_cmd, find_executable, refresh_process_path, run_streaming};

static INSTALL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn install_lock() -> &'static Mutex<()> {
    INSTALL_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Clone)]
struct InstallLogger {
    app: AppHandle,
    item_id: String,
}

impl InstallLogger {
    fn emit(&self, stage: &str, line: impl Into<String>, is_error: bool) {
        let _ = self.app.emit(
            "install-log",
            InstallProgressEvent {
                item_id: self.item_id.clone(),
                stage: stage.to_string(),
                log_line: line.into(),
                is_error,
            },
        );
    }

    fn info(&self, line: impl Into<String>) {
        self.emit("installing", line, false);
    }

    fn success(&self, line: impl Into<String>) {
        self.emit("success", line, false);
    }

    fn error(&self, line: impl Into<String>) {
        self.emit("failed", line, true);
    }
}

fn command_output_text(command: &str, args: &[&str]) -> Result<String, String> {
    let out = execute_cmd(command, args, None);
    if !out.success {
        let details = if !out.stderr.trim().is_empty() {
            out.stderr.trim()
        } else if !out.stdout.trim().is_empty() {
            out.stdout.trim()
        } else {
            "命令执行失败且未返回详细输出"
        };
        return Err(format!("{} {} 执行失败: {}", command, args.join(" "), details));
    }

    let text = if !out.stdout.trim().is_empty() {
        out.stdout.trim()
    } else {
        out.stderr.trim()
    };
    Ok(text.to_string())
}

fn prepend_path(path: PathBuf) {
    if !path.exists() {
        return;
    }

    let current = env::var_os("PATH").unwrap_or_default();
    if env::split_paths(&current).any(|entry| entry == path) {
        return;
    }

    let mut entries = vec![path];
    entries.extend(env::split_paths(&current));
    if let Ok(joined) = env::join_paths(entries) {
        env::set_var("PATH", joined);
    }
}

fn ensure_cargo_bin_on_path() {
    if let Some(home) = dirs::home_dir() {
        prepend_path(home.join(".cargo").join("bin"));
    }
}

fn refresh_runtime_environment() {
    refresh_process_path();
    ensure_cargo_bin_on_path();
}

fn verify_node() -> Result<String, String> {
    let path = find_executable("node").ok_or_else(|| "安装完成后仍找不到 node 可执行文件".to_string())?;
    let version = command_output_text("node", &["-v"])?;
    let major = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(|| format!("无法解析 Node.js 版本: {}", version))?;

    if major < 26 {
        #[cfg(target_os = "windows")]
        let candidates = {
            let out = execute_cmd("where.exe", &["node"], None);
            if out.success && !out.stdout.trim().is_empty() {
                format!("\n当前 PATH 中的 node 候选:\n{}", out.stdout.trim())
            } else {
                String::new()
            }
        };
        #[cfg(not(target_os = "windows"))]
        let candidates = String::new();

        return Err(format!(
            "Node.js 安装程序已结束，但当前活动版本仍为 {}（路径: {}），要求 >= 26。{}",
            version, path, candidates
        ));
    }

    Ok(format!("Node.js {} 已验证，活动路径: {}", version, path))
}

fn verify_simple(command: &str, args: &[&str], display_name: &str) -> Result<String, String> {
    let path = find_executable(command)
        .ok_or_else(|| format!("安装完成后仍找不到 {} 可执行文件", display_name))?;
    let version = command_output_text(command, args)?;
    Ok(format!("{} 已验证: {}（{}）", display_name, version.lines().next().unwrap_or(""), path))
}

fn verify_cargo_binstall() -> Result<String, String> {
    if find_executable("cargo-binstall").is_some() {
        return verify_simple("cargo-binstall", &["-V"], "cargo-binstall");
    }

    let version = command_output_text("cargo", &["binstall", "-V"])?;
    Ok(format!("cargo-binstall 已验证: {}", version.lines().next().unwrap_or("")))
}

fn chappie_package_file() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".pi")
            .join("agent")
            .join("npm")
            .join("node_modules")
            .join("@zetaloop")
            .join("chappie")
            .join("package.json")
    })
}

fn verify_chappie() -> Result<String, String> {
    if let Some(pkg) = chappie_package_file() {
        if pkg.exists() {
            let version = std::fs::read_to_string(&pkg)
                .ok()
                .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
                .and_then(|json| json.get("version").and_then(|v| v.as_str()).map(str::to_string));
            return Ok(match version {
                Some(version) => format!("Chappie 扩展 v{} 已验证（{}）", version, pkg.display()),
                None => format!("Chappie 扩展已验证（{}）", pkg.display()),
            });
        }
    }

    let list = execute_cmd("pi", &["list"], None);
    if list.success && list.stdout.contains("@zetaloop/chappie") {
        return Ok("Chappie 扩展已通过 pi list 验证".to_string());
    }

    let help = execute_cmd("pi", &["--help"], None);
    if help.success && help.stdout.contains("--chappie") {
        return Ok("Chappie 扩展已通过 Pi --chappie 能力验证".to_string());
    }

    Err("安装命令结束后仍无法验证 @zetaloop/chappie 扩展".to_string())
}

fn verify_component(item_id: &str) -> Result<String, String> {
    refresh_runtime_environment();
    match item_id {
        "node" => verify_node(),
        "npm" => verify_simple("npm", &["-v"], "npm"),
        "git" => verify_simple("git", &["--version"], "Git"),
        "cargo" => verify_simple("cargo", &["-V"], "Cargo"),
        "cargo_binstall" => verify_cargo_binstall(),
        "otunnel" => verify_simple("otunnel", &["--version"], "otunnel"),
        "pi" => verify_simple("pi", &["--version"], "Pi"),
        "chappie" => verify_chappie(),
        _ => Err(format!("未知的安装组件: {}", item_id)),
    }
}

fn run_logged(logger: &InstallLogger, command: &str, args: &[&str]) -> bool {
    logger.info(format!("> {} {}", command, args.join(" ")));

    // stderr is a stream, not a severity. Cargo, rustup, winget and npm routinely
    // print normal progress to stderr. Only mark the operation as failed after the
    // process exit code/post-condition verification proves failure.
    let ok = run_streaming(command, args, None, {
        let logger = logger.clone();
        move |line, _from_stderr| logger.info(line)
    });

    if !ok {
        logger.info(format!("命令返回非零退出状态: {} {}", command, args.join(" ")));
    }
    ok
}

#[cfg(target_os = "windows")]
fn install_node_windows(logger: &InstallLogger, repair_npm: bool) -> bool {
    if repair_npm {
        logger.info("npm 缺失，将通过重新安装 Node.js 包修复 Node/npm 运行时。".to_string());
        return run_logged(
            logger,
            "winget",
            &[
                "install",
                "--id",
                "OpenJS.NodeJS",
                "-e",
                "--force",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ],
        );
    }

    if find_executable("node").is_some() {
        logger.info("检测到已有 Node.js，优先执行 winget upgrade；如果该安装不受 winget 管理，将自动回退到覆盖安装。".to_string());
        let upgraded = run_logged(
            logger,
            "winget",
            &[
                "upgrade",
                "--id",
                "OpenJS.NodeJS",
                "-e",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ],
        );
        refresh_runtime_environment();
        if verify_node().is_ok() {
            return true;
        }
        if upgraded {
            logger.info("winget 已完成升级，但当前 PATH 仍未解析到满足 >= 26 的 Node.js；停止重复覆盖安装，交由最终验证报告活动路径冲突。".to_string());
            return true;
        }

        logger.info("winget upgrade 不可用或当前 Node 不受 winget 管理，回退到强制安装最新版。".to_string());
    }

    run_logged(
        logger,
        "winget",
        &[
            "install",
            "--id",
            "OpenJS.NodeJS",
            "-e",
            "--force",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ],
    )
}

#[cfg(target_os = "macos")]
fn install_node_macos(logger: &InstallLogger, repair_npm: bool) -> Result<bool, String> {
    if find_executable("brew").is_none() {
        return Err("macOS 自动安装 Node.js 需要 Homebrew".to_string());
    }

    if repair_npm {
        return Ok(run_logged(logger, "brew", &["reinstall", "node"]));
    }

    if find_executable("node").is_some() {
        let upgraded = run_logged(logger, "brew", &["upgrade", "node"]);
        refresh_runtime_environment();
        if verify_node().is_ok() {
            return Ok(true);
        }
        if upgraded {
            logger.info("Homebrew 已完成升级，但活动 Node.js 仍未满足要求；停止重复安装，最终验证将报告当前活动路径。".to_string());
            return Ok(true);
        }
    }

    Ok(run_logged(logger, "brew", &["install", "node"]))
}

fn install_component_blocking(app: AppHandle, item_id: String) -> Result<bool, String> {
    let logger = InstallLogger {
        app,
        item_id: item_id.clone(),
    };

    logger.emit("starting", format!("开始处理组件: {}", item_id), false);
    refresh_runtime_environment();

    // Idempotency: an already-valid post-condition is success. This also makes
    // "一键全自动装配" safe when an earlier step installed a dependency that also
    // appears later in the original missing-items snapshot (for example Node + npm).
    if let Ok(message) = verify_component(&item_id) {
        logger.success(format!("无需重复安装：{}", message));
        return Ok(true);
    }

    let command_ok = match item_id.as_str() {
        "node" => {
            #[cfg(target_os = "windows")]
            {
                install_node_windows(&logger, false)
            }
            #[cfg(target_os = "macos")]
            {
                install_node_macos(&logger, false)?
            }
            #[cfg(target_os = "linux")]
            {
                return Err("Linux 发行版差异较大，请使用发行版包管理器或 Node 官方方式安装 Node.js >= 26".to_string());
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                return Err("当前平台不支持自动安装 Node.js".to_string());
            }
        }
        "npm" => {
            #[cfg(target_os = "windows")]
            {
                install_node_windows(&logger, true)
            }
            #[cfg(target_os = "macos")]
            {
                install_node_macos(&logger, true)?
            }
            #[cfg(target_os = "linux")]
            {
                return Err("Linux 上请通过 Node.js 安装源修复 npm".to_string());
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                return Err("当前平台不支持自动修复 npm".to_string());
            }
        }
        "git" => {
            #[cfg(target_os = "windows")]
            {
                run_logged(
                    &logger,
                    "winget",
                    &[
                        "install",
                        "--id",
                        "Git.Git",
                        "-e",
                        "--accept-source-agreements",
                        "--accept-package-agreements",
                    ],
                )
            }
            #[cfg(target_os = "macos")]
            {
                if find_executable("brew").is_none() {
                    return Err("macOS 自动安装 Git 需要 Homebrew；也可以运行 xcode-select --install".to_string());
                }
                run_logged(&logger, "brew", &["install", "git"])
            }
            #[cfg(target_os = "linux")]
            {
                return Err("请使用当前 Linux 发行版的包管理器安装 Git（例如 apt/dnf/pacman）".to_string());
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                return Err("当前平台不支持自动安装 Git".to_string());
            }
        }
        "cargo" => {
            #[cfg(target_os = "windows")]
            {
                run_logged(
                    &logger,
                    "winget",
                    &[
                        "install",
                        "--id",
                        "Rustlang.Rustup",
                        "-e",
                        "--accept-source-agreements",
                        "--accept-package-agreements",
                    ],
                )
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                if find_executable("curl").is_none() {
                    return Err("自动安装 Rust 需要 curl".to_string());
                }
                run_logged(
                    &logger,
                    "sh",
                    &[
                        "-c",
                        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
                    ],
                )
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                return Err("当前平台不支持自动安装 Rust".to_string());
            }
        }
        "cargo_binstall" => {
            if find_executable("cargo").is_none() {
                return Err("安装 cargo-binstall 前必须先安装 Rust / Cargo".to_string());
            }
            logger.info("cargo-binstall 需要编译时会耗时较长；控制台将持续显示 Cargo 真实输出与静默阶段心跳。".to_string());
            run_logged(&logger, "cargo", &["install", "cargo-binstall", "--locked"])
        }
        "otunnel" => {
            if find_executable("cargo-binstall").is_some() {
                logger.info("检测到 cargo-binstall，优先使用预编译二进制安装 otunnel。".to_string());
                run_logged(&logger, "cargo-binstall", &["otunnel", "-y"])
            } else {
                if find_executable("cargo").is_none() {
                    return Err("安装 otunnel 前必须先安装 Cargo 或 cargo-binstall".to_string());
                }
                logger.info("未检测到 cargo-binstall，将通过 Cargo 源码编译 otunnel；此过程可能需要较长时间。".to_string());
                run_logged(&logger, "cargo", &["install", "otunnel", "--locked"])
            }
        }
        "pi" => {
            if find_executable("npm").is_none() {
                return Err("安装 Pi 前必须先安装 npm".to_string());
            }
            run_logged(
                &logger,
                "npm",
                &[
                    "install",
                    "-g",
                    "--ignore-scripts",
                    "@earendil-works/pi-coding-agent",
                ],
            )
        }
        "chappie" => {
            if find_executable("pi").is_none() {
                return Err("安装 Chappie 扩展前必须先安装 Pi".to_string());
            }
            run_logged(&logger, "pi", &["install", "npm:@zetaloop/chappie"])
        }
        _ => return Err(format!("未知的安装组件: {}", item_id)),
    };

    refresh_runtime_environment();

    match verify_component(&item_id) {
        Ok(message) => {
            if !command_ok {
                logger.info("安装命令曾返回非零状态，但最终运行时后置条件验证通过，以实际可用状态为准。".to_string());
            }
            logger.success(format!("安装完成：{}", message));
            Ok(true)
        }
        Err(verify_error) => {
            let message = if command_ok {
                format!("安装命令已结束，但后置验证失败：{}", verify_error)
            } else {
                format!("安装命令失败，且后置验证未通过：{}", verify_error)
            };
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn install_component_v2(app: AppHandle, item_id: String) -> Result<bool, String> {
    // Package managers mutate shared PATH/toolchain state. Serialize installs so
    // double-clicks or multiple cards cannot run winget/cargo/npm concurrently.
    let _guard = install_lock().lock().await;

    // Package installs and Cargo compilation are blocking operations. Keep them off
    // Tauri/Tokio's async worker threads so health polling and window IPC stay alive.
    let failure_app = app.clone();
    let failure_item_id = item_id.clone();
    let result = tokio::task::spawn_blocking(move || install_component_blocking(app, item_id))
        .await
        .map_err(|join_error| format!("安装任务异常终止: {}", join_error))?;

    if let Err(ref error) = result {
        InstallLogger {
            app: failure_app,
            item_id: failure_item_id,
        }
        .error(error.clone());
    }

    result
}
