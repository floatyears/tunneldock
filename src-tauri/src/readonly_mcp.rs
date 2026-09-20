use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const PROTOCOL_VERSION: &str = "2025-11-25";
const MAX_FILE_BYTES: u64 = 1_048_576;
const MAX_RESULT_CHARS: usize = 60_000;
const MAX_FILES: usize = 2_000;
const MAX_WALK_ENTRIES: usize = 20_000;
const MAX_WALK_DEPTH: usize = 32;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", "vendor"];

#[derive(Clone)]
struct Workspace {
    id: String,
    name: String,
    root: PathBuf,
}

pub fn run_stdio() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.map_err(|error| format!("MCP stdin read failed: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle_message(&request),
            Err(error) => Some(json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {"code": -32700, "message": format!("Parse error: {error}")}
            })),
        };

        if let Some(response) = response {
            serde_json::to_writer(&mut out, &response)
                .map_err(|error| format!("MCP stdout write failed: {error}"))?;
            out.write_all(b"\n")
                .map_err(|error| format!("MCP stdout write failed: {error}"))?;
            out.flush()
                .map_err(|error| format!("MCP stdout flush failed: {error}"))?;
        }
    }

    Ok(())
}

fn handle_message(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or_default();

    if method.starts_with("notifications/") || method == "$/cancelRequest" {
        return None;
    }

    let Some(id) = id else {
        return None;
    };

    let result = match method {
        "initialize" => {
            let requested = request
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_VERSION);
            json!({
                "protocolVersion": requested,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {
                    "name": "tunneldock-safe",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools": tool_catalog()}),
        "tools/call" => {
            let name = request
                .pointer("/params/name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = request
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(name, &args) {
                Ok(text) if matches!(name, "read_file" | "fetch") => {
                    let snapshot: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
                    json!({
                        "content": [{"type": "text", "text": text}],
                        "structuredContent": snapshot
                    })
                }
                Ok(text) => json!({"content": [{"type": "text", "text": truncate(&text)}]}),
                Err(error) => json!({
                    "content": [{"type": "text", "text": truncate(&error)}],
                    "isError": true
                }),
            }
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("Method not found: {method}")}
            }));
        }
    };

    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn tool_catalog() -> Vec<Value> {
    let read_annotations = json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    });
    vec![
        tool("list_workspaces", "List enabled read-only workspaces by stable ID and display name.", json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }), &read_annotations),
        tool("search", "Find workspace files by a case-insensitive path substring.", schema(json!({
            "query": {"type": "string", "description": "Text to match in relative file paths."}
        }), &["workspaceId", "query"]), &read_annotations),
        tool("fetch", "Fetch the UTF-8 contents of one file from a registered workspace.", schema(json!({
            "path": {"type": "string", "description": "Workspace-relative file path."}
        }), &["workspaceId", "path"]), &read_annotations),
        tool("read_file", "Read a UTF-8 file (up to 1 MiB) from a registered workspace.", schema(json!({
            "path": {"type": "string", "description": "Workspace-relative file path."}
        }), &["workspaceId", "path"]), &read_annotations),
        tool("search_code", "Search workspace text files for a literal string.", schema(json!({
            "query": {"type": "string", "description": "Literal text to search for."},
            "path": {"type": "string", "description": "Optional workspace-relative file or directory."}
        }), &["workspaceId", "query"]), &read_annotations),
        tool("list_files", "List files beneath a workspace directory, excluding dependency and build folders.", schema(json!({
            "path": {"type": "string", "description": "Optional workspace-relative directory; defaults to the workspace root."}
        }), &["workspaceId"]), &read_annotations),
        tool("project_tree", "Show a bounded directory tree for a registered workspace.", schema(json!({
            "path": {"type": "string", "description": "Optional workspace-relative directory; defaults to the workspace root."},
            "max_depth": {"type": "integer", "minimum": 1, "maximum": 8}
        }), &["workspaceId"]), &read_annotations),
        tool("git_status", "Read the current Git branch and working tree status.", schema(json!({}), &["workspaceId"]), &read_annotations),
        tool("git_diff", "Read changes against HEAD, optionally limited to one workspace-relative path.", schema(json!({
            "path": {"type": "string", "description": "Optional workspace-relative file or directory."}
        }), &["workspaceId"]), &read_annotations),
        tool("git_log", "Read recent Git commit summaries.", schema(json!({
            "limit": {"type": "integer", "minimum": 1, "maximum": 100}
        }), &["workspaceId"]), &read_annotations),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value, annotations: &Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": annotations
    })
}

