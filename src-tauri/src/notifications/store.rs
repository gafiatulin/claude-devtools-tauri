use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::models::notifications::{DetectedError, NotificationsResult};

/// Maximum number of notifications to retain.
const MAX_NOTIFICATIONS: usize = 200;

/// Thread-safe in-memory notification store with JSON file persistence.
pub struct NotificationStore {
    notifications: Arc<RwLock<Vec<DetectedError>>>,
    path: PathBuf,
}

impl NotificationStore {
    /// Create a new empty store that will persist to `path`.
    pub fn new(path: PathBuf) -> Self {
        Self {
            notifications: Arc::new(RwLock::new(Vec::new())),
            path,
        }
    }

    /// Load from an existing JSON file. If the file does not exist, returns an empty store.
    pub fn load(path: &Path) -> Result<Self, String> {
        let notifications = if path.exists() {
            let data =
                fs::read_to_string(path).map_err(|e| format!("Failed to read notifications: {e}"))?;
            let loaded: Vec<DetectedError> = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse notifications JSON: {e}"))?;
            loaded
        } else {
            Vec::new()
        };

        Ok(Self {
            notifications: Arc::new(RwLock::new(notifications)),
            path: path.to_path_buf(),
        })
    }

    /// Add a notification, cap the list at `MAX_NOTIFICATIONS`, and persist.
    pub fn add(&self, notification: DetectedError) -> Result<(), String> {
        let mut list = self
            .notifications
            .write()
            .map_err(|e| format!("Write lock error: {e}"))?;

        // Insert at the front (most recent first)
        list.insert(0, notification);

        // Cap
        list.truncate(MAX_NOTIFICATIONS);

        let snapshot = list.clone();
        drop(list);
        self.persist(&snapshot)
    }

    /// Get a page of notifications with pagination metadata.
    pub fn get(&self, limit: usize, offset: usize) -> NotificationsResult {
        let list = self.notifications.read().expect("Read lock poisoned");
        let total = list.len();
        let unread_count = list.iter().filter(|n| !n.is_read).count();

        let page: Vec<DetectedError> = list.iter().skip(offset).take(limit).cloned().collect();
        let has_more = offset + page.len() < total;

        NotificationsResult {
            notifications: page,
            total: total as u32,
            total_count: total as u32,
            unread_count: unread_count as u32,
            has_more,
        }
    }

    /// Mark a single notification as read. Returns `true` if found.
    pub fn mark_read(&self, id: &str) -> Result<bool, String> {
        let mut list = self
            .notifications
            .write()
            .map_err(|e| format!("Write lock error: {e}"))?;

        let found = list.iter_mut().find(|n| n.id == id);
        if let Some(notif) = found {
            notif.is_read = true;
            let snapshot = list.clone();
            drop(list);
            self.persist(&snapshot)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Mark all notifications as read.
    pub fn mark_all_read(&self) -> Result<bool, String> {
        let mut list = self
            .notifications
            .write()
            .map_err(|e| format!("Write lock error: {e}"))?;

        for n in list.iter_mut() {
            n.is_read = true;
        }

        let snapshot = list.clone();
        drop(list);
        self.persist(&snapshot)?;
        Ok(true)
    }

    /// Delete a single notification by ID. Returns `true` if found and removed.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let mut list = self
            .notifications
            .write()
            .map_err(|e| format!("Write lock error: {e}"))?;

        let before = list.len();
        list.retain(|n| n.id != id);
        let removed = list.len() < before;

        if removed {
            let snapshot = list.clone();
            drop(list);
            self.persist(&snapshot)?;
        }

        Ok(removed)
    }

    /// Clear all notifications.
    pub fn clear(&self) -> Result<(), String> {
        let mut list = self
            .notifications
            .write()
            .map_err(|e| format!("Write lock error: {e}"))?;
        list.clear();
        drop(list);
        self.persist(&Vec::new())
    }

    /// Get the count of unread notifications.
    pub fn get_unread_count(&self) -> usize {
        self.notifications
            .read()
            .expect("Read lock poisoned")
            .iter()
            .filter(|n| !n.is_read)
            .count()
    }

    /// Persist the notification list to disk as JSON.
    fn persist(&self, notifications: &[DetectedError]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create notifications directory: {e}"))?;
        }

        let json = serde_json::to_string_pretty(notifications)
            .map_err(|e| format!("Failed to serialize notifications: {e}"))?;

        let tmp_path = self.path.with_extension("json.tmp");
        fs::write(&tmp_path, json.as_bytes())
            .map_err(|e| format!("Failed to write temp notifications file: {e}"))?;
        fs::rename(&tmp_path, &self.path)
            .map_err(|e| format!("Failed to rename temp notifications file: {e}"))?;

        Ok(())
    }

