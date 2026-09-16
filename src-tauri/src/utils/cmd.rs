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

pub fn kill_process_tree(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        let out = execute_raw("taskkill.exe", &["/F", "/T", "/PID", &pid.to_string()], None);
        out.success
    }
    #[cfg(not(target_os = "windows"))]
    {
        let out = execute_raw("kill", &["-9", &pid.to_string()], None);
        out.success
    }
}

pub fn is_process_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        let check = format!("Get-Process -Id {} -ErrorAction SilentlyContinue", pid);
        let out = execute_powershell(&check, None);
        out.success && !out.stdout.trim().is_empty()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let out = execute_raw("kill", &["-0", &pid.to_string()], None);
        out.success
    }
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
