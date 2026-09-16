use std::fs;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;
use tauri::State;
use crate::models::{DoctorCheckItem, DoctorReport, OtunnelDaemonStatus};
use crate::state::AppState;
use crate::utils::cmd::{execute_cmd, execute_powershell, is_process_running, kill_process_tree};
use crate::utils::paths::{ensure_chappie_yaml_synced, get_chappie_yaml_path};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[tauri::command]
pub async fn get_otunnel_status(state: State<'_, Arc<AppState>>) -> Result<OtunnelDaemonStatus, String> {
    let port = {
        let settings = state.settings.lock();
        settings.health_port
    };

    // Find if otunnel process is running
    let mut pid = *state.otunnel_pid.lock();
    let is_running_tracked = pid.map(|p| is_process_running(p)).unwrap_or(false);

    let mut running = is_running_tracked;
    if !running {
        // Clear stale pid from state
        *state.otunnel_pid.lock() = None;
        pid = None;

        // Look up by process name in system
        let check = execute_powershell(
            "Get-Process -Name otunnel -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id -First 1",
            None,
        );
        if check.success && !check.stdout.trim().is_empty() {
            if let Ok(sys_pid) = check.stdout.trim().parse::<u32>() {
                running = true;
                pid = Some(sys_pid);
                *state.otunnel_pid.lock() = Some(sys_pid);
            }
        }
    }

    // Probe healthz & readyz
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .map_err(|e| e.to_string())?;

    let health_url = format!("http://127.0.0.1:{}/healthz", port);
    let ready_url = format!("http://127.0.0.1:{}/readyz", port);

    let start = Instant::now();
    let health_res = client.get(&health_url).send().await;
    let latency = start.elapsed().as_millis() as u64;

    let healthz_ok = match health_res {
        Ok(res) => res.status().is_success(),
        Err(_) => false,
    };

    let readyz_ok = if healthz_ok {
        match client.get(&ready_url).send().await {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    } else {
        false
    };

    let tunnel_id = {
        let settings = state.settings.lock();
        if settings.tunnel_id.is_empty() {
            None
        } else {
            Some(settings.tunnel_id.clone())
        }
    };

    if !running && !healthz_ok {
        pid = None;
    }

    Ok(OtunnelDaemonStatus {
        running: running || healthz_ok,
        pid,
        healthz_ok,
        readyz_ok,
        latency_ms: if healthz_ok { Some(latency) } else { None },
        listen_port: port,
        tunnel_id,
        uptime_seconds: None,
    })
}

#[tauri::command]
pub async fn start_otunnel(state: State<'_, Arc<AppState>>) -> Result<u32, String> {
    ensure_chappie_yaml_synced();

    // Check if already running
    let cur_status = get_otunnel_status(state.clone()).await?;
    if cur_status.running && cur_status.healthz_ok {
        if let Some(p) = cur_status.pid {
            *state.otunnel_pid.lock() = Some(p);
        }
        return Ok(cur_status.pid.unwrap_or(0));
    }

    let mut cmd = Command::new("otunnel");
    cmd.arg("run");
    if let Some(profile_file) = get_chappie_yaml_path() {
        cmd.args(["--profile-file", &profile_file.to_string_lossy()]);
    } else {
        cmd.args(["--profile", "chappie"]);
    }

    // Capture logs to ~/.chappie/otunnel.log
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let log_dir = home.join(".chappie");
    let _ = fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("otunnel.log");

    let out_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file)
        .map_err(|e| format!("打开日志文件失败: {}", e))?;
    let err_file = out_file.try_clone().map_err(|e| format!("复制日志句柄失败: {}", e))?;

    cmd.stdout(Stdio::from(out_file));
    cmd.stderr(Stdio::from(err_file));

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| format!("启动 otunnel 失败: {}", e))?;
    let pid = child.id();

    *state.otunnel_pid.lock() = Some(pid);

    // Wait a brief moment for socket to bind or immediate exit check
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    if let Ok(Some(status)) = child.try_wait() {
        *state.otunnel_pid.lock() = None;
        let last_log = fs::read_to_string(&log_file).unwrap_or_default();
        let last_lines = last_log.lines().rev().take(6).collect::<Vec<&str>>().into_iter().rev().collect::<Vec<&str>>().join("\n");
        return Err(format!("otunnel 异常退出 (退出码: {}):\n{}", status, last_lines));
    }

    Ok(pid)
}

