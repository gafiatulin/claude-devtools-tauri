use tauri::State;

use crate::models::config::AppConfig;
use crate::models::notifications::{NotificationTrigger, TriggerTestResult};
use crate::AppState;

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<serde_json::Value, String> {
    state.config.get_serialized()
}

#[tauri::command]
pub fn update_config(
    section: String,
    data: serde_json::Value,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.update(&section, data)
}

#[tauri::command]
pub fn add_ignore_regex(
    pattern: String,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.add_ignore_regex(pattern)
}

#[tauri::command]
pub fn remove_ignore_regex(
    pattern: String,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.remove_ignore_regex(&pattern)
}

#[tauri::command]
pub fn add_ignore_repository(
    repository_id: String,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.add_ignore_repository(repository_id)
}

#[tauri::command]
pub fn remove_ignore_repository(
    repository_id: String,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.remove_ignore_repository(&repository_id)
}

#[tauri::command]
pub fn snooze_notifications(
    minutes: u32,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.snooze(minutes)
}

#[tauri::command]
pub fn clear_snooze(state: State<AppState>) -> Result<AppConfig, String> {
    state.config.clear_snooze()
}

#[tauri::command]
pub fn add_trigger(
    trigger: NotificationTrigger,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.add_trigger(trigger)
}

#[tauri::command]
pub fn update_trigger(
    trigger_id: String,
    updates: serde_json::Value,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.update_trigger(&trigger_id, updates)
}

#[tauri::command]
pub fn remove_trigger(
    trigger_id: String,
    state: State<AppState>,
) -> Result<AppConfig, String> {
    state.config.remove_trigger(&trigger_id)
}

#[tauri::command]
pub fn get_triggers(state: State<AppState>) -> Result<Vec<NotificationTrigger>, String> {
    Ok(state.config.get_triggers())
}

#[tauri::command]
pub fn test_trigger(
    trigger: NotificationTrigger,
    state: State<AppState>,
) -> Result<TriggerTestResult, String> {
    let claude_root = state.claude_root();
    let projects_dir = claude_root.join("projects");

    // Collect all project directories
    let project_dirs: Vec<std::path::PathBuf> = if projects_dir.exists() {
        std::fs::read_dir(&projects_dir)
            .map_err(|e| format!("Failed to read projects dir: {e}"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect()
    } else {
        Vec::new()
    };

    Ok(crate::notifications::trigger_tester::test_trigger(
        &trigger,
        &project_dirs,
        100,
        30,
    ))
}

#[tauri::command]
pub fn pin_session(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    state.config.pin_session(&project_id, &session_id)?;
    Ok(())
}

#[tauri::command]
pub fn unpin_session(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    state.config.unpin_session(&project_id, &session_id)?;
    Ok(())
}

#[tauri::command]
pub fn hide_session(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    state.config.hide_session(&project_id, &session_id)?;
    Ok(())
}

#[tauri::command]
pub fn unhide_session(
    project_id: String,
    session_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    state.config.unhide_session(&project_id, &session_id)?;
    Ok(())
}

#[tauri::command]
pub fn hide_sessions(
    project_id: String,
    session_ids: Vec<String>,
    state: State<AppState>,
) -> Result<(), String> {
    state.config.hide_sessions(&project_id, &session_ids)?;
    Ok(())
}

#[tauri::command]
pub fn unhide_sessions(
    project_id: String,
    session_ids: Vec<String>,
    state: State<AppState>,
) -> Result<(), String> {
    state.config.unhide_sessions(&project_id, &session_ids)?;
    Ok(())
}
