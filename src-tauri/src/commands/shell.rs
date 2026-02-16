use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ShellResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Open a file or directory using macOS `open` command.
#[tauri::command]
pub fn open_path(
    target_path: String,
    project_root: Option<String>,
) -> Result<ShellResult, String> {
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
pub fn scroll_to_line(
    session_id: String,
    line_number: u32,
) -> Result<(), String> {
    let _ = (session_id, line_number);
    // No-op on the backend. The frontend handles scroll-to-line
    // directly through its own state management.
    Ok(())
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
