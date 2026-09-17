use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::models::CommandOutput;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn execute_raw(command: &str, args: &[&str], cwd: Option<&Path>) -> CommandOutput {
    let mut cmd = Command::new(command);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            CommandOutput {
                success: output.status.success(),
                stdout,
                stderr,
                code: output.status.code(),
            }
        }
        Err(err) => CommandOutput {
            success: false,
            stdout: String::new(),
            stderr: err.to_string(),
            code: None,
        },
    }
}

pub fn execute_cmd(command: &str, args: &[&str], cwd: Option<&Path>) -> CommandOutput {
    #[cfg(target_os = "windows")]
    {
        // On Windows, running via cmd.exe /d /s /c ensures .cmd, .bat, and npm wrappers
        // like pi.cmd are properly found and executed via PATH and PATHEXT.
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/d", "/s", "/c", command]);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.creation_flags(CREATE_NO_WINDOW);

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                CommandOutput {
                    success: output.status.success(),
                    stdout,
                    stderr,
                    code: output.status.code(),
                }
            }
            Err(err) => CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: err.to_string(),
                code: None,
            },
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        execute_raw(command, args, cwd)
    }
}

pub fn execute_powershell(script: &str, cwd: Option<&Path>) -> CommandOutput {
    execute_raw(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
        cwd,
    )
}

