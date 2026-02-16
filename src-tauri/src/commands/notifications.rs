use tauri::State;

use crate::models::notifications::NotificationsResult;
use crate::AppState;

#[tauri::command]
pub fn get_notifications(
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<AppState>,
) -> Result<NotificationsResult, String> {
    let limit = limit.unwrap_or(50) as usize;
    let offset = offset.unwrap_or(0) as usize;
    Ok(state.notifications.get(limit, offset))
}

#[tauri::command]
pub fn mark_notification_read(
    id: String,
    state: State<AppState>,
) -> Result<bool, String> {
    state.notifications.mark_read(&id)
}

#[tauri::command]
pub fn mark_all_notifications_read(
    state: State<AppState>,
) -> Result<bool, String> {
    state.notifications.mark_all_read()
}

#[tauri::command]
pub fn delete_notification(
    id: String,
    state: State<AppState>,
) -> Result<bool, String> {
    state.notifications.delete(&id)
}

#[tauri::command]
pub fn clear_notifications(
    state: State<AppState>,
) -> Result<bool, String> {
    state.notifications.clear()?;
    Ok(true)
}

#[tauri::command]
pub fn get_unread_count(
    state: State<AppState>,
) -> Result<u32, String> {
    Ok(state.notifications.get_unread_count() as u32)
}
