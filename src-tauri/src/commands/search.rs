use tauri::State;

use crate::models::domain::SearchSessionsResult;
use crate::AppState;

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn search_sessions(
    project_id: String,
    query: String,
    max_results: Option<u32>,
    state: State<AppState>,
) -> Result<SearchSessionsResult, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    let max_results = max_results.unwrap_or(50) as usize;
    Ok(crate::search::engine::search_sessions(
        &project_dir,
        &project_id,
        &query,
        max_results,
    ))
}