#[tauri::command]
pub async fn stop_otunnel(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let pid_opt = *state.otunnel_pid.lock();
    let mut killed = false;

    if let Some(pid) = pid_opt {
        if kill_process_tree(pid) {
            killed = true;
        }
        *state.otunnel_pid.lock() = None;
    }

    // Also ensure all otunnel.exe processes and their child trees are terminated
    #[cfg(target_os = "windows")]
    {
        let out = crate::utils::cmd::execute_raw("taskkill.exe", &["/F", "/IM", "otunnel.exe", "/T"], None);
        if out.success {
            killed = true;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let kill_all = execute_powershell("killall -9 otunnel 2>/dev/null", None);
        if kill_all.success {
            killed = true;
        }
    }

    Ok(killed)
}

#[tauri::command]
pub async fn restart_otunnel(state: State<'_, Arc<AppState>>) -> Result<u32, String> {
    let _ = stop_otunnel(state.clone()).await;
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    start_otunnel(state).await
}

#[tauri::command]
pub async fn run_otunnel_doctor() -> Result<DoctorReport, String> {
    ensure_chappie_yaml_synced();

    let mut args = vec!["doctor".to_string()];
    if let Some(profile_file) = get_chappie_yaml_path() {
        args.push("--profile-file".to_string());
        args.push(profile_file.to_string_lossy().to_string());
    } else {
        args.push("--profile".to_string());
        args.push("chappie".to_string());
    }

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = execute_cmd("otunnel", &args_ref, None);
    let full_raw = format!("{}\n{}", out.stdout, out.stderr);

    let mut items = Vec::new();
    let mut overall = if out.success { "PASS".to_string() } else { "FAIL".to_string() };

    for line in full_raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("CHECK ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[1].to_string();
                let status = parts[2].to_string();
                let details = if parts.len() > 3 {
                    parts[3..].join(" ")
                } else {
                    String::new()
                };

                let suggestion = match (name.as_str(), status.as_str()) {
                    ("mcp_server_reachable", "FAIL") => Some(
                        "本地 MCP 进程未能成功启动或响应。请检查系统 Node.js 是否 >= 26，并在终端执行 'pi --chappie' 确认是否存在语法或模块错误。".to_string(),
                    ),
                    ("health_listener", "FAIL") => Some(
                        "8080 端口已被绑定或已被现有的 otunnel 实例占用。可先停止当前运行的 otunnel 再运行检查。".to_string(),
                    ),
                    ("control_plane_connection", "FAIL") => Some(
                        "无法连接 OpenAI 控制面。请检查网络是否能访问 api.openai.com，或检查系统代理设置。".to_string(),
                    ),
                    ("control_plane_api_key", "FAIL") => Some(
                        "API Key 无效。请在 OpenAI Platform 创建只包含 Tunnels: Read/Use 权限的 Key 并填入设置。".to_string(),
                    ),
                    ("tunnel_id", "FAIL") => Some(
                        "Tunnel ID 校验失败。请确认在 OpenAI 平台创建的 Tunnel 状态是否正常。".to_string(),
                    ),
                    _ => None,
                };

                items.push(DoctorCheckItem {
                    name,
                    status,
                    details,
                    suggestion,
                });
            }
        } else if trimmed.starts_with("RESULT ") {
            let res = trimmed.trim_start_matches("RESULT ").trim();
            overall = if res == "ok" { "PASS".to_string() } else { "FAIL".to_string() };
        }
    }

    // If items were empty because command failed before CHECK lines, add raw error item
    if items.is_empty() {
        items.push(DoctorCheckItem {
            name: "otunnel_execution".to_string(),
            status: "FAIL".to_string(),
            details: full_raw.clone(),
            suggestion: Some("请检查 otunnel 是否安装，以及 Profile 配置是否正确初始化。".to_string()),
        });
        overall = "FAIL".to_string();
    }

    Ok(DoctorReport {
        overall,
        items,
        raw_output: full_raw,
    })
}

#[tauri::command]
pub async fn probe_network_latency() -> Result<u64, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(4000))
        .build()
        .map_err(|e| e.to_string())?;

    let start = Instant::now();
    let res = client.get("https://api.openai.com").send().await;
    match res {
        Ok(_) => Ok(start.elapsed().as_millis() as u64),
        Err(err) => Err(format!("网络探测失败: {}", err)),
    }
}
