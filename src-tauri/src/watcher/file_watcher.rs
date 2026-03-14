use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::Emitter;

use crate::models::chunks::FileChangeEvent;

use super::debouncer::{DebouncedFileEvent, Debouncer};

/// Map a `notify::EventKind` to the change type string expected by the frontend.
///
/// Returns `None` for event kinds that should be ignored (e.g. Access events).
fn event_kind_to_change_type(kind: EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Create(_) => Some("add"),
        EventKind::Modify(_) => Some("change"),
        EventKind::Remove(_) => Some("unlink"),
        // Other/Access events are not relevant
        _ => None,
    }
}

/// Parse a path under `{claude_root}/projects/` to extract project ID, session
/// ID, and whether the file is a subagent.
///
/// Expected path structures:
/// - `{claude_root}/projects/{projectId}/{sessionId}.jsonl`
/// - `{claude_root}/projects/{projectId}/{sessionId}/agent_{agentId}.jsonl`
/// - `{claude_root}/projects/{projectId}/agent_{agentId}.jsonl` (legacy)
fn parse_project_event(path: &Path, projects_dir: &Path) -> Option<(String, String, bool)> {
    let relative = path.strip_prefix(projects_dir).ok()?;
    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Need at least projectId + filename
    if components.len() < 2 {
        return None;
    }

    let project_id = components[0].to_string();
    let filename = components.last()?;

    // Only care about .jsonl files
    if !filename.ends_with(".jsonl") {
        return None;
    }

    let stem = filename.strip_suffix(".jsonl")?;
    let is_subagent = stem.starts_with("agent_");

    // Determine session ID:
    // - For subagents in subdirectory: projects/{projectId}/{sessionId}/agent_{agentId}.jsonl
    //   -> sessionId is components[1] (the subdirectory name)
    // - For subagents at root (legacy): projects/{projectId}/agent_{agentId}.jsonl
    //   -> sessionId is the agent UUID (stem without "agent_" prefix)
    // - For regular sessions: projects/{projectId}/{sessionId}.jsonl
    //   -> sessionId is the stem
    let session_id = if is_subagent && components.len() >= 3 {
        // Subagent in subdirectory: the parent dir is the session ID
        components[1].to_string()
    } else if is_subagent {
        // Legacy subagent at project root
        stem.strip_prefix("agent_").unwrap_or(stem).to_string()
    } else {
        stem.to_string()
    };

    Some((project_id, session_id, is_subagent))
}

/// Parse a path under `{claude_root}/todos/` to extract the session ID.
///
/// Expected path: `{claude_root}/todos/{sessionId}.json`
fn parse_todo_event(path: &Path, todos_dir: &Path) -> Option<String> {
    let relative = path.strip_prefix(todos_dir).ok()?;
    let filename = relative.file_name()?.to_str()?;

    if !filename.ends_with(".json") {
        return None;
    }

    let session_id = filename.strip_suffix(".json")?;
    Some(session_id.to_string())
}

/// Start watching the Claude root directory for file changes and emit Tauri
/// events to the frontend.
///
/// Watches:
/// - `{claude_root}/projects/` for session JSONL changes -> emits "file-change"
/// - `{claude_root}/todos/` for todo JSON changes -> emits "todo-change"
///
/// Returns a `WatcherHandle` that keeps the watcher alive. The watcher stops
/// when the handle is dropped.
pub fn start_watcher(
    app_handle: tauri::AppHandle,
    claude_root: &Path,
) -> Result<WatcherHandle, Box<dyn std::error::Error>> {
    let projects_dir = claude_root.join("projects");
    let todos_dir = claude_root.join("todos");

    // Create directories if they don't exist to ensure we can watch them
    std::fs::create_dir_all(&projects_dir)?;
    std::fs::create_dir_all(&todos_dir)?;

    let projects_dir_clone = projects_dir.clone();
    let todos_dir_clone = todos_dir.clone();

    // Channel for notify -> debouncer communication
    let (tx, rx) = mpsc::channel::<(PathBuf, EventKind)>();

    // Create the notify watcher with a sync channel sender
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let _ = tx.send((path, event.kind));
                }
            }
        })?;

    // Start watching both directories recursively
    watcher.watch(&projects_dir, RecursiveMode::Recursive)?;
    if todos_dir.exists() {
        watcher.watch(&todos_dir, RecursiveMode::Recursive)?;
    }

    // Set up debouncers for projects (300ms) and todos (300ms)
    let app_projects = app_handle.clone();
    let projects_dir_for_debounce = projects_dir_clone.clone();

    let projects_debouncer = Debouncer::new(
        Duration::from_millis(300),
        move |events: Vec<DebouncedFileEvent>| {
            for event in events {
                let change_type = match event_kind_to_change_type(event.kind) {
                    Some(t) => t,
                    None => continue,
                };

                if let Some((project_id, session_id, is_subagent)) =
                    parse_project_event(&event.path, &projects_dir_for_debounce)
                {
                    let payload = FileChangeEvent {
                        event_type: change_type.to_string(),
                        path: event.path.to_string_lossy().to_string(),
                        project_id: Some(project_id),
                        session_id: Some(session_id),
                        is_subagent,
                    };

                    if let Err(e) = app_projects.emit("file-change", &payload) {
                        eprintln!("[watcher] Failed to emit file-change event: {e}");
                    }
                }
            }
        },
    );

    let app_todos = app_handle.clone();
    let todos_dir_for_debounce = todos_dir_clone.clone();

    let todos_debouncer = Debouncer::new(
        Duration::from_millis(300),
        move |events: Vec<DebouncedFileEvent>| {
            for event in events {
                let change_type = match event_kind_to_change_type(event.kind) {
                    Some(t) => t,
                    None => continue,
                };

                if let Some(session_id) = parse_todo_event(&event.path, &todos_dir_for_debounce) {
                    let payload = FileChangeEvent {
                        event_type: change_type.to_string(),
                        path: event.path.to_string_lossy().to_string(),
                        project_id: None,
                        session_id: Some(session_id),
                        is_subagent: false,
                    };

                    if let Err(e) = app_todos.emit("todo-change", &payload) {
                        eprintln!("[watcher] Failed to emit todo-change event: {e}");
                    }
                }
            }
        },
    );

    // Spawn a task that routes raw events from notify to the appropriate debouncer
    let projects_dir_for_route = projects_dir_clone;
    let todos_dir_for_route = todos_dir_clone;

    let router_handle = std::thread::spawn(move || {
        while let Ok((path, kind)) = rx.recv() {
            if path.starts_with(&projects_dir_for_route) {
                projects_debouncer.add_event(path, kind);
            } else if path.starts_with(&todos_dir_for_route) {
                todos_debouncer.add_event(path, kind);
            }
        }
    });

    Ok(WatcherHandle {
        _watcher: watcher,
        _router_handle: router_handle,
    })
}