fn schema(properties: Value, required: &[&str]) -> Value {
    let mut properties = properties.as_object().cloned().unwrap_or_default();
    properties.insert(
        "workspaceId".to_string(),
        json!({
            "type": "string",
            "description": "Exact ID of an enabled workspace registered in TunnelDock."
        }),
    );
    let mut all_required = required.to_vec();
    if !all_required.contains(&"workspaceId") {
        all_required.push("workspaceId");
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": all_required,
        "additionalProperties": false
    })
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    if name == "list_workspaces" {
        return list_workspaces();
    }
    let workspace = resolve_workspace(required_string(args, "workspaceId")?)?;
    match name {
        "search" => search_files(&workspace, required_string(args, "query")?),
        "fetch" | "read_file" => read_file(&workspace, required_string(args, "path")?),
        "search_code" => search_code(
            &workspace,
            required_string(args, "query")?,
            optional_string(args, "path")?,
        ),
        "list_files" => list_files(&workspace, optional_string(args, "path")?.unwrap_or(".")),
        "project_tree" => project_tree(
            &workspace,
            optional_string(args, "path")?.unwrap_or("."),
            args.get("max_depth").and_then(Value::as_u64).unwrap_or(3).clamp(1, 8) as usize,
        ),
        "git_status" => git_output(&workspace, &["status", "--short", "--branch", "--untracked-files=no"]),
        "git_diff" => git_diff(&workspace, optional_string(args, "path")?),
        "git_log" => {
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20).clamp(1, 100);
            let limit = limit.to_string();
            git_output(&workspace, &["log", "--oneline", "--decorate", "--no-color", "-n", &limit])
        }
        _ => Err(format!("Unknown read-only tool: {name}")),
    }
}

