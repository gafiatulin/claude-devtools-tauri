use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::models::notifications::{
    NotificationTrigger, TriggerTestError, TriggerTestErrorContext, TriggerTestResult,
};

use super::evaluator::{evaluate_triggers_cached, EvalContext, RegexCache};

/// Maximum number of sessions to scan during a trigger test.
const MAX_SESSIONS: usize = 100;

/// Default timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Test a trigger against historical session data.
///
/// Scans JSONL files from the given project directories, evaluates the trigger
/// against each entry, and returns aggregated results.
pub fn test_trigger(
    trigger: &NotificationTrigger,
    project_dirs: &[PathBuf],
    max_sessions: usize,
    timeout_secs: u64,
) -> TriggerTestResult {
    let max_sessions = max_sessions.min(MAX_SESSIONS);
    let timeout_secs = if timeout_secs == 0 {
        DEFAULT_TIMEOUT_SECS
    } else {
        timeout_secs
    };
    let start = Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);

    let mut errors: Vec<TriggerTestError> = Vec::new();
    let mut total_count: u32 = 0;
    let mut truncated = false;
    let mut sessions_scanned: usize = 0;

    let triggers = vec![trigger.clone()];
    let cache = RegexCache::new(&triggers);

    'outer: for project_dir in project_dirs {
        let jsonl_files = match collect_jsonl_files(project_dir) {
            Ok(files) => files,
            Err(_) => continue,
        };

        let project_id = project_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let project_name = decode_project_name(&project_id);

        for file_path in &jsonl_files {
            if sessions_scanned >= max_sessions {
                truncated = true;
                break 'outer;
            }

            if start.elapsed() > timeout {
                truncated = true;
                break 'outer;
            }

            let session_id = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Skip subagent files
            if session_id.starts_with("agent_") {
                continue;
            }

            sessions_scanned += 1;

            let entries = match crate::jsonl::reader::read_jsonl_file(file_path) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            let ctx = EvalContext {
                project_id: project_id.clone(),
                session_id: session_id.clone(),
                file_path: file_path.to_string_lossy().to_string(),
                project_name: project_name.clone(),
                repository_ids: vec![],
            };

            for (line_idx, entry) in entries.iter().enumerate() {
                if start.elapsed() > timeout {
                    truncated = true;
                    break 'outer;
                }

                let detected = evaluate_triggers_cached(
                    entry,
                    &triggers,
                    &ctx,
                    Some((line_idx + 1) as u32),
                    &cache,
                );

                for d in detected {
                    total_count += 1;

                    // Keep a reasonable number of error details (cap at 100)
                    if errors.len() < 100 {
                        errors.push(TriggerTestError {
                            id: d.id,
                            session_id: d.session_id,
                            project_id: d.project_id,
                            message: d.message,
                            timestamp: d.timestamp,
                            source: d.source,
                            tool_use_id: d.tool_use_id,
                            subagent_id: d.subagent_id,
                            line_number: d.line_number,
                            context: TriggerTestErrorContext {
                                project_name: project_name.clone(),
                            },
                        });
                    }

                    // Cap total count
                    if total_count >= 10_000 {
                        truncated = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    TriggerTestResult {
        total_count,
        errors,
        truncated: if truncated { Some(true) } else { None },
    }
}

/// Collect all .jsonl files in a project directory, sorted by modification time (newest first).
fn collect_jsonl_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read dir: {e}"))?;

    let mut files: Vec<(PathBuf, std::time::SystemTime)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|e| {
            let path = e.path();
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    // Sort newest first so we scan the most recent sessions first
    files.sort_by_key(|(_path, mtime)| std::cmp::Reverse(*mtime));

    Ok(files.into_iter().map(|(p, _)| p).collect())
}

/// Decode a project directory name to a human-readable project name.
/// Takes the last path component (e.g., "-Users-victor-myproject" -> "myproject").
fn decode_project_name(project_id: &str) -> String {
    project_id
        .rsplit('-')
        .next()
        .unwrap_or(project_id)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notifications::NotificationTrigger;

    fn make_test_trigger() -> NotificationTrigger {
        NotificationTrigger {
            id: "test".to_string(),
            name: "Test".to_string(),
            enabled: true,
            content_type: "tool_result".to_string(),
            tool_name: None,
            is_builtin: None,
            ignore_patterns: None,
            mode: "error_status".to_string(),
            require_error: Some(true),
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }
    }

    #[test]
    fn test_empty_project_dirs() {
        let result = test_trigger(&make_test_trigger(), &[], 100, 30);
        assert_eq!(result.total_count, 0);
        assert!(result.errors.is_empty());
        assert!(result.truncated.is_none());
    }

    #[test]
    fn test_nonexistent_project_dir() {
        let result = test_trigger(
            &make_test_trigger(),
            &[PathBuf::from("/nonexistent/path/12345")],
            100,
            30,
        );
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_decode_project_name() {
        assert_eq!(decode_project_name("-Users-victor-myproject"), "myproject");
        assert_eq!(decode_project_name("singlename"), "singlename");
    }

    #[test]
    fn test_max_sessions_capped() {
        // test_trigger should cap max_sessions at MAX_SESSIONS (100)
        let result = test_trigger(&make_test_trigger(), &[], 9999, 30);
        assert_eq!(result.total_count, 0);
    }
}
