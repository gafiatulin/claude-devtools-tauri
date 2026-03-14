use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ShellResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Open a file or directory using macOS `open` command.
#[tauri::command]
pub fn open_path(target_path: String, project_root: Option<String>) -> Result<ShellResult, String> {
    let path = if std::path::Path::new(&target_path).is_absolute() {
        target_path.clone()
    } else if let Some(root) = &project_root {
        std::path::Path::new(root)
            .join(&target_path)
            .to_string_lossy()
            .to_string()
    } else {
        target_path.clone()
    };

    match std::process::Command::new("open").arg(&path).status() {
        Ok(status) if status.success() => Ok(ShellResult {
            success: true,
            error: None,
        }),
        Ok(status) => Ok(ShellResult {
            success: false,
            error: Some(format!("open exited with status: {status}")),
        }),
        Err(e) => Ok(ShellResult {
            success: false,
            error: Some(e.to_string()),
        }),
    }
}

/// Open a URL in the default browser using macOS `open` command.
#[tauri::command]
pub fn open_external(url: String) -> Result<ShellResult, String> {
    match std::process::Command::new("open").arg(&url).status() {
        Ok(status) if status.success() => Ok(ShellResult {
            success: true,
            error: None,
        }),
        Ok(status) => Ok(ShellResult {
            success: false,
            error: Some(format!("open exited with status: {status}")),
        }),
        Err(e) => Ok(ShellResult {
            success: false,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub fn scroll_to_line(session_id: String, line_number: u32) -> Result<(), String> {
    let _ = (session_id, line_number);
    // No-op on the backend. The frontend handles scroll-to-line
    // directly through its own state management.
    Ok(())
}

/// Background task output with running state.
#[derive(Debug, Clone, Serialize)]
pub struct BackgroundTaskResult {
    pub content: String,
    /// True if the file was modified in the last 10 seconds (task likely still running).
    pub is_running: bool,
}

/// Read output from a background task's output file.
/// Searches /private/tmp/claude-*/*/tasks/{task_id}.output
#[tauri::command]
pub fn read_background_task_output(
    task_id: String,
) -> Result<Option<BackgroundTaskResult>, String> {
    // Validate task_id to prevent path traversal
    if task_id.contains('/') || task_id.contains("..") {
        return Err("Invalid task_id".to_string());
    }

    let tmp = std::path::Path::new("/private/tmp");
    let tmp_entries = match std::fs::read_dir(tmp) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    let target = format!("{task_id}.output");

    for claude_dir in tmp_entries.flatten() {
        let name = claude_dir.file_name();
        if !name.to_string_lossy().starts_with("claude-") {
            continue;
        }
        let project_entries = match std::fs::read_dir(claude_dir.path()) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for project_dir in project_entries.flatten() {
            let output_file = project_dir.path().join("tasks").join(&target);
            if output_file.exists() {
                // Combine two signals: process has file open OR file recently modified.
                // Claude Code may open/write/close per chunk, so lsof alone isn't enough.
                let lsof_open = std::process::Command::new("/usr/sbin/lsof")
                    .arg(&output_file)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                let recently_modified = std::fs::metadata(&output_file)
                    .and_then(|m| m.modified())
                    .map(|mtime| mtime.elapsed().map_or(true, |d| d.as_secs() < 60))
                    .unwrap_or(false);

                let is_running = lsof_open || recently_modified;

                match std::fs::read_to_string(&output_file) {
                    Ok(content) => {
                        return Ok(Some(BackgroundTaskResult {
                            content,
                            is_running,
                        }))
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_app_version() {
        let version = get_app_version().unwrap();
        assert!(!version.is_empty());
        // Should look like a semver string
        assert!(version.contains('.'));
    }

    #[test]
    fn test_scroll_to_line_is_noop() {
        let result = scroll_to_line("session-id".to_string(), 42);
        assert!(result.is_ok());
    }
}
