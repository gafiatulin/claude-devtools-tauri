use tauri::State;

use crate::models::chunks::{ConversationGroup, SessionDetail, SubagentDetail, SubagentMetrics};
use crate::models::domain::{
    PaginatedSessionsResult, Session, SessionMetrics, SessionsByIdsOptions,
    SessionsPaginationOptions,
};
use crate::models::visualization::WaterfallData;
use crate::AppState;

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_sessions(
    project_id: String,
    state: State<AppState>,
) -> Result<Vec<Session>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    crate::scanner::sessions::scan_sessions_with_cache(
        &project_dir,
        &project_id,
        Some(&state.session_cache),
    )
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_sessions_paginated(
    project_id: String,
    cursor: Option<String>,
    limit: Option<u32>,
    options: Option<SessionsPaginationOptions>,
    state: State<AppState>,
) -> Result<PaginatedSessionsResult, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    let limit = limit.unwrap_or(20) as usize;
    let options = options.unwrap_or_else(|| SessionsPaginationOptions {
        include_total_count: Some(true),
        prefilter_all: Some(true),
        metadata_level: None,
    });
    crate::scanner::sessions::scan_sessions_paginated_with_cache(
        &project_dir,
        &project_id,
        cursor.as_deref(),
        limit,
        Some(&options),
        Some(&state.session_cache),
    )
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_sessions_by_ids(
    project_id: String,
    session_ids: Vec<String>,
    options: Option<SessionsByIdsOptions>,
    state: State<AppState>,
) -> Result<Vec<Session>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    let options = options.unwrap_or(SessionsByIdsOptions {
        metadata_level: None,
    });
    crate::scanner::sessions::get_sessions_by_ids(
        &project_dir,
        &project_id,
        &session_ids,
        Some(&options),
        Some(&state.session_cache),
    )
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_session_detail(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<Option<SessionDetail>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    match crate::scanner::sessions::scan_session_detail(&project_dir, &project_id, &session_id) {
        Ok(detail) => Ok(Some(detail)),
        Err(_) => Ok(None),
    }
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_session_metrics(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<Option<SessionMetrics>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    match crate::scanner::sessions::get_session_metrics(
        &project_dir,
        &session_id,
        Some(&state.metrics_cache),
    ) {
        Ok(metrics) => Ok(Some(metrics)),
        Err(_) => Ok(None),
    }
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_session_groups(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<Vec<ConversationGroup>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    let detail = crate::scanner::sessions::scan_session_detail(&project_dir, &project_id, &session_id)?;
    Ok(crate::parser::conversation_grouper::build_conversation_groups(
        &detail.messages,
        &detail.processes,
    ))
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_waterfall_data(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<Option<WaterfallData>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);
    match crate::scanner::sessions::scan_session_detail(&project_dir, &project_id, &session_id) {
        Ok(detail) => Ok(Some(crate::parser::waterfall_builder::build_waterfall(&detail))),
        Err(_) => Ok(None),
    }
}

#[tracing::instrument(skip(state))]
#[tauri::command]
pub fn get_subagent_detail(
    project_id: String,
    session_id: String,
    subagent_id: String,
    state: State<AppState>,
) -> Result<Option<SubagentDetail>, String> {
    let claude_root = state.claude_root();
    let project_dir = claude_root.join("projects").join(&project_id);

    // Scan subagents for this session
    let processes = crate::scanner::subagents::scan_subagents(&project_dir, &session_id)?;

    // Find the specific subagent
    let process = match processes.into_iter().find(|p| p.id == subagent_id) {
        Some(p) => p,
        None => return Ok(None),
    };

    // Build chunks from the subagent's messages
    let chunks = crate::parser::chunk_builder::build_chunks(&process.messages, &[]);
    let enhanced_chunks = crate::parser::chunk_builder::enhance_chunks(chunks, &process.messages);

    Ok(Some(SubagentDetail {
        id: process.id,
        description: process.description.unwrap_or_default(),
        chunks: enhanced_chunks,
        semantic_step_groups: None,
        start_time: process.start_time,
        end_time: process.end_time,
        duration: process.duration_ms,
        metrics: SubagentMetrics {
            input_tokens: process.metrics.input_tokens,
            output_tokens: process.metrics.output_tokens,
            thinking_tokens: 0,
            message_count: process.metrics.message_count,
        },
    }))
}