    /// Returns the default notifications file path for macOS.
    /// `~/Library/Application Support/com.github.gafiatulin.claudedevtools/notifications.json`
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.github.gafiatulin.claudedevtools")
            .join("notifications.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notifications::DetectedErrorContext;
    use std::env;

    fn make_notification(id: &str) -> DetectedError {
        DetectedError {
            id: id.to_string(),
            timestamp: 1000.0,
            session_id: "s1".to_string(),
            project_id: "p1".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            source: "ToolResult".to_string(),
            message: "test error".to_string(),
            line_number: Some(1),
            tool_use_id: None,
            subagent_id: None,
            is_read: false,
            created_at: 2000.0,
            trigger_color: None,
            trigger_id: None,
            trigger_name: None,
            context: DetectedErrorContext {
                project_name: "Test".to_string(),
                cwd: None,
            },
        }
    }

    #[test]
    fn test_add_and_get() {
        let dir = env::temp_dir().join("notif_test_add_get");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        store.add(make_notification("n1")).unwrap();
        store.add(make_notification("n2")).unwrap();

        let result = store.get(10, 0);
        assert_eq!(result.total, 2);
        assert_eq!(result.unread_count, 2);
        // Most recent first
        assert_eq!(result.notifications[0].id, "n2");
        assert_eq!(result.notifications[1].id, "n1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_mark_read() {
        let dir = env::temp_dir().join("notif_test_mark_read");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        store.add(make_notification("n1")).unwrap();

        assert_eq!(store.get_unread_count(), 1);
        store.mark_read("n1").unwrap();
        assert_eq!(store.get_unread_count(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_delete() {
        let dir = env::temp_dir().join("notif_test_delete");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        store.add(make_notification("n1")).unwrap();
        store.add(make_notification("n2")).unwrap();

        store.delete("n1").unwrap();
        let result = store.get(10, 0);
        assert_eq!(result.total, 1);
        assert_eq!(result.notifications[0].id, "n2");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clear() {
        let dir = env::temp_dir().join("notif_test_clear");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        store.add(make_notification("n1")).unwrap();
        store.clear().unwrap();

        let result = store.get(10, 0);
        assert_eq!(result.total, 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cap_at_max() {
        let dir = env::temp_dir().join("notif_test_cap");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        for i in 0..210 {
            store.add(make_notification(&format!("n{i}"))).unwrap();
        }

        let result = store.get(300, 0);
        assert_eq!(result.total, 200);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_persistence_roundtrip() {
        let dir = env::temp_dir().join("notif_test_persist");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        store.add(make_notification("n1")).unwrap();
        store.add(make_notification("n2")).unwrap();

        // Load from disk
        let store2 = NotificationStore::load(&path).unwrap();
        let result = store2.get(10, 0);
        assert_eq!(result.total, 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_pagination() {
        let dir = env::temp_dir().join("notif_test_pagination");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notifications.json");

        let store = NotificationStore::new(path.clone());
        for i in 0..5 {
            store.add(make_notification(&format!("n{i}"))).unwrap();
        }

        let page1 = store.get(2, 0);
        assert_eq!(page1.notifications.len(), 2);
        assert!(page1.has_more);

        let page2 = store.get(2, 2);
        assert_eq!(page2.notifications.len(), 2);
        assert!(page2.has_more);

        let page3 = store.get(2, 4);
        assert_eq!(page3.notifications.len(), 1);
        assert!(!page3.has_more);

        let _ = fs::remove_dir_all(&dir);
    }
}
