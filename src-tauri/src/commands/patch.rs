use crate::models::WorkspaceItem;
use crate::state::AppState;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct PatchFilePreview {
    pub path: String,
    pub change_type: String,
    pub additions: usize,
    pub deletions: usize,
    pub expected_sha256: Option<String>,
    pub current_sha256: Option<String>,
    pub hash_matches: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchPreview {
    pub files: Vec<PatchFilePreview>,
    pub can_apply: bool,
    pub validation_message: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchApplyResult {
    pub applied: bool,
    pub message: String,
    pub files: Vec<PatchFilePreview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeType {
    Modified,
    Added,
    Deleted,
}

impl ChangeType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
        }
    }
}

#[derive(Debug)]
struct PreparedPatch {
    apply_diff: String,
    display_diff: String,
    files: Vec<PatchFilePreview>,
}

#[derive(Debug)]
struct CustomHunk {
    anchor: Option<String>,
    lines: Vec<String>,
}

#[derive(Debug)]
struct CustomFile {
    path: String,
    operation: ChangeType,
    expected_sha256: Option<String>,
    hunks: Vec<CustomHunk>,
    added_lines: Vec<String>,
    additions: usize,
    deletions: usize,
}

#[derive(Debug)]
struct UnifiedFile {
    path: String,
    change_type: ChangeType,
    additions: usize,
    deletions: usize,
}

#[tauri::command]
pub fn preview_patch(
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
    patch_text: String,
) -> Result<PatchPreview, String> {
    let (_, root) = resolve_workspace(&state, &workspace_id)?;
    let prepared = prepare_patch(&root, &patch_text)?;
    if let Some((path, expected, current)) = prepared
        .files
        .iter()
        .find(|file| !file.hash_matches)
        .map(|file| {
            (
                file.path.clone(),
                file.expected_sha256
                    .clone()
                    .unwrap_or_else(|| "missing".to_string()),
                file.current_sha256
                    .clone()
                    .unwrap_or_else(|| "absent".to_string()),
            )
        })
    {
        let validation_message = format!(
            "File changed since ChatGPT read it, or the patch has no required base SHA-256: {}. Expected: {}; current: {}.",
            path, expected, current
        );
        return Ok(PatchPreview {
            files: prepared.files,
            can_apply: false,
            validation_message,
            diff: prepared.display_diff,
        });
    }

    match run_git_apply(&root, &prepared.apply_diff, true) {
        Ok(()) => Ok(PatchPreview {
            files: prepared.files,
            can_apply: true,
            validation_message: "SHA-256 checks passed and git apply --check accepted this patch."
                .to_string(),
            diff: prepared.display_diff,
        }),
        Err(error) => Ok(PatchPreview {
            files: prepared.files,
            can_apply: false,
            validation_message: format!("git apply --check rejected this patch: {error}"),
            diff: prepared.display_diff,
        }),
    }
}

#[tauri::command]
pub fn apply_patch(
    state: State<'_, Arc<AppState>>,
    workspace_id: String,
    patch_text: String,
) -> Result<PatchApplyResult, String> {
    let (_, root) = resolve_workspace(&state, &workspace_id)?;
    let prepared = prepare_patch(&root, &patch_text)?;
    if let Some(file) = prepared.files.iter().find(|file| !file.hash_matches) {
        let expected = file.expected_sha256.as_deref().unwrap_or("missing");
        let current = file.current_sha256.as_deref().unwrap_or("absent");
        return Err(format!(
            "File changed since ChatGPT read it: {}\nExpected: {}\nCurrent: {}",
            file.path, expected, current
        ));
    }

    run_git_apply(&root, &prepared.apply_diff, true)?;
    // Re-read every base immediately before applying to catch edits made while
    // the user was reviewing the diff.
    ensure_hashes_still_match(&root, &prepared.files)?;
    run_git_apply(&root, &prepared.apply_diff, false)?;
    Ok(PatchApplyResult {
        applied: true,
        message: format!("Applied changes to {} file(s).", prepared.files.len()),
        files: prepared.files,
    })
}

fn resolve_workspace(
    state: &AppState,
    workspace_id: &str,
) -> Result<(WorkspaceItem, PathBuf), String> {
    let workspace = state
        .workspaces
        .lock()
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .cloned()
        .ok_or_else(|| "Select a workspace registered in TunnelDock".to_string())?;
    let root = fs::canonicalize(&workspace.path)
        .map_err(|error| format!("Workspace path is unavailable: {error}"))?;
    if !root.is_dir() {
        return Err("Workspace path is not a directory".to_string());
    }
    Ok((workspace, root))
}

fn prepare_patch(root: &Path, patch_text: &str) -> Result<PreparedPatch, String> {
    if patch_text.trim().is_empty() {
        return Err("Paste a patch before previewing it".to_string());
    }
    if patch_text.len() > 2 * 1024 * 1024 {
        return Err("Patch is larger than the 2 MiB limit".to_string());
    }
    if patch_text.trim_start().starts_with("*** Begin Patch") {
        prepare_custom_patch(root, patch_text)
    } else {
        prepare_unified_patch(root, patch_text)
    }
}

