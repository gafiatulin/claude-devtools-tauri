use std::time::{Duration, Instant};

use tauri::State;

use crate::models::domain::{Project, RepositoryGroup, Session};
use crate::AppState;

/// TTL for the projects cache. Within this window, repeated scan_projects
/// calls return the cached result instead of re-scanning the filesystem.
const PROJECTS_CACHE_TTL: Duration = Duration::from_secs(2);

/// Scan projects with a short-lived cache to avoid redundant scans when
/// multiple commands (get_projects, get_repository_groups) are called in
/// quick succession.
fn scan_projects_cached(state: &AppState) -> Result<Vec<Project>, String> {
    if let Ok(guard) = state.projects_cache.lock() {
        if let Some((cached_at, ref projects)) = *guard {
            if cached_at.elapsed() < PROJECTS_CACHE_TTL {
                return Ok(projects.clone());
            }
        }
    }

    let claude_root = state.claude_root();
    let projects = crate::scanner::projects::scan_projects(&claude_root)?;

    if let Ok(mut guard) = state.projects_cache.lock() {
        *guard = Some((Instant::now(), projects.clone()));
    }

    Ok(projects)
}

#[tauri::command]
pub fn get_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    scan_projects_cached(&state)
}

#[tauri::command]
pub fn get_repository_groups(state: State<AppState>) -> Result<Vec<RepositoryGroup>, String> {
    let projects = scan_projects_cached(&state)?;
    Ok(crate::git::detector::build_repository_groups(&projects))
}

#[tauri::command]
pub fn get_worktree_sessions(
    worktree_id: String,
    state: State<AppState>,
) -> Result<Vec<Session>, String> {
    let claude_root = state.claude_root();
    // worktree_id is a project ID (same as the worktree path encoded)
    let project_dir = claude_root.join("projects").join(&worktree_id);
    crate::scanner::sessions::scan_sessions(&project_dir, &worktree_id)
}
