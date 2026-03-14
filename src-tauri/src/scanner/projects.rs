use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use rayon::prelude::*;

use crate::models::domain::Project;

/// Global cache for decoded project IDs to avoid repeated filesystem stat() calls.
/// Key: encoded project ID string, Value: decoded filesystem path.
static DECODED_PATH_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn decoded_path_cache() -> &'static Mutex<HashMap<String, String>> {
    DECODED_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Decode a project directory name back to its original filesystem path.
///
/// The encoding scheme replaces `/` with `-`:
///   /Users/username/project-name -> -Users-username-project-name
///
/// Since directory names can contain hyphens, naive replacement is ambiguous.
/// We use a greedy filesystem-probing approach: try to match the longest
/// existing path component at each position, falling back to treating each
/// `-` as a path separator.
pub fn decode_project_id(encoded: &str) -> String {
    // Check cache first
    if let Ok(cache) = decoded_path_cache().lock() {
        if let Some(cached) = cache.get(encoded) {
            return cached.clone();
        }
    }

    // Strip leading `-` which represents the root `/`
    let rest = encoded.strip_prefix('-').unwrap_or(encoded);
    if rest.is_empty() {
        return "/".to_string();
    }

    let parts: Vec<&str> = rest.split('-').collect();
    let result = decode_path_greedy(&parts, 0, std::path::Path::new("/")).unwrap_or_else(|| {
        // Fallback: simple replacement
        encoded.replace('-', "/")
    });

    // Cache the result, but only if the resolved path exists on disk
    let resolved_path = Path::new(&result);
    if resolved_path.exists() {
        if let Ok(mut cache) = decoded_path_cache().lock() {
            cache.insert(encoded.to_string(), result.clone());
        }
    }

    result
}

/// Greedily reconstruct a filesystem path from hyphen-separated parts.
///
/// At each step, try joining the maximum number of remaining parts with `-`
/// to see if that directory exists, then recurse on the remaining parts.
fn decode_path_greedy(
    parts: &[&str],
    start: usize,
    current_path: &std::path::Path,
) -> Option<String> {
    if start >= parts.len() {
        return Some(current_path.to_string_lossy().to_string());
    }

    // Try longest possible segment first (greedy)
    for end in (start + 1..=parts.len()).rev() {
        let segment = parts[start..end].join("-");
        let candidate = current_path.join(&segment);

        if end == parts.len() {
            // Last segment: doesn't need to be a directory, just check existence
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        } else if candidate.is_dir() {
            // Not the last segment: must be a directory
            if let Some(result) = decode_path_greedy(parts, end, &candidate) {
                return result.into();
            }
        }
    }

    None
}

/// Extract a display name from a decoded project path (last path segment).
fn project_name_from_path(path: &str) -> String {
    path.rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// Get the creation time of a directory as a Unix timestamp in milliseconds.
///
/// On macOS, uses the file's birth time (creation time). Falls back to
/// modification time if birth time is unavailable.
fn get_creation_time_ms(path: &Path) -> Option<f64> {
    let metadata = fs::metadata(path).ok()?;

    // Try birthtime first (macOS supports this via created())
    let time = metadata.created().or_else(|_| metadata.modified()).ok()?;

    let duration = time.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_secs_f64() * 1000.0)
}

/// Get the modification time of a file as a Unix timestamp in milliseconds.
fn get_modified_time_ms(path: &Path) -> Option<f64> {
    let metadata = fs::metadata(path).ok()?;
    let time = metadata.modified().ok()?;
    let duration = time.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_secs_f64() * 1000.0)
}