fn prepare_custom_patch(root: &Path, patch_text: &str) -> Result<PreparedPatch, String> {
    let custom_files = parse_custom_patch(patch_text)?;
    let mut generated_diff = String::new();
    let mut files = Vec::with_capacity(custom_files.len());
    let mut all_hashes_match = true;

    for file in custom_files {
        let target = resolve_patch_path(root, &file.path, file.operation == ChangeType::Added)?;
        let current_bytes = read_optional_file(&target, &file.path)?;
        let current_sha = current_bytes.as_ref().map(|bytes| sha256(bytes));
        let expected_sha = match file.operation {
            ChangeType::Added => Some("ABSENT".to_string()),
            ChangeType::Modified | ChangeType::Deleted => file.expected_sha256.clone(),
        };
        let hash_matches = match file.operation {
            ChangeType::Added => current_bytes.is_none(),
            ChangeType::Modified | ChangeType::Deleted => expected_sha
                .as_deref()
                .zip(current_sha.as_deref())
                .is_some_and(|(expected, current)| expected.eq_ignore_ascii_case(current)),
        };
        all_hashes_match &= hash_matches;
        let deleted_lines = if file.operation == ChangeType::Deleted {
            current_bytes
                .as_deref()
                .map(count_text_lines)
                .unwrap_or_default()
        } else {
            file.deletions
        };

        if hash_matches {
            match file.operation {
                ChangeType::Added => {
                    let contents = file.added_lines.join("\n");
                    let contents = if file.added_lines.is_empty() {
                        String::new()
                    } else {
                        format!("{contents}\n")
                    };
                    generated_diff.push_str(&full_file_diff(&file.path, None, Some(&contents))?);
                }
                ChangeType::Deleted => {
                    if !file.hunks.is_empty() || !file.added_lines.is_empty() {
                        return Err(format!("Delete block for {} cannot contain hunks", file.path));
                    }
                    let bytes = current_bytes.as_ref().ok_or_else(|| {
                        format!("File to delete does not exist: {}", file.path)
                    })?;
                    let contents = String::from_utf8(bytes.clone())
                        .map_err(|_| format!("File is not UTF-8 text: {}", file.path))?;
                    generated_diff.push_str(&full_file_diff(
                        &file.path,
                        Some(&contents),
                        None,
                    )?);
                }
                ChangeType::Modified => {
                    if !file.added_lines.is_empty() {
                        return Err(format!(
                            "Update block for {} has content outside a @@ hunk",
                            file.path
                        ));
                    }
                    let bytes = current_bytes.as_ref().ok_or_else(|| {
                        format!("File to update does not exist: {}", file.path)
                    })?;
                    let contents = String::from_utf8(bytes.clone())
                        .map_err(|_| format!("File is not UTF-8 text: {}", file.path))?;
                    let changed = apply_custom_hunks(&contents, &file.hunks, &file.path)?;
                    if changed == contents {
                        return Err(format!("Update block for {} makes no changes", file.path));
                    }
                    generated_diff.push_str(&full_file_diff(
                        &file.path,
                        Some(&contents),
                        Some(&changed),
                    )?);
                }
            }
        }

        files.push(PatchFilePreview {
            path: file.path,
            change_type: file.operation.as_str().to_string(),
            additions: file.additions,
            deletions: deleted_lines,
            expected_sha256: expected_sha,
            current_sha256: current_sha,
            hash_matches,
        });
    }

    let display_diff = if all_hashes_match {
        generated_diff.clone()
    } else {
        patch_text.to_string()
    };
    Ok(PreparedPatch {
        apply_diff: if all_hashes_match {
            generated_diff
        } else {
            String::new()
        },
        display_diff,
        files,
    })
}

