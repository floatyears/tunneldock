use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use crate::models::WorkspaceItem;
use crate::state::AppState;
use crate::utils::cmd::{execute_cmd, is_process_running, kill_process_tree};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[tauri::command]
pub async fn list_workspaces(state: State<'_, Arc<AppState>>) -> Result<Vec<WorkspaceItem>, String> {
    let mut list = state.workspaces.lock().clone();
    let pids = state.running_workspace_pids.lock().clone();

    for item in &mut list {
        let is_running = if let Some(pid) = pids.get(&item.id) {
            is_process_running(*pid)
        } else {
            false
        };

        if is_running {
            item.status = "ready".to_string();
            item.pid = pids.get(&item.id).copied();
        } else {
            item.status = "stopped".to_string();
            item.pid = None;
        }

        // Check git details
        let path = Path::new(&item.path);
        if path.exists() {
            let branch_out = execute_cmd("git", &["branch", "--show-current"], Some(path));
            if branch_out.success && !branch_out.stdout.trim().is_empty() {
                item.git_branch = Some(branch_out.stdout.trim().to_string());
            }

            let status_out = execute_cmd("git", &["status", "--porcelain"], Some(path));
            if status_out.success {
                let changes = status_out.stdout.lines().count();
                item.git_status = if changes == 0 {
                    Some("工作区干净 (Clean)".to_string())
                } else {
                    Some(format!("{} 个文件未提交变更", changes))
                };
            }
        }
    }

    *state.workspaces.lock() = list.clone();
    Ok(list)
}

#[tauri::command]
pub async fn add_workspace(
    state: State<'_, Arc<AppState>>,
    path: String,
    name: Option<String>,
) -> Result<WorkspaceItem, String> {
    let p = PathBuf::from(&path);
    if !p.exists() || !p.is_dir() {
        return Err("指定的项目目录不存在或不是有效文件夹".to_string());
    }

    let default_name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Workspace".to_string());

    let final_name = name.unwrap_or(default_name);
    let id = format!("ws_{}", chrono::Utc::now().timestamp_millis());

    let mut item = WorkspaceItem {
        id: id.clone(),
        name: final_name,
        path: p.to_string_lossy().to_string(),
        status: "stopped".to_string(),
        session_id: None,
        pid: None,
        binding_count: 0,
        git_branch: None,
        git_status: None,
        last_started_at: None,
        error_message: None,
    };

    // Probe Git
    let branch_out = execute_cmd("git", &["branch", "--show-current"], Some(&p));
    if branch_out.success && !branch_out.stdout.trim().is_empty() {
        item.git_branch = Some(branch_out.stdout.trim().to_string());
    }

    {
        let mut workspaces = state.workspaces.lock();
        // Prevent duplicate path
        if workspaces.iter().any(|w| w.path == item.path) {
            return Err("该工作区路径已存在于列表中".to_string());
        }
        workspaces.push(item.clone());
    }

    state.save_workspaces();
    Ok(item)
}