fn resolve_workspace(selector: &str) -> Result<Workspace, String> {
    let data_dir = dirs::data_dir().ok_or_else(|| "Could not locate the user data directory".to_string())?;
    let workspaces_path = data_dir.join("TunnelDock").join("workspaces.json");
    let data = fs::read_to_string(&workspaces_path)
        .map_err(|error| format!("Could not read registered workspaces: {error}"))?;
    let entries: Vec<Value> = serde_json::from_str(&data)
        .map_err(|error| format!("Registered workspaces file is invalid: {error}"))?;

    let matches = entries
        .iter()
        .filter(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(selector)
                && entry
                    .get("mcp_access_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err("Workspace is not enabled for read-only access in TunnelDock".to_string());
    }
    if matches.len() != 1 {
        return Err("Workspace name is ambiguous; pass its workspace ID".to_string());
    }

    let entry = matches[0];
    let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
    let name = entry.get("name").and_then(Value::as_str).unwrap_or_default();
    let path = entry.get("path").and_then(Value::as_str).ok_or_else(|| {
        "Registered workspace does not have a valid path".to_string()
    })?;
    let root = fs::canonicalize(path).map_err(|error| format!("Workspace path is unavailable: {error}"))?;
    if !root.is_dir() {
        return Err("Registered workspace path is not a directory".to_string());
    }
    Ok(Workspace { id: id.to_string(), name: name.to_string(), root })
}

fn list_workspaces() -> Result<String, String> {
    let data_dir = dirs::data_dir().ok_or_else(|| "Could not locate the user data directory".to_string())?;
    let workspaces_path = data_dir.join("TunnelDock").join("workspaces.json");
    let data = fs::read_to_string(&workspaces_path)
        .map_err(|error| format!("Could not read registered workspaces: {error}"))?;
    let entries: Vec<Value> = serde_json::from_str(&data)
        .map_err(|error| format!("Registered workspaces file is invalid: {error}"))?;
    let enabled = entries
        .iter()
        .filter(|entry| {
            entry
                .get("mcp_access_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
        .filter_map(|entry| {
            Some(json!({
                "workspaceId": entry.get("id")?.as_str()?,
                "name": entry.get("name")?.as_str()?
            }))
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&enabled)
        .map_err(|error| format!("Could not encode the enabled workspace list: {error}"))
}

fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing required string argument: {key}"))
}

fn optional_string<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(format!("Argument {key} must be a string")),
    }
}

fn resolve_path(workspace: &Workspace, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|part| matches!(part, Component::ParentDir))
    {
        return Err("Only workspace-relative paths without '..' are allowed".to_string());
    }
    if relative_path.components().any(|part| match part {
        Component::Normal(name) => {
            let name = name.to_string_lossy().to_ascii_lowercase();
            SKIP_DIRS.contains(&name.as_str())
        }
        _ => false,
    }) {
        return Err("Access to dependency, build, and Git metadata directories is disabled".to_string());
    }
    let target = fs::canonicalize(workspace.root.join(relative_path))
        .map_err(|error| format!("Workspace path is unavailable: {error}"))?;
    if !target.starts_with(&workspace.root) {
        return Err("Path resolves outside the registered workspace".to_string());
    }
    Ok(target)
}

fn read_file(workspace: &Workspace, relative: &str) -> Result<String, String> {
    let path = resolve_path(workspace, relative)?;
    if !path.is_file() {
        return Err("Requested path is not a file".to_string());
    }
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err("File exceeds the 1 MiB read limit".to_string());
    }
    let bytes = fs::read(&path).map_err(|error| format!("Could not read file: {error}"))?;
    let content = String::from_utf8(bytes.clone())
        .map_err(|error| format!("Could not read UTF-8 file: {error}"))?;
    let relative_path = path
        .strip_prefix(&workspace.root)
        .map_err(|_| "Path resolves outside the registered workspace".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    serde_json::to_string(&json!({
        "path": relative_path,
        "sha256": sha256,
        "content": content
    }))
    .map_err(|error| format!("Could not encode file snapshot: {error}"))
}

fn search_files(workspace: &Workspace, query: &str) -> Result<String, String> {
    if query.trim().is_empty() {
        return Err("Search query cannot be empty".to_string());
    }
    let files = collect_files(&workspace.root, &workspace.root, MAX_FILES)?;
    let query = query.to_lowercase();
    let results = files
        .into_iter()
        .filter(|path| path.to_string_lossy().to_lowercase().contains(&query))
        .take(200)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>();
    if results.is_empty() {
        Ok("No matching workspace files".to_string())
    } else {
        Ok(results.join("\n"))
    }
}

fn list_files(workspace: &Workspace, relative: &str) -> Result<String, String> {
    let base = resolve_path(workspace, relative)?;
    if !base.is_dir() {
        return Err("Requested path is not a directory".to_string());
    }
    let files = collect_files(&workspace.root, &base, MAX_FILES)?;
    Ok(files
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn project_tree(workspace: &Workspace, relative: &str, max_depth: usize) -> Result<String, String> {
    let base = resolve_path(workspace, relative)?;
    if !base.is_dir() {
        return Err("Requested path is not a directory".to_string());
    }
    let mut lines = vec![format!("{} ({})", workspace.name, workspace.id)];
    let mut count = 0usize;
    append_tree(&workspace.root, &base, 0, max_depth, &mut lines, &mut count)?;
    Ok(lines.join("\n"))
}

fn append_tree(
    root: &Path,
    directory: &Path,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
    count: &mut usize,
) -> Result<(), String> {
    if depth >= max_depth || *count >= 500 {
        return Ok(());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("Could not list workspace directory: {error}"))?
        .take(501usize.saturating_sub(*count))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        let entry_path = entry.path();
        let relative = entry_path
            .strip_prefix(root)
            .map_err(|_| "Workspace entry escaped its registered root".to_string())?;
        lines.push(format!("{}{}", "  ".repeat(depth), relative.to_string_lossy()));
        *count += 1;
        if file_type.is_dir() {
            append_tree(root, &entry_path, depth + 1, max_depth, lines, count)?;
        }
        if *count >= 500 {
            break;
        }
    }
    Ok(())
}

fn collect_files(root: &Path, base: &Path, limit: usize) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut visited = 0;
    collect_files_into(root, base, limit, 0, &mut visited, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_into(
    root: &Path,
    directory: &Path,
    limit: usize,
    depth: usize,
    visited: &mut usize,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if files.len() >= limit || depth > MAX_WALK_DEPTH || *visited >= MAX_WALK_ENTRIES {
        return Ok(());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("Could not list workspace directory: {error}"))?
        .take(MAX_WALK_ENTRIES.saturating_sub(*visited))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        *visited += 1;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || SKIP_DIRS.contains(&name.as_ref()) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        let entry_path = entry.path();
        if file_type.is_dir() {
            collect_files_into(root, &entry_path, limit, depth + 1, visited, files)?;
        } else if file_type.is_file() {
            let relative = entry_path
                .strip_prefix(root)
                .map_err(|_| "Workspace entry escaped its registered root".to_string())?;
            files.push(relative.to_path_buf());
        }
        if files.len() >= limit {
            break;
        }
    }
    Ok(())
}

fn search_code(workspace: &Workspace, query: &str, relative: Option<&str>) -> Result<String, String> {
    if query.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }
    let base = resolve_path(workspace, relative.unwrap_or("."))?;
    if !base.is_dir() && !base.is_file() {
        return Err("Search path is neither a file nor a directory".to_string());
    }
    let files = if base.is_file() {
        vec![base]
    } else {
        collect_files(&workspace.root, &base, MAX_FILES)?
            .into_iter()
            .map(|path| workspace.root.join(path))
            .collect()
    };
    let mut matches = Vec::new();
    for path in files {
        if matches.len() >= 200 {
            break;
        }
        let Ok(metadata) = fs::metadata(&path) else { continue };
        if metadata.len() > MAX_FILE_BYTES { continue; }
        let Ok(contents) = fs::read_to_string(&path) else { continue };
        let Ok(relative_path) = path.strip_prefix(&workspace.root) else { continue };
        let relative_path = relative_path.to_string_lossy().replace('\\', "/");
        for (line_number, line) in contents.lines().enumerate() {
            if line.contains(query) {
                matches.push(format!("{}:{}:{}", relative_path, line_number + 1, line));
                if matches.len() >= 200 { break; }
            }
        }
    }
    if matches.is_empty() {
        Ok("No matching workspace text".to_string())
    } else {
        Ok(matches.join("\n"))
    }
}

fn git_diff(workspace: &Workspace, relative: Option<&str>) -> Result<String, String> {
    let mut args = vec!["diff", "--no-ext-diff", "--no-textconv", "--no-color", "HEAD", "--"];
    let path;
    if let Some(relative) = relative {
        path = resolve_path(workspace, relative)?;
        let path_rel = path.strip_prefix(&workspace.root).map_err(|_| "Path is outside workspace".to_string())?;
        args.push(path_rel.to_str().ok_or_else(|| "Diff path is not valid UTF-8".to_string())?);
    }
    git_output(workspace, &args)
}

fn git_output(workspace: &Workspace, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .arg("--no-pager")
        .arg("-C")
        .arg(&workspace.root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|error| format!("Could not start Git: {error}"))?;
    output_text(output)
}

fn output_text(output: Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match output.status.code() {
        Some(0) => Ok(if stdout.is_empty() { "No results".to_string() } else { stdout }),
        Some(1) if stderr.is_empty() => Ok(if stdout.is_empty() { "No results".to_string() } else { stdout }),
        _ => Err(if stderr.is_empty() { "Git command failed".to_string() } else { stderr }),
    }
}

fn truncate(text: &str) -> String {
    let truncated = text.chars().take(MAX_RESULT_CHARS).collect::<String>();
    if text.chars().count() > MAX_RESULT_CHARS {
        format!("{truncated}\n\n[Output truncated at {MAX_RESULT_CHARS} characters]")
    } else {
        truncated
    }
}