fn parse_custom_patch(patch_text: &str) -> Result<Vec<CustomFile>, String> {
    let lines = patch_text
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    if lines.first().copied() != Some("*** Begin Patch")
        || lines.last().copied() != Some("*** End Patch")
    {
        return Err(
            "Custom patches must start with *** Begin Patch and end with *** End Patch"
                .to_string(),
        );
    }

    let mut files = Vec::new();
    let mut current: Option<CustomFile> = None;
    let mut current_hunk: Option<CustomHunk> = None;

    for line in lines.iter().skip(1).take(lines.len().saturating_sub(2)) {
        let operation = if let Some(path) = line.strip_prefix("*** Update File: ") {
            Some((path.trim(), ChangeType::Modified))
        } else if let Some(path) = line.strip_prefix("*** Add File: ") {
            Some((path.trim(), ChangeType::Added))
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            Some((path.trim(), ChangeType::Deleted))
        } else {
            None
        };

        if let Some((path, operation)) = operation {
            flush_custom_hunk(&mut current, &mut current_hunk);
            if let Some(file) = current.take() {
                files.push(file);
            }
            validate_patch_path_text(path)?;
            current = Some(CustomFile {
                path: path.to_string(),
                operation,
                expected_sha256: None,
                hunks: Vec::new(),
                added_lines: Vec::new(),
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        if let Some(hash) = line.strip_prefix("*** Base SHA256: ") {
            let file = current
                .as_mut()
                .ok_or_else(|| "Base SHA256 must follow a file header".to_string())?;
            if file.expected_sha256.is_some() {
                return Err(format!("Duplicate Base SHA256 for {}", file.path));
            }
            let hash = hash.trim();
            if hash != "ABSENT" && !valid_sha256(hash) {
                return Err(format!("Invalid SHA-256 value for {}", file.path));
            }
            file.expected_sha256 = Some(hash.to_string());
            continue;
        }

        if line.starts_with("@@") {
            flush_custom_hunk(&mut current, &mut current_hunk);
            if current.is_none() {
                return Err("Hunk appears before a file header".to_string());
            }
            let anchor = line
                .strip_prefix("@@")
                .unwrap_or_default()
                .trim()
                .trim_end_matches("@@")
                .trim();
            current_hunk = Some(CustomHunk {
                anchor: if anchor.is_empty() {
                    None
                } else {
                    Some(anchor.to_string())
                },
                lines: Vec::new(),
            });
            continue;
        }

        if *line == "*** End of File" {
            continue;
        }

        let file = current
            .as_mut()
            .ok_or_else(|| "Patch content appears before a file header".to_string())?;
        if file.operation == ChangeType::Added && current_hunk.is_none() {
            if let Some(content) = line.strip_prefix('+') {
                file.added_lines.push(content.to_string());
                file.additions += 1;
            } else if line.is_empty() {
                continue;
            } else {
                return Err(format!(
                    "Added-file patch lines must begin with + for {}",
                    file.path
                ));
            }
            continue;
        }

        let Some(hunk) = current_hunk.as_mut() else {
            if line.is_empty() {
                continue;
            }
            return Err(format!("Expected a @@ hunk before patch lines for {}", file.path));
        };
        if line.starts_with('+') {
            file.additions += 1;
        } else if line.starts_with('-') {
            file.deletions += 1;
        } else if line.starts_with(' ') || line.is_empty() || line.starts_with('\\') {
            // A blank context line is represented by a single space in normal
            // diffs; tolerate a zero-width blank line as the same context.
        } else {
            return Err(format!("Invalid hunk line for {}: {line}", file.path));
        }
        hunk.lines.push(if line.is_empty() {
            " ".to_string()
        } else {
            line.to_string()
        });
    }
    flush_custom_hunk(&mut current, &mut current_hunk);
    if let Some(file) = current {
        files.push(file);
    }
    if files.is_empty() {
        return Err("Patch has no file changes".to_string());
    }

    let mut seen = HashSet::new();
    for file in &files {
        if !seen.insert(file.path.clone()) {
            return Err(format!("Patch contains more than one block for {}", file.path));
        }
        match file.operation {
            ChangeType::Added => {
                if !file.hunks.is_empty() {
                    return Err(format!("Add block for {} cannot use @@ hunks", file.path));
                }
                if file
                    .expected_sha256
                    .as_deref()
                    .is_some_and(|hash| !hash.eq_ignore_ascii_case("ABSENT"))
                {
                    return Err(format!(
                        "Added file {} must use Base SHA256: ABSENT",
                        file.path
                    ));
                }
            }
            ChangeType::Modified | ChangeType::Deleted => {
                let hash = file.expected_sha256.as_deref().ok_or_else(|| {
                    format!(
                        "Every update and delete block needs *** Base SHA256: <hash> for {}",
                        file.path
                    )
                })?;
                if !valid_sha256(hash) {
                    return Err(format!("Invalid SHA-256 value for {}", file.path));
                }
            }
        }
        if file.operation == ChangeType::Modified && file.hunks.is_empty() {
            return Err(format!("Update block for {} has no hunks", file.path));
        }
        if file.operation == ChangeType::Modified && file.additions + file.deletions == 0 {
            return Err(format!("Update block for {} does not change any lines", file.path));
        }
    }
    Ok(files)
}

fn flush_custom_hunk(current: &mut Option<CustomFile>, hunk: &mut Option<CustomHunk>) {
    if let (Some(file), Some(hunk)) = (current.as_mut(), hunk.take()) {
        if !hunk.lines.is_empty() {
            file.hunks.push(hunk);
        }
    }
}

fn apply_custom_hunks(contents: &str, hunks: &[CustomHunk], path: &str) -> Result<String, String> {
    let newline = if contents.contains("\r\n") { "\r\n" } else { "\n" };
    let normalized = contents.replace("\r\n", "\n");
    let has_final_newline = normalized.ends_with('\n');
    let mut source = if normalized.is_empty() {
        Vec::new()
    } else {
        normalized.split('\n').map(str::to_string).collect::<Vec<_>>()
    };
    if has_final_newline {
        source.pop();
    }

    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut cursor = 0usize;
    for hunk in hunks {
        let mut old = Vec::new();
        let mut new = Vec::new();
        for line in &hunk.lines {
            if line.starts_with("\\ No newline at end of file") {
                continue;
            }
            if let Some(line) = line.strip_prefix(' ') {
                old.push(line.to_string());
                new.push(line.to_string());
            } else if let Some(line) = line.strip_prefix('-') {
                old.push(line.to_string());
            } else if let Some(line) = line.strip_prefix('+') {
                new.push(line.to_string());
            } else {
                return Err(format!("Invalid hunk line for {path}: {line}"));
            }
        }
        if old.is_empty() && new.is_empty() {
            return Err(format!("Empty hunk for {path}"));
        }
        let (start, old_len) = if old.is_empty() {
            if source.is_empty() && hunk.anchor.is_none() {
                (0, 0)
            } else {
                let anchor = hunk.anchor.as_deref().ok_or_else(|| {
                    format!("Insertion hunk for {path} needs a context line after @@")
                })?;
                let candidates = (cursor..source.len())
                    .filter(|index| source[*index].contains(anchor))
                    .collect::<Vec<_>>();
                match candidates.as_slice() {
                    [index] => (*index + 1, 0),
                    [] => return Err(format!("Hunk anchor no longer matches {path}")),
                    _ => return Err(format!("Hunk anchor is ambiguous in {path}")),
                }
            }
        } else {
            let candidates = (cursor..=source.len().saturating_sub(old.len()))
                .filter(|start| source.get(*start..*start + old.len()) == Some(old.as_slice()))
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [start] => (*start, old.len()),
                [] => return Err(format!("Hunk context no longer matches {path}")),
                _ => {
                    return Err(format!(
                        "Hunk context is ambiguous in {path}; include more context lines"
                    ))
                }
            }
        };
        cursor = start + old_len;
        replacements.push((start, old_len, new));
    }

    for (start, old_len, new_lines) in replacements.into_iter().rev() {
        source.splice(start..start + old_len, new_lines);
    }
    let mut result = source.join(newline);
    if has_final_newline && !result.is_empty() {
        result.push_str(newline);
    }
    Ok(result)
}

fn full_file_diff(path: &str, old: Option<&str>, new: Option<&str>) -> Result<String, String> {
    validate_patch_path_text(path)?;
    let eol = if old
        .into_iter()
        .chain(new)
        .any(|contents| contents.contains("\r\n"))
    {
        "\r\n"
    } else {
        "\n"
    };
    let old_path = if old.is_some() {
        format_git_path('a', path)?
    } else {
        "/dev/null".to_string()
    };
    let new_path = if new.is_some() {
        format_git_path('b', path)?
    } else {
        "/dev/null".to_string()
    };
    let mut output = format!(
        "diff --git {} {}{eol}",
        format_git_path('a', path)?,
        format_git_path('b', path)?
    );
    if old.is_none() {
        output.push_str(&format!("new file mode 100644{eol}"));
    } else if new.is_none() {
        output.push_str(&format!("deleted file mode 100644{eol}"));
    }
    output.push_str(&format!("--- {old_path}{eol}+++ {new_path}{eol}"));
    let old_lines = old.map(split_lines).unwrap_or_default();
    let new_lines = new.map(split_lines).unwrap_or_default();
    if old_lines.is_empty() && new_lines.is_empty() {
        return Ok(output);
    }
    if old.is_some() && new.is_some() && old_lines == new_lines {
        return Err(format!("Update block for {path} does not change any text"));
    }

    // Use a compact hunk around the changed region for normal newline-terminated
    // text. Fall back to a whole-file hunk when EOF newline markers matter.
    let can_trim = old.is_some_and(|contents| contents.ends_with('\n'))
        && new.is_some_and(|contents| contents.ends_with('\n'));
    let mut common_prefix = 0usize;
    let mut common_suffix = 0usize;
    if can_trim {
        while common_prefix < old_lines.len().min(new_lines.len())
            && old_lines[common_prefix] == new_lines[common_prefix]
        {
            common_prefix += 1;
        }
        while common_suffix < old_lines.len().saturating_sub(common_prefix)
            && common_suffix < new_lines.len().saturating_sub(common_prefix)
            && old_lines[old_lines.len() - common_suffix - 1]
                == new_lines[new_lines.len() - common_suffix - 1]
        {
            common_suffix += 1;
        }
    }
    let context_before = common_prefix.min(if can_trim { 3 } else { 0 });
    let context_after = common_suffix.min(if can_trim { 3 } else { 0 });
    let old_begin = common_prefix - context_before;
    let new_begin = common_prefix - context_before;
    let old_change_end = old_lines.len() - common_suffix;
    let new_change_end = new_lines.len() - common_suffix;
    let old_end = old_change_end + context_after;
    let new_end = new_change_end + context_after;
    let old_hunk_count = old_end - old_begin;
    let new_hunk_count = new_end - new_begin;
    let old_start = if old_hunk_count == 0 {
        old_begin
    } else {
        old_begin + 1
    };
    let new_start = if new_hunk_count == 0 {
        new_begin
    } else {
        new_begin + 1
    };
    output.push_str(&format!(
        "@@ -{old_start},{} +{new_start},{} @@{eol}",
        old_hunk_count,
        new_hunk_count
    ));
    for line in &old_lines[old_begin..common_prefix] {
        append_diff_line(&mut output, ' ', line, eol);
    }
    for line in &old_lines[common_prefix..old_change_end] {
        append_diff_line(&mut output, '-', line, eol);
    }
    for line in &new_lines[common_prefix..new_change_end] {
        append_diff_line(&mut output, '+', line, eol);
    }
    for line in &old_lines[old_change_end..old_end] {
        append_diff_line(&mut output, ' ', line, eol);
    }
    if !can_trim {
        if old.is_some_and(|contents| !contents.ends_with('\n')) && !old_lines.is_empty() {
            output.push_str("\\ No newline at end of file");
            output.push_str(eol);
        }
        if new.is_some_and(|contents| !contents.ends_with('\n')) && !new_lines.is_empty() {
            output.push_str("\\ No newline at end of file");
            output.push_str(eol);
        }
    }
    Ok(output)
}

fn append_diff_line(output: &mut String, prefix: char, line: &str, eol: &str) {
    output.push(prefix);
    output.push_str(line);
    output.push_str(eol);
}

fn format_git_path(prefix: char, path: &str) -> Result<String, String> {
    let value = format!("{prefix}/{path}");
    if value
        .chars()
        .any(|character| character.is_whitespace() || character == '"' || character == '\\')
    {
        serde_json::to_string(&value).map_err(|error| format!("Could not quote patch path: {error}"))
    } else {
        Ok(value)
    }
}

fn split_lines(contents: &str) -> Vec<String> {
    if contents.is_empty() {
        return Vec::new();
    }
    let normalized = contents.replace("\r\n", "\n");
    let mut lines = normalized.split('\n').map(str::to_string).collect::<Vec<_>>();
    if normalized.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn prepare_unified_patch(root: &Path, patch_text: &str) -> Result<PreparedPatch, String> {
    let mut expected_hashes = HashMap::new();
    let mut clean_lines = Vec::new();
    for line in patch_text.lines() {
        if let Some(metadata) = line.strip_prefix("# base_sha256:") {
            let (path, hash) = parse_hash_metadata(metadata.trim())?;
            validate_patch_path_text(&path)?;
            if hash != "ABSENT" && !valid_sha256(&hash) {
                return Err(format!("Invalid SHA-256 value for {path}"));
            }
            if expected_hashes.insert(path.clone(), hash).is_some() {
                return Err(format!("Duplicate base SHA-256 metadata for {path}"));
            }
        } else {
            clean_lines.push(line.trim_end_matches('\r'));
        }
    }
    let clean_patch = format!("{}\n", clean_lines.join("\n"));
    let parsed_files = parse_unified_headers_and_stats(&clean_lines)?;
    let mut files = Vec::with_capacity(parsed_files.len());
    let mut parsed_paths = HashSet::new();

    for parsed in parsed_files {
        if !parsed_paths.insert(parsed.path.clone()) {
            return Err(format!("Patch contains more than one block for {}", parsed.path));
        }
        let target = resolve_patch_path(root, &parsed.path, parsed.change_type == ChangeType::Added)?;
        let current_bytes = read_optional_file(&target, &parsed.path)?;
        let current_sha = current_bytes.as_ref().map(|bytes| sha256(bytes));
        let supplied_hash = expected_hashes.remove(&parsed.path);
        let expected_sha = match parsed.change_type {
            ChangeType::Added => {
                if supplied_hash
                    .as_deref()
                    .is_some_and(|hash| !hash.eq_ignore_ascii_case("ABSENT"))
                {
                    return Err(format!("Added file {} must have base SHA256 ABSENT", parsed.path));
                }
                Some("ABSENT".to_string())
            }
            ChangeType::Modified | ChangeType::Deleted => supplied_hash,
        };
        let hash_matches = match parsed.change_type {
            ChangeType::Added => current_bytes.is_none(),
            ChangeType::Modified | ChangeType::Deleted => expected_sha
                .as_deref()
                .zip(current_sha.as_deref())
                .is_some_and(|(expected, current)| expected.eq_ignore_ascii_case(current)),
        };
        files.push(PatchFilePreview {
            path: parsed.path,
            change_type: parsed.change_type.as_str().to_string(),
            additions: parsed.additions,
            deletions: parsed.deletions,
            expected_sha256: expected_sha,
            current_sha256: current_sha,
            hash_matches,
        });
    }
    if !expected_hashes.is_empty() {
        return Err(format!(
            "Base SHA-256 metadata has no matching patch file: {}",
            expected_hashes.keys().next().unwrap()
        ));
    }
    if files.is_empty() {
        return Err("No supported unified diff file changes were found".to_string());
    }
    Ok(PreparedPatch {
        apply_diff: clean_patch.clone(),
        display_diff: clean_patch,
        files,
    })
}

fn parse_unified_headers_and_stats(lines: &[&str]) -> Result<Vec<UnifiedFile>, String> {
    let mut files = Vec::new();
    let mut old_path: Option<Option<String>> = None;
    let mut new_path: Option<Option<String>> = None;
    let mut diff_path: Option<String> = None;
    let mut additions = 0usize;
    let mut deletions = 0usize;
    let mut in_hunk = false;
    let mut remaining_old = 0usize;
    let mut remaining_new = 0usize;

    for line in lines {
        if let Some(mode) = line
            .strip_prefix("new mode ")
            .or_else(|| line.strip_prefix("old mode "))
            .or_else(|| line.strip_prefix("new file mode "))
            .or_else(|| line.strip_prefix("deleted file mode "))
        {
            validate_regular_mode(mode.trim())?;
            continue;
        }
        if let Some(mode) = line
            .strip_prefix("index ")
            .and_then(|index| index.split_whitespace().last())
            .filter(|mode| mode.len() == 6 && mode.chars().all(|ch| ch.is_ascii_digit()))
        {
            validate_regular_mode(mode)?;
        }
        if line.starts_with("GIT binary patch") || line.starts_with("Binary files ") {
            return Err("Binary patches are not supported in Patch Inbox".to_string());
        }
        if line.starts_with("diff --git ") {
            if in_hunk && (remaining_old != 0 || remaining_new != 0) {
                return Err("Unified diff ended before its hunk was complete".to_string());
            }
            in_hunk = false;
            finish_unified_file(
                &mut files,
                &mut old_path,
                &mut new_path,
                &mut diff_path,
                &mut additions,
                &mut deletions,
            )?;
            diff_path = Some(parse_diff_git_path(line)?);
            continue;
        }
        if line.starts_with("@@") {
            let (old_count, new_count) = parse_hunk_counts(line)?;
            remaining_old = old_count;
            remaining_new = new_count;
            in_hunk = old_count != 0 || new_count != 0;
            continue;
        }
        if in_hunk {
            if line.starts_with("\\ No newline at end of file") {
                continue;
            }
            let kind = line.chars().next().ok_or_else(|| {
                "Malformed unified diff hunk: an empty line has no diff prefix".to_string()
            })?;
            match kind {
                ' ' => {
                    if remaining_old == 0 || remaining_new == 0 {
                        return Err("Unified diff hunk has more context than declared".to_string());
                    }
                    remaining_old -= 1;
                    remaining_new -= 1;
                }
                '+' => {
                    if remaining_new == 0 {
                        return Err("Unified diff hunk has more additions than declared".to_string());
                    }
                    remaining_new -= 1;
                    additions += 1;
                }
                '-' => {
                    if remaining_old == 0 {
                        return Err("Unified diff hunk has more deletions than declared".to_string());
                    }
                    remaining_old -= 1;
                    deletions += 1;
                }
                _ => return Err("Malformed unified diff hunk".to_string()),
            }
            if remaining_old == 0 && remaining_new == 0 {
                in_hunk = false;
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("--- ") {
            if old_path.is_some() || new_path.is_some() {
                if old_path.is_none() || new_path.is_none() {
                    return Err("Unified diff file headers are incomplete".to_string());
                }
                finish_unified_file(
                    &mut files,
                    &mut old_path,
                    &mut new_path,
                    &mut diff_path,
                    &mut additions,
                    &mut deletions,
                )?;
            }
            old_path = Some(parse_unified_path(path, 'a')?);
            continue;
        }
        if let Some(path) = line.strip_prefix("+++ ") {
            if old_path.is_none() {
                return Err("Unified diff has a new-file path without an old-file path".to_string());
            }
            new_path = Some(parse_unified_path(path, 'b')?);
        }
    }

    if in_hunk && (remaining_old != 0 || remaining_new != 0) {
        return Err("Unified diff ended before its hunk was complete".to_string());
    }
    finish_unified_file(
        &mut files,
        &mut old_path,
        &mut new_path,
        &mut diff_path,
        &mut additions,
        &mut deletions,
    )?;
    Ok(files)
}

fn validate_regular_mode(mode: &str) -> Result<(), String> {
    if mode == "100644" || mode == "100755" {
        Ok(())
    } else {
        Err(format!(
            "Patch file mode {mode} is unsupported; Patch Inbox accepts regular files only"
        ))
    }
}

fn finish_unified_file(
    files: &mut Vec<UnifiedFile>,
    old_path: &mut Option<Option<String>>,
    new_path: &mut Option<Option<String>>,
    diff_path: &mut Option<String>,
    additions: &mut usize,
    deletions: &mut usize,
) -> Result<(), String> {
    if old_path.is_none() && new_path.is_none() {
        if diff_path.is_some() {
            return Err("Git diff header is missing its file headers".to_string());
        }
        return Ok(());
    }
    let old = old_path
        .take()
        .ok_or_else(|| "Unified diff block is missing its old path".to_string())?;
    let new = new_path
        .take()
        .ok_or_else(|| "Unified diff block is missing its new path".to_string())?;
    let (path, change_type) = match (old, new) {
        (Some(old), Some(new)) if old == new => (old, ChangeType::Modified),
        (None, Some(new)) => (new, ChangeType::Added),
        (Some(old), None) => (old, ChangeType::Deleted),
        (Some(_), Some(_)) => {
            return Err("Rename and copy patches are not supported in Patch Inbox".to_string())
        }
        (None, None) => return Err("Patch block has no target file".to_string()),
    };
    validate_patch_path_text(&path)?;
    if let Some(header_path) = diff_path.take() {
        if header_path != path {
            return Err(format!(
                "Git diff header path does not match file headers for {path}"
            ));
        }
    }
    files.push(UnifiedFile {
        path,
        change_type,
        additions: *additions,
        deletions: *deletions,
    });
    *additions = 0;
    *deletions = 0;
    Ok(())
}

fn parse_diff_git_path(line: &str) -> Result<String, String> {
    let rest = line
        .strip_prefix("diff --git ")
        .ok_or_else(|| "Invalid diff --git header".to_string())?;
    let (old, rest) = take_git_path(rest)?;
    let (new, rest) = take_git_path(rest)?;
    if !rest.trim().is_empty() {
        return Err("Unexpected text after diff --git paths".to_string());
    }
    let old = strip_git_path_prefix(&old, 'a')?;
    let new = strip_git_path_prefix(&new, 'b')?;
    if old != new {
        return Err("Rename and copy patches are not supported in Patch Inbox".to_string());
    }
    Ok(old)
}

fn take_git_path(input: &str) -> Result<(String, &str), String> {
    let input = input.trim_start();
    if input.starts_with('"') {
        let closing = quoted_path_end(input)
            .ok_or_else(|| "Invalid quoted path in diff --git header".to_string())?;
        let path = serde_json::from_str::<String>(&input[..=closing])
            .map_err(|_| "Quoted Git paths must use JSON-compatible escaping".to_string())?;
        Ok((path, &input[closing + 1..]))
    } else {
        let end = input
            .find(char::is_whitespace)
            .unwrap_or(input.len());
        if end == 0 {
            return Err("diff --git header is missing a path".to_string());
        }
        Ok((input[..end].to_string(), &input[end..]))
    }
}

fn strip_git_path_prefix(path: &str, expected: char) -> Result<String, String> {
    let prefix = format!("{expected}/");
    let path = path
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("Git diff paths must begin with {prefix}"))?;
    validate_patch_path_text(path)?;
    Ok(path.to_string())
}

fn parse_hunk_counts(header: &str) -> Result<(usize, usize), String> {
    let fields = header
        .split("@@")
        .nth(1)
        .ok_or_else(|| "Invalid unified diff hunk header".to_string())?;
    let old = fields
        .split_whitespace()
        .find(|field| field.starts_with('-'))
        .ok_or_else(|| "Unified hunk is missing its old range".to_string())?;
    let new = fields
        .split_whitespace()
        .find(|field| field.starts_with('+'))
        .ok_or_else(|| "Unified hunk is missing its new range".to_string())?;
    let count = |range: &str| -> Result<usize, String> {
        match range.split_once(',') {
            Some((_, count)) => count
                .parse()
                .map_err(|_| "Invalid hunk line count".to_string()),
            None => Ok(1),
        }
    };
    Ok((count(&old[1..])?, count(&new[1..])?))
}

fn parse_unified_path(header_path: &str, expected_prefix: char) -> Result<Option<String>, String> {
    let header_path = header_path.trim();
    let value = if header_path.starts_with('"') {
        let closing = quoted_path_end(header_path)
            .ok_or_else(|| "Invalid quoted path in unified diff".to_string())?;
        serde_json::from_str::<String>(&header_path[..=closing])
            .map_err(|_| "Quoted file paths must use JSON-compatible escaping".to_string())?
    } else {
        header_path
            .split('\t')
            .next()
            .unwrap_or(header_path)
            .to_string()
    };
    if value == "/dev/null" {
        return Ok(None);
    }
    let prefix = format!("{expected_prefix}/");
    let path = value
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("Unified diff paths must begin with {prefix}"))?;
    validate_patch_path_text(path)?;
    Ok(Some(path.to_string()))
}

fn quoted_path_end(value: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(index);
        }
    }
    None
}

fn parse_hash_metadata(metadata: &str) -> Result<(String, String), String> {
    let (path, hash) = if let Some((path, hash)) = metadata.rsplit_once('=') {
        (path, hash)
    } else {
        let separator = metadata
            .rfind(char::is_whitespace)
            .ok_or_else(|| "Use '# base_sha256: path sha256' before unified diff blocks".to_string())?;
        (&metadata[..separator], &metadata[separator + 1..])
    };
    if path.trim().is_empty() || hash.trim().is_empty() {
        return Err("Use '# base_sha256: path sha256' before unified diff blocks".to_string());
    }
    Ok((path.trim().to_string(), hash.trim().to_string()))
}

fn resolve_patch_path(root: &Path, relative: &str, allow_missing: bool) -> Result<PathBuf, String> {
    validate_patch_path_text(relative)?;
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::CurDir
            )
        })
    {
        return Err(format!("Patch path is not workspace-relative: {relative}"));
    }
    if path.components().any(|component| match component {
        Component::Normal(part) => part.to_string_lossy().eq_ignore_ascii_case(".git"),
        _ => false,
    }) {
        return Err("Patches cannot modify Git metadata".to_string());
    }
    let joined = root.join(path);
    match fs::symlink_metadata(&joined) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!("Patch target cannot be a symbolic link: {relative}"));
            }
            let canonical = fs::canonicalize(&joined)
                .map_err(|error| format!("Could not resolve patch path {relative}: {error}"))?;
            if !canonical.starts_with(root) {
                return Err(format!(
                    "Patch path resolves outside the selected workspace: {relative}"
                ));
            }
            if !metadata.is_file() {
                return Err(format!("Patch target is not a file: {relative}"));
            }
            Ok(canonical)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_missing => {
            let parent = joined
                .parent()
                .ok_or_else(|| format!("Patch target has no parent directory: {relative}"))?;
            let canonical_parent = fs::canonicalize(parent).map_err(|error| {
                format!("Patch parent directory is unavailable for {relative}: {error}")
            })?;
            if !canonical_parent.starts_with(root) {
                return Err(format!(
                    "Patch path resolves outside the selected workspace: {relative}"
                ));
            }
            Ok(joined)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("Patch target does not exist: {relative}"))
        }
        Err(error) => Err(format!("Could not inspect patch path {relative}: {error}")),
    }
}