/// Refresh this GUI process' PATH from the Windows registry without restarting
/// the application. Package installers such as winget update the registry, but
/// an already-running Tauri process keeps the environment block it inherited at
/// startup. Preserve any process-local PATH entries after the refreshed entries
/// so development launches do not lose their temporary tool paths.
#[cfg(target_os = "windows")]
pub fn refresh_process_path() -> bool {
    let out = execute_powershell(
        "$machine=[Environment]::GetEnvironmentVariable('Path','Machine'); \
         $user=[Environment]::GetEnvironmentVariable('Path','User'); \
         [Console]::OutputEncoding=[Text.UTF8Encoding]::new(); \
         Write-Output (($machine,$user) -join ';')",
        None,
    );

    if !out.success {
        return false;
    }

    let fresh = out.stdout.trim();
    if fresh.is_empty() {
        return false;
    }

    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut seen = HashSet::new();
    let mut merged: Vec<PathBuf> = Vec::new();

    for entry in std::env::split_paths(OsStr::new(fresh))
        .chain(std::env::split_paths(&current))
    {
        if entry.as_os_str().is_empty() {
            continue;
        }
        let key = entry
            .to_string_lossy()
            .trim_end_matches(&['\\', '/'][..])
            .to_ascii_lowercase();
        if seen.insert(key) {
            merged.push(entry);
        }
    }

    match std::env::join_paths(merged) {
        Ok(path) => {
            std::env::set_var("PATH", path);
            true
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn refresh_process_path() -> bool {
    true
}

pub fn find_executable(name: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let out = execute_raw("where.exe", &[name], None);
        if out.success && !out.stdout.trim().is_empty() {
            let lines: Vec<String> = out
                .stdout
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // On Windows, prioritize .exe, then .cmd, then .bat over extensionless scripts.
            // Preserve where.exe ordering inside each class because PATH order matters.
            if let Some(exe) = lines.iter().find(|s| s.to_lowercase().ends_with(".exe")) {
                return Some(exe.clone());
            }
            if let Some(cmd_file) = lines.iter().find(|s| s.to_lowercase().ends_with(".cmd")) {
                return Some(cmd_file.clone());
            }
            if let Some(bat_file) = lines.iter().find(|s| s.to_lowercase().ends_with(".bat")) {
                return Some(bat_file.clone());
            }
            return lines.first().cloned();
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let out = execute_raw("which", &[name], None);
        if out.success && !out.stdout.trim().is_empty() {
            return out.stdout.lines().next().map(|s| s.trim().to_string());
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn active_node_is_nvm_managed() -> bool {
    if find_executable("nvm").is_none() {
        return false;
    }

    let Some(node_path) = find_executable("node") else {
        return std::env::var_os("NVM_HOME").is_some() && std::env::var_os("NVM_SYMLINK").is_some();
    };
    let Some(nvm_symlink) = std::env::var_os("NVM_SYMLINK") else {
        return false;
    };

    let node_path = node_path.replace('/', "\\").to_ascii_lowercase();
    let nvm_symlink = nvm_symlink
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase();

    node_path.starts_with(&format!("{}\\", nvm_symlink)) || node_path == format!("{}\\node.exe", nvm_symlink)
}

#[cfg(target_os = "windows")]
fn parse_semver_token(token: &str) -> Option<(u32, u32, u32, String)> {
    let cleaned = token
        .trim()
        .trim_start_matches('*')
        .trim_start_matches('v')
        .trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
    let mut parts = cleaned.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch, format!("{}.{}.{}", major, minor, patch)))
}

#[cfg(target_os = "windows")]
fn newest_installed_nvm_node() -> Option<String> {
    let out = execute_cmd("nvm", &["list"], None);
    if !out.success {
        return None;
    }

    out.stdout
        .lines()
        .flat_map(|line| line.split_whitespace())
        .filter_map(parse_semver_token)
        .max_by_key(|(major, minor, patch, _)| (*major, *minor, *patch))
        .map(|(_, _, _, version)| version)
}

#[cfg(target_os = "windows")]
fn node_major_version() -> Option<u32> {
    let out = execute_cmd("node", &["-v"], None);
    if !out.success {
        return None;
    }
    out.stdout
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|part| part.parse::<u32>().ok())
}

/// Kill a process and every currently-visible descendant without invoking a shell.
///
/// Using sysinfo here is intentional: spawning `taskkill`, `killall`, PowerShell, or
/// a platform shell from the GUI shutdown path can block the Tauri event loop and
/// was the primary cause of the app becoming "Not responding" while closing.
pub fn kill_process_tree(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let root = Pid::from_u32(pid);
    if system.process(root).is_none() {
        // The process is already gone, which is the desired post-condition.
        return true;
    }

    let mut tree = vec![root];
    loop {
        let before = tree.len();
        for (candidate_pid, process) in system.processes() {
            if let Some(parent) = process.parent() {
                if tree.contains(&parent) && !tree.contains(candidate_pid) {
                    tree.push(*candidate_pid);
                }
            }
        }
        if tree.len() == before {
            break;
        }
    }

    // Children first, parent last. `kill()` maps to the native force-termination
    // primitive on Windows, macOS and Linux and does not create a console window.
    let mut success = true;
    for process_pid in tree.into_iter().rev() {
        if let Some(process) = system.process(process_pid) {
            if !process.kill() {
                success = false;
            }
        }
    }
    success
}

pub fn is_process_running(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    system.process(Pid::from_u32(pid)).is_some()
}

/// Find a process by executable name in a platform-neutral way.
/// `.exe` is ignored so callers can consistently use names such as `otunnel`.
pub fn find_process_by_name(name: &str) -> Option<u32> {
    use sysinfo::{ProcessesToUpdate, System};

    fn normalize(value: &str) -> String {
        value
            .trim()
            .trim_end_matches(".exe")
            .to_ascii_lowercase()
    }

    let expected = normalize(name);
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    system.processes().iter().find_map(|(pid, process)| {
        let actual = normalize(&process.name().to_string_lossy());
        (actual == expected).then(|| pid.as_u32())
    })
}

pub fn run_streaming<F>(
    command: &str,
    args: &[&str],
    cwd: Option<&Path>,
    mut on_line: F,
) -> bool
where
    F: FnMut(String, bool),
{
    #[cfg(target_os = "windows")]
    {
        let is_node_winget_install = command.eq_ignore_ascii_case("winget")
            && args.iter().any(|arg| arg.eq_ignore_ascii_case("OpenJS.NodeJS"));

        // If the active Node comes from nvm-windows, installing a second standalone
        // Node with winget does not change `node` at all: PATH still resolves the
        // NVM_SYMLINK first. Upgrade the active nvm installation instead, then switch
        // the symlink so existing terminals that already contain NVM_SYMLINK also see
        // the new version immediately.
        if is_node_winget_install && active_node_is_nvm_managed() {
            on_line(
                "检测到当前 Node.js 由 nvm-windows 管理；将通过 nvm 升级活动版本，避免 winget 安装后仍命中旧版 Node。".to_string(),
                false,
            );

            if !run_streaming_inner("nvm", &["install", "latest"], cwd, &mut on_line) {
                on_line("nvm 安装最新版 Node.js 失败。".to_string(), true);
                return false;
            }

            let Some(version) = newest_installed_nvm_node() else {
                on_line("nvm 已执行安装，但无法确定刚安装的 Node.js 版本。".to_string(), true);
                return false;
            };

            on_line(format!("正在切换 nvm 活动版本到 Node.js {}...", version), false);
            if !run_streaming_inner("nvm", &["use", version.as_str()], cwd, &mut on_line) {
                on_line(
                    "nvm use 失败；请检查 nvm-windows 的符号链接权限（必要时以管理员权限运行一次 nvm use）。".to_string(),
                    true,
                );
                return false;
            }

            refresh_process_path();
            match node_major_version() {
                Some(major) if major >= 26 => {
                    let out = execute_cmd("node", &["-v"], None);
                    on_line(
                        format!("Node.js 已切换完成，当前实际版本：{}", out.stdout.trim()),
                        false,
                    );
                    return true;
                }
                Some(major) => {
                    on_line(
                        format!("nvm 切换后当前 Node.js 主版本仍为 {}，未满足 >= 26。", major),
                        true,
                    );
                    return false;
                }
                None => {
                    on_line("nvm 切换后仍无法执行 node -v。".to_string(), true);
                    return false;
                }
            }
        }
    }

    let ok = run_streaming_inner(command, args, cwd, &mut on_line);
    if ok {
        // winget/rustup/installers modify persistent environment variables, but
        // the running GUI process does not receive WM_SETTINGCHANGE as a new process
        // environment block. Refresh PATH before the frontend immediately re-checks.
        refresh_process_path();
    }
    ok
}

fn run_streaming_inner(
    command: &str,
    args: &[&str],
    cwd: Option<&Path>,
    on_line: &mut dyn FnMut(String, bool),
) -> bool {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd.exe");
        c.args(["/d", "/s", "/c", command]);
        c.args(args);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new(command);
        c.args(args);
        c
    };

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            on_line(format!("Failed to spawn command {}: {}", command, err), true);
            return false;
        }
    };

    // stdout and stderr must be drained concurrently. Cargo writes most build
    // progress to stderr; reading stdout to EOF first can fill stderr's pipe and
    // deadlock both the child and the GUI, which previously looked like an install
    // that was permanently stuck with no progress output.
    let (tx, rx) = mpsc::channel::<(String, bool)>();
    let mut readers = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let _ = tx.send((line, false));
                    }
                    Err(err) => {
                        let _ = tx.send((format!("读取 stdout 失败: {}", err), true));
                        break;
                    }
                }
            }
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let _ = tx.send((line, true));
                    }
                    Err(err) => {
                        let _ = tx.send((format!("读取 stderr 失败: {}", err), true));
                        break;
                    }
                }
            }
        }));
    }

    // Drop the original sender so the receiver closes as soon as both reader
    // threads finish after the child closes its pipes.
    drop(tx);
    let mut idle_seconds = 0u64;
    loop {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok((line, is_err)) => {
                idle_seconds = 0;
                on_line(line, is_err);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                idle_seconds += 2;
                if idle_seconds >= 10 {
                    on_line(
                        "命令仍在执行中（当前阶段暂时没有新的行输出）...".to_string(),
                        false,
                    );
                    idle_seconds = 0;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    for reader in readers {
        let _ = reader.join();
    }

    match child.wait() {
        Ok(status) => status.success(),
        Err(err) => {
            on_line(format!("Error waiting for command: {}", err), true);
            false
        }
    }
}