/// Handle that keeps the file watcher alive. When dropped, the watcher and
/// associated background tasks are stopped.
pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
    _router_handle: std::thread::JoinHandle<()>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_event_kind_to_change_type() {
        assert_eq!(
            event_kind_to_change_type(EventKind::Create(notify::event::CreateKind::File)),
            Some("add")
        );
        assert_eq!(
            event_kind_to_change_type(EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content
            ))),
            Some("change")
        );
        assert_eq!(
            event_kind_to_change_type(EventKind::Remove(notify::event::RemoveKind::File)),
            Some("unlink")
        );
        assert_eq!(
            event_kind_to_change_type(EventKind::Access(notify::event::AccessKind::Read)),
            None
        );
    }

    #[test]
    fn test_parse_project_event_regular_session() {
        let projects_dir = PathBuf::from("/Users/me/.claude/projects");
        let path = projects_dir.join("-Users-me-myproject/abc-def-123.jsonl");

        let result = parse_project_event(&path, &projects_dir);
        assert_eq!(
            result,
            Some((
                "-Users-me-myproject".to_string(),
                "abc-def-123".to_string(),
                false
            ))
        );
    }

    #[test]
    fn test_parse_project_event_subagent_in_subdirectory() {
        let projects_dir = PathBuf::from("/Users/me/.claude/projects");
        let path = projects_dir.join("-Users-me-myproject/abc-def-123/agent_xyz-789.jsonl");

        let result = parse_project_event(&path, &projects_dir);
        assert_eq!(
            result,
            Some((
                "-Users-me-myproject".to_string(),
                "abc-def-123".to_string(),
                true
            ))
        );
    }

    #[test]
    fn test_parse_project_event_legacy_subagent() {
        let projects_dir = PathBuf::from("/Users/me/.claude/projects");
        let path = projects_dir.join("-Users-me-myproject/agent_xyz-789.jsonl");

        let result = parse_project_event(&path, &projects_dir);
        assert_eq!(
            result,
            Some((
                "-Users-me-myproject".to_string(),
                "xyz-789".to_string(),
                true
            ))
        );
    }

    #[test]
    fn test_parse_project_event_non_jsonl() {
        let projects_dir = PathBuf::from("/Users/me/.claude/projects");
        let path = projects_dir.join("-Users-me-myproject/readme.txt");

        assert_eq!(parse_project_event(&path, &projects_dir), None);
    }

    #[test]
    fn test_parse_todo_event() {
        let todos_dir = PathBuf::from("/Users/me/.claude/todos");
        let path = todos_dir.join("abc-def-123.json");

        assert_eq!(
            parse_todo_event(&path, &todos_dir),
            Some("abc-def-123".to_string())
        );
    }

    #[test]
    fn test_parse_todo_event_non_json() {
        let todos_dir = PathBuf::from("/Users/me/.claude/todos");
        let path = todos_dir.join("abc-def-123.txt");

        assert_eq!(parse_todo_event(&path, &todos_dir), None);
    }
}