#[tauri::command]
pub async fn remove_workspace(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<bool, String> {
    // Stop if running
    let _ = stop_workspace_session(app, state.clone(), workspace_id.clone()).await;

    let mut workspaces = state.workspaces.lock();
    workspaces.retain(|w| w.id != workspace_id);
    drop(workspaces);

    state.save_workspaces();
    Ok(true)
}

#[tauri::command]
pub async fn start_workspace_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<u32, String> {
    let (path_str, ws_name) = {
        let workspaces = state.workspaces.lock();
        let ws = workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .ok_or_else(|| "工作区未找到".to_string())?;
        (ws.path.clone(), ws.name.clone())
    };

    let dir = PathBuf::from(&path_str);
    if !dir.exists() {
        return Err("工作区目录不存在".to_string());
    }

    // Stop existing process if any
    let _ = stop_workspace_session(app.clone(), state.clone(), workspace_id.clone()).await;

    // Emit startup feedback into terminal drawer
    let _ = app.emit(
        "workspace-log",
        serde_json::json!({
            "workspace_id": &workspace_id,
            "line": format!(">>> 正在启动工作区 Pi Session: {} ({})", ws_name, path_str),
            "is_error": false,
        }),
    );
    let _ = app.emit(
        "workspace-log",
        serde_json::json!({
            "workspace_id": &workspace_id,
            "line": ">>> 运行模式: RPC 后台服务 (连接 Chappie MCP Broker)...",
            "is_error": false,
        }),
    );

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd.exe");
        c.args(["/d", "/s", "/c", "pi", "--mode", "rpc", "--provider", "chappie", "--model", "chatgpt"]);
        c.current_dir(&dir);
        c.stdin(Stdio::piped());
        c.stdout(Stdio::piped());
        c.stderr(Stdio::piped());
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("pi");
        c.args(["--mode", "rpc", "--provider", "chappie", "--model", "chatgpt"]);
        c.current_dir(&dir);
        c.stdin(Stdio::piped());
        c.stdout(Stdio::piped());
        c.stderr(Stdio::piped());
        c
    };

    let mut child = cmd.spawn().map_err(|e| format!("无法启动 Pi Session: {}", e))?;
    let pid = child.id();

    // Track PID
    state.running_workspace_pids.lock().insert(workspace_id.clone(), pid);

    // Write initial command to Pi RPC stdin to query session state & obtain sessionId
    let mut stdin = child.stdin.take();
    if let Some(ref mut sin) = stdin {
        use std::io::Write;
        let _ = sin.write_all(b"{\"id\":\"init\",\"type\":\"get_state\"}\n");
        let _ = sin.flush();
    }
    if let Some(sin) = stdin {
        state.running_workspace_stdins.lock().insert(workspace_id.clone(), sin);
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Stream stdout to terminal drawer & parse session_id
    if let Some(out) = stdout {
        let app_handle = app.clone();
        let ws_id = workspace_id.clone();
        let state_clone = state.inner().clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(out);
            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Look for JSON RPC response containing sessionId
                    if trimmed.contains("\"sessionId\"") {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            let sid = val["data"]["sessionId"]
                                .as_str()
                                .or_else(|| val["sessionId"].as_str());
                            if let Some(session_id) = sid {
                                let mut list = state_clone.workspaces.lock();
                                if let Some(w) = list.iter_mut().find(|w| w.id == ws_id) {
                                    w.session_id = Some(session_id.to_string());
                                }
                                drop(list);
                                state_clone.save_workspaces();

                                let _ = app_handle.emit(
                                    "workspace-log",
                                    serde_json::json!({
                                        "workspace_id": &ws_id,
                                        "line": format!(">>> Pi 会话已激活并注册至 Broker | Session ID: {}", session_id),
                                        "is_error": false,
                                    }),
                                );
                            }
                        }
                    }

                    let _ = app_handle.emit(
                        "workspace-log",
                        serde_json::json!({
                            "workspace_id": &ws_id,
                            "line": trimmed,
                            "is_error": false,
                        }),
                    );
                }
            }
        });
    }

    // Stream stderr to terminal drawer
    if let Some(err) = stderr {
        let app_handle = app.clone();
        let ws_id = workspace_id.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(err);
            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let lower = trimmed.to_lowercase();
                    let is_err = lower.contains("error:") || lower.contains("fatal:") || lower.contains("exception");
                    let _ = app_handle.emit(
                        "workspace-log",
                        serde_json::json!({
                            "workspace_id": &ws_id,
                            "line": trimmed,
                            "is_error": is_err,
                        }),
                    );
                }
            }
        });
    }

    // Monitor child process exit
    {
        let app_handle = app.clone();
        let ws_id = workspace_id.clone();
        let state_clone = state.inner().clone();
        std::thread::spawn(move || {
            let _ = child.wait();
            state_clone.running_workspace_pids.lock().remove(&ws_id);
            state_clone.running_workspace_stdins.lock().remove(&ws_id);

            let mut list = state_clone.workspaces.lock();
            if let Some(w) = list.iter_mut().find(|w| w.id == ws_id) {
                w.status = "stopped".to_string();
                w.pid = None;
            }
            drop(list);
            state_clone.save_workspaces();

            let _ = app_handle.emit(
                "workspace-log",
                serde_json::json!({
                    "workspace_id": &ws_id,
                    "line": ">>> Pi 工作区会话已结束退出",
                    "is_error": false,
                }),
            );
        });
    }

    {
        let mut list = state.workspaces.lock();
        if let Some(w) = list.iter_mut().find(|w| w.id == workspace_id) {
            w.status = "ready".to_string();
            w.pid = Some(pid);
            w.last_started_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    state.save_workspaces();
    Ok(pid)
}

#[tauri::command]
pub async fn stop_workspace_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<bool, String> {
    // 1. Close stdin to signal EOF to Pi process
    state.running_workspace_stdins.lock().remove(&workspace_id);

    // 2. Terminate PID process tree
    let pid_opt = state.running_workspace_pids.lock().remove(&workspace_id);
    let mut stopped = false;

    if let Some(pid) = pid_opt {
        stopped = kill_process_tree(pid);
    }

    {
        let mut list = state.workspaces.lock();
        if let Some(w) = list.iter_mut().find(|w| w.id == workspace_id) {
            w.status = "stopped".to_string();
            w.pid = None;
        }
    }

    state.save_workspaces();

    let _ = app.emit(
        "workspace-log",
        serde_json::json!({
            "workspace_id": &workspace_id,
            "line": ">>> Pi 工作区会话已成功停止",
            "is_error": false,
        }),
    );

    Ok(stopped)
}

#[tauri::command]
pub async fn restart_workspace_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<u32, String> {
    let _ = stop_workspace_session(app.clone(), state.clone(), workspace_id.clone()).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    start_workspace_session(app, state, workspace_id).await
}

#[tauri::command]
pub fn generate_chatgpt_prompt(path: String, session_id: Option<String>) -> String {
    if let Some(sid) = session_id {
        if !sid.trim().is_empty() {
            return format!(
r#"@Chappie

我要操作项目：
{}

使用指定的 sessionId 调用 init：
init({{ sessionId: "{}" }})

随后输出当前 cwd、Git 分支及状态。"#,
                path, sid
            );
        }
    }

    format!(
r#"@Chappie

我要操作项目：
{}

执行以下步骤：
1. 调用 sessions。
2. 根据 cwd 精确找到该项目对应的 Pi Session。
3. 必须确认只有一个匹配。
4. 使用对应 sessionId 调用 init。
5. 输出当前 cwd、Git branch 和 git status。
6. 不要修改任何文件。

如果项目不存在、存在多个匹配、Pi 未在线，停止操作并告诉我当前 sessions 状态。"#,
        path
    )
}
