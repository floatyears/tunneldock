use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tauri::{AppHandle, State};
use crate::models::WorkspaceItem;
use crate::state::AppState;
use crate::utils::cmd::{execute_cmd, is_process_running, kill_process_tree};

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
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<bool, String> {
    // Stop if running
    let _ = stop_workspace_session(state.clone(), workspace_id.clone()).await;

    let mut workspaces = state.workspaces.lock();
    workspaces.retain(|w| w.id != workspace_id);
    drop(workspaces);

    state.save_workspaces();
    Ok(true)
}

#[tauri::command]
pub async fn start_workspace_session(
    _app: AppHandle,
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
    let _ = stop_workspace_session(state.clone(), workspace_id.clone()).await;

    // Pi is an interactive coding agent (TUI) that requires an active terminal (TTY)
    // to render its UI, handle stdin/stdout, and register its session with the Chappie MCP Broker.
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("powershell.exe");
        let script = format!(
            "$Host.UI.RawUI.WindowTitle = 'Pi Session: {}'; Set-Location -LiteralPath '{}'; Write-Host '>>> 正在启动 Pi 工作区会话 [{}]...' -ForegroundColor Green; Write-Host '>>> 保持此终端窗口运行，ChatGPT 即可随时连接并操作本目录代码。' -ForegroundColor DarkGray; pi --provider chappie --model chatgpt",
            ws_name.replace('\'', "''"),
            path_str.replace('\'', "''"),
            ws_name.replace('\'', "''")
        );
        c.args(["-NoExit", "-Command", &script]);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("pi");
        c.args(["--provider", "chappie", "--model", "chatgpt"]);
        c.current_dir(&dir);
        c
    };

    let child = cmd.spawn().map_err(|e| format!("无法启动 Pi Session 终端: {}", e))?;
    let pid = child.id();

    // Track PID
    state.running_workspace_pids.lock().insert(workspace_id.clone(), pid);

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
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<bool, String> {
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
    Ok(stopped)
}

#[tauri::command]
pub async fn restart_workspace_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
) -> Result<u32, String> {
    let _ = stop_workspace_session(state.clone(), workspace_id.clone()).await;
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
