use std::collections::HashMap;
use std::path::Path;

use crate::models::api::ClaudeMdFileInfo;

/// Approximate token count from character count (~4 chars per token).
fn estimate_tokens(char_count: u32) -> u32 {
    char_count.div_ceil(4)
}

/// Build a `ClaudeMdFileInfo` for a given path. If the file exists, reads it
/// and computes character count and estimated tokens. If it does not exist,
/// returns a stub with `exists: false`.
fn read_claude_md_at(path: &Path) -> ClaudeMdFileInfo {
    let path_str = path.to_string_lossy().to_string();
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let char_count = content.len() as u32;
            ClaudeMdFileInfo {
                path: path_str,
                exists: true,
                char_count,
                estimated_tokens: estimate_tokens(char_count),
            }
        }
        Err(_) => ClaudeMdFileInfo {
            path: path_str,
            exists: false,
            char_count: 0,
            estimated_tokens: 0,
        },
    }
}

/// Read all CLAUDE.md files relevant to a project.
///
/// Searches for:
/// - `{project_root}/CLAUDE.md`
/// - `{project_root}/.claude/CLAUDE.md`
/// - Parent directories up to filesystem root (CLAUDE.md in each)
///
/// Returns a map of absolute path -> ClaudeMdFileInfo.
#[tauri::command]
pub fn read_claude_md_files(
    project_root: String,
) -> Result<HashMap<String, ClaudeMdFileInfo>, String> {
    let root = Path::new(&project_root);
    let mut results = HashMap::new();

    // Check project root CLAUDE.md
    let project_claude_md = root.join("CLAUDE.md");
    let info = read_claude_md_at(&project_claude_md);
    if info.exists {
        results.insert(info.path.clone(), info);
    }

    // Check .claude/CLAUDE.md in project root
    let dot_claude_md = root.join(".claude").join("CLAUDE.md");
    let info = read_claude_md_at(&dot_claude_md);
    if info.exists {
        results.insert(info.path.clone(), info);
    }

    // Walk up parent directories looking for CLAUDE.md files
    let mut current = root.parent();
    while let Some(dir) = current {
        let claude_md = dir.join("CLAUDE.md");
        let info = read_claude_md_at(&claude_md);
        if info.exists {
            results.insert(info.path.clone(), info);
        }
        current = dir.parent();
    }

    Ok(results)
}

/// Read CLAUDE.md from a specific directory.
#[tauri::command]
pub fn read_directory_claude_md(dir_path: String) -> Result<ClaudeMdFileInfo, String> {
    let dir = Path::new(&dir_path);
    let claude_md = dir.join("CLAUDE.md");
    Ok(read_claude_md_at(&claude_md))
}

/// Read a specific file and return metadata about it.
///
/// If `max_tokens` is provided, the character count is capped at approximately
/// `max_tokens * 4` characters. Returns `None` if the file does not exist.
#[tauri::command]
pub fn read_mentioned_file(
    absolute_path: String,
    project_root: String,
    max_tokens: Option<u32>,
) -> Result<Option<ClaudeMdFileInfo>, String> {
    let _ = &project_root; // Available for future path validation

    let path = Path::new(&absolute_path);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file {absolute_path}: {e}"))?;

    let char_count = if let Some(max_tok) = max_tokens {
        let max_chars = (max_tok as usize) * 4;
        content.len().min(max_chars) as u32
    } else {
        content.len() as u32
    };

    Ok(Some(ClaudeMdFileInfo {
        path: absolute_path,
        exists: true,
        char_count,
        estimated_tokens: estimate_tokens(char_count),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(5), 2);
        assert_eq!(estimate_tokens(100), 25);
    }

    #[test]
    fn test_read_directory_claude_md_nonexistent() {
        let result = read_directory_claude_md("/nonexistent/path/xyz".to_string()).unwrap();
        assert!(!result.exists);
        assert_eq!(result.char_count, 0);
        assert_eq!(result.estimated_tokens, 0);
    }

    #[test]
    fn test_read_mentioned_file_nonexistent() {
        let result =
            read_mentioned_file("/nonexistent/file.txt".to_string(), "/".to_string(), None)
                .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_mentioned_file_with_max_tokens() {
        // /etc/hosts exists on macOS and has some content
        let result = read_mentioned_file(
            "/etc/hosts".to_string(),
            "/".to_string(),
            Some(2), // 2 tokens = ~8 chars max
        )
        .unwrap();
        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.exists);
        assert!(info.char_count <= 8);
        assert!(info.estimated_tokens <= 2);
    }

    #[test]
    fn test_read_claude_md_files_nonexistent_root() {
        let result = read_claude_md_files("/nonexistent/path/xyz".to_string()).unwrap();
        // No CLAUDE.md files should be found under a nonexistent path
        assert!(result.is_empty());
    }
}