fn validate_patch_path_text(path: &str) -> Result<(), String> {
    if path.trim().is_empty() || path.chars().any(char::is_control) {
        return Err("Patch paths cannot be empty or contain control characters".to_string());
    }
    Ok(())
}

fn read_optional_file(target: &Path, relative: &str) -> Result<Option<Vec<u8>>, String> {
    match fs::read(target) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Could not read {relative}: {error}")),
    }
}

fn ensure_hashes_still_match(root: &Path, files: &[PatchFilePreview]) -> Result<(), String> {
    for file in files {
        let allow_missing = file.change_type == "A";
        let target = resolve_patch_path(root, &file.path, allow_missing)?;
        let current = read_optional_file(&target, &file.path)?.map(|bytes| sha256(&bytes));
        let matches = if allow_missing {
            current.is_none()
        } else {
            file.expected_sha256
                .as_deref()
                .zip(current.as_deref())
                .is_some_and(|(expected, actual)| expected.eq_ignore_ascii_case(actual))
        };
        if !matches {
            return Err(format!("File changed since preview: {}", file.path));
        }
    }
    Ok(())
}

fn run_git_apply(root: &Path, diff: &str, check_only: bool) -> Result<(), String> {
    if diff.trim().is_empty() {
        return Err("Patch did not produce a valid diff".to_string());
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("apply")
        .arg("--whitespace=nowarn");
    if check_only {
        command.arg("--check");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not start git apply: {error}"))?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "Could not open git apply stdin".to_string())?
        .write_all(diff.as_bytes());
    let output = child
        .wait_with_output()
        .map_err(|error| format!("Could not collect git apply result: {error}"))?;
    if output.status.success() && write_result.is_ok() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let message = if stderr.is_empty() {
        "git apply failed".to_string()
    } else {
        stderr
    };
    if let Err(error) = write_result {
        return Err(format!("{message} (patch input failed: {error})"));
    }
    Err(message)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn count_text_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let newline_count = bytes.iter().filter(|byte| **byte == b'\n').count();
    if bytes.last() == Some(&b'\n') {
        newline_count
    } else {
        newline_count + 1
    }
}

fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit())
}
