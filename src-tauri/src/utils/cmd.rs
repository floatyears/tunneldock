use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::path::Path;
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
        &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script],
        cwd,
    )
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

            // On Windows, prioritize .exe, then .cmd, then .bat over extensionless scripts
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

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                on_line(l, false);
            }
        }
    }

    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(l) = line {
                on_line(l, true);
            }
        }
    }

    match child.wait() {
        Ok(status) => status.success(),
        Err(err) => {
            on_line(format!("Error waiting for command: {}", err), true);
            false
        }
    }
}
