use tauri::{AppHandle, Emitter};

use crate::models::chunks::FileChangeEvent;
use crate::models::notifications::DetectedError;

/// Emit a file-change event to the frontend.
pub fn emit_file_change(app: &AppHandle, event: &FileChangeEvent) {
    if let Err(e) = app.emit("file-change", event) {
        eprintln!("[events] Failed to emit file-change event: {e}");
    }
}

/// Emit a todo-change event to the frontend.
pub fn emit_todo_change(app: &AppHandle, event: &FileChangeEvent) {
    if let Err(e) = app.emit("todo-change", event) {
        eprintln!("[events] Failed to emit todo-change event: {e}");
    }
}

/// Emit a new notification event to the frontend.
pub fn emit_notification_new(app: &AppHandle, error: &DetectedError) {
    if let Err(e) = app.emit("notification:new", error) {
        eprintln!("[events] Failed to emit notification:new event: {e}");
    }
}

/// Payload for notification:updated events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationUpdatedPayload {
    pub total: u32,
    #[serde(rename = "unreadCount")]
    pub unread_count: u32,
}

/// Emit a notification updated event to the frontend.
pub fn emit_notification_updated(app: &AppHandle, total: u32, unread_count: u32) {
    let payload = NotificationUpdatedPayload {
        total,
        unread_count,
    };
    if let Err(e) = app.emit("notification:updated", &payload) {
        eprintln!("[events] Failed to emit notification:updated event: {e}");
    }
}