/// Scan the `~/.claude/projects/` directory and return a list of discovered projects.
///
/// Each subdirectory under `projects/` represents one project. Session files
/// are `{uuid}.jsonl` files directly inside the project directory.
pub fn scan_projects(claude_root: &Path) -> Result<Vec<Project>, String> {
    let projects_dir = claude_root.join("projects");
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {e}"))?;

    // Collect project directory paths first
    let project_dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let dir_name = path.file_name()?.to_str()?.to_string();
            if !dir_name.starts_with('-') {
                return None;
            }
            Some((path, dir_name))
        })
        .collect();

    // Scan projects in parallel
    let pool = crate::scanner::scan_pool();
    let mut projects: Vec<Project> = pool.install(|| {
        project_dirs
            .par_iter()
            .map(|(path, dir_name)| {
                let decoded_path = decode_project_id(dir_name);
                let name = project_name_from_path(&decoded_path);
                let created_at = get_creation_time_ms(path).unwrap_or(0.0);
                let (session_ids, most_recent_session) = scan_session_files(path);

                Project {
                    id: dir_name.clone(),
                    path: decoded_path,
                    name,
                    sessions: session_ids,
                    created_at,
                    most_recent_session,
                }
            })
            .collect()
    });

    // Sort by most recent session (descending), then by created_at (descending)
    projects.sort_by(|a, b| {
        let a_recent = a.most_recent_session.unwrap_or(a.created_at);
        let b_recent = b.most_recent_session.unwrap_or(b.created_at);
        b_recent
            .partial_cmp(&a_recent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(projects)
}

/// Scan a project directory for session JSONL files.
///
/// Returns a tuple of (session_ids, most_recent_modified_time).
/// Excludes `agent_*.jsonl` files at root level (legacy subagent files)
/// and files inside subdirectories.
fn scan_session_files(project_dir: &Path) -> (Vec<String>, Option<f64>) {
    let entries = match fs::read_dir(project_dir) {
        Ok(e) => e,
        Err(_) => return (Vec::new(), None),
    };

    let mut session_ids = Vec::new();
    let mut most_recent: Option<f64> = None;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        // Only consider files (not directories)
        if !path.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Must be a .jsonl file
        if !file_name.ends_with(".jsonl") {
            continue;
        }

        // Skip legacy subagent files at root level (agent_*.jsonl)
        if file_name.starts_with("agent_") || file_name.starts_with("agent-") {
            continue;
        }

        // Extract session ID (filename without .jsonl extension)
        let session_id = file_name.trim_end_matches(".jsonl").to_string();
        session_ids.push(session_id);

        // Track most recent modification time
        if let Some(mtime) = get_modified_time_ms(&path) {
            most_recent = Some(match most_recent {
                Some(current) => current.max(mtime),
                None => mtime,
            });
        }
    }

    (session_ids, most_recent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_decode_project_id_with_existing_paths() {
        // This test uses real filesystem paths - it will only pass if these directories exist.
        // On CI or other machines, the greedy decoder falls back to simple replacement.
        let decoded = decode_project_id("-Users-victor-Workspace-personal-claude-devtools-tauri");

        // If the path exists on the filesystem, greedy decoding should resolve it.
        // Otherwise, it falls back to naive replacement.
        let candidate =
            std::path::Path::new("/Users/victor/Workspace/personal/claude-devtools-tauri");
        if candidate.exists() {
            assert_eq!(
                decoded,
                "/Users/victor/Workspace/personal/claude-devtools-tauri"
            );
        }
    }

    #[test]
    fn test_decode_project_id_fallback() {
        // Non-existent path falls back to simple replacement
        let decoded = decode_project_id("-nonexistent-path-here");
        assert_eq!(decoded, "/nonexistent/path/here");
    }

    #[test]
    fn test_decode_project_id_root_only() {
        let decoded = decode_project_id("-");
        assert_eq!(decoded, "/");
    }

    #[test]
    fn test_decode_project_id_empty() {
        let decoded = decode_project_id("");
        // Empty string: strip_prefix('-') fails, rest = "", is_empty() -> returns "/"
        assert_eq!(decoded, "/");
    }

    #[test]
    fn test_decode_project_id_single_segment() {
        let decoded = decode_project_id("-tmp");
        // /tmp exists on macOS/Linux
        if std::path::Path::new("/tmp").exists() {
            assert_eq!(decoded, "/tmp");
        }
    }

    #[test]
    fn test_project_name_from_path() {
        assert_eq!(
            project_name_from_path("/Users/victor/Workspace/personal/claude-devtools"),
            "claude-devtools"
        );
        assert_eq!(project_name_from_path("/Users/victor/project"), "project");
        assert_eq!(project_name_from_path("/single"), "single");
    }

    #[test]
    fn test_project_name_from_path_trailing_slash() {
        assert_eq!(project_name_from_path("/foo/bar/"), "bar");
    }

    #[test]
    fn test_scan_projects_empty_root() {
        let tmp = TempDir::new().unwrap();
        // No projects/ directory at all
        let result = scan_projects(tmp.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_scan_projects_empty_projects_dir() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("projects")).unwrap();
        let result = scan_projects(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_projects_with_sessions() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects");
        fs::create_dir(&projects_dir).unwrap();

        // Create a project directory with encoded name
        let proj_dir = projects_dir.join("-tmp-myproject");
        fs::create_dir(&proj_dir).unwrap();

        // Create session files
        fs::write(proj_dir.join("session-aaa.jsonl"), "{}\n").unwrap();
        fs::write(proj_dir.join("session-bbb.jsonl"), "{}\n").unwrap();

        // Create a subagent file that should be skipped
        fs::write(proj_dir.join("agent_xyz.jsonl"), "{}\n").unwrap();

        // Create a non-jsonl file that should be skipped
        fs::write(proj_dir.join("readme.txt"), "hi").unwrap();

        let result = scan_projects(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "-tmp-myproject");
        assert_eq!(result[0].sessions.len(), 2);
        assert!(result[0].sessions.contains(&"session-aaa".to_string()));
        assert!(result[0].sessions.contains(&"session-bbb".to_string()));
    }

    #[test]
    fn test_scan_projects_skips_non_project_dirs() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects");
        fs::create_dir(&projects_dir).unwrap();

        // Directories without leading '-' should be skipped
        fs::create_dir(projects_dir.join("not-a-project")).unwrap();
        fs::create_dir(projects_dir.join(".hidden")).unwrap();

        // Regular file at root should be ignored
        fs::write(projects_dir.join("stray-file.txt"), "hi").unwrap();

        let result = scan_projects(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_session_files_excludes_agent_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main-session.jsonl"), "{}").unwrap();
        fs::write(tmp.path().join("agent_sub1.jsonl"), "{}").unwrap();
        fs::write(tmp.path().join("agent-sub2.jsonl"), "{}").unwrap();

        let (ids, _) = scan_session_files(tmp.path());
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], "main-session");
    }
}
