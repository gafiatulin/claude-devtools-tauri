use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::models::config::{AppConfig, HiddenSession, PinnedSession};
use crate::models::notifications::NotificationTrigger;

use super::defaults::default_config;

/// Thread-safe configuration manager.
///
/// Holds the in-memory config protected by an `RwLock` and the filesystem path
/// for persistence. All mutation methods acquire a write lock, update the
/// in-memory copy, persist to disk via atomic write, and return the new config.
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    path: PathBuf,
}

impl ConfigManager {
    /// Create a new `ConfigManager` with default config and the given file path.
    pub fn new(path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(default_config())),
            path,
        }
    }

    /// Load config from disk, merging with defaults for any missing fields.
    /// If the file does not exist, returns a manager with defaults.
    pub fn load(path: &Path) -> Result<Self, String> {
        let config = if path.exists() {
            let data = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read config file: {e}"))?;
            let mut loaded: AppConfig = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse config JSON: {e}"))?;

            // Merge defaults: ensure built-in triggers exist
            merge_builtin_triggers(&mut loaded);

            loaded
        } else {
            default_config()
        };

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            path: path.to_path_buf(),
        })
    }

    /// Atomic write: write to a temp file in the same directory, then rename.
    pub fn save(&self) -> Result<(), String> {
        let config = self.config.read().map_err(|e| format!("Read lock error: {e}"))?;
        let json = serde_json::to_string_pretty(&*config)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {e}"))?;
        }

        let tmp_path = self.path.with_extension("json.tmp");
        fs::write(&tmp_path, json.as_bytes())
            .map_err(|e| format!("Failed to write temp config file: {e}"))?;
        fs::rename(&tmp_path, &self.path)
            .map_err(|e| format!("Failed to rename temp config file: {e}"))?;

        Ok(())
    }

    /// Return a clone of the current config.
    pub fn get(&self) -> AppConfig {
        self.config
            .read()
            .expect("Config read lock poisoned")
            .clone()
    }

    /// Serialize the current config directly from the read lock, avoiding
    /// a full AppConfig clone. Returns a serde_json::Value ready for Tauri.
    pub fn get_serialized(&self) -> Result<serde_json::Value, String> {
        let guard = self.config.read().map_err(|e| format!("Read lock error: {e}"))?;
        serde_json::to_value(&*guard).map_err(|e| format!("Serialization error: {e}"))
    }

    /// Update a top-level section of the config by merging JSON data.
    pub fn update(&self, section: &str, data: serde_json::Value) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;

        // Helper: merge patch fields into a serialized section, then deserialize back.
        // This allows callers to send partial updates (e.g. just { "theme": "system" })
        // without needing to supply every field of the section.
        fn merge_patch<T: serde::Serialize + serde::de::DeserializeOwned>(
            current: &T,
            patch: serde_json::Value,
            label: &str,
        ) -> Result<T, String> {
            let mut base = serde_json::to_value(current)
                .map_err(|e| format!("Failed to serialize {label}: {e}"))?;
            if let (Some(base_obj), Some(patch_obj)) = (base.as_object_mut(), patch.as_object()) {
                for (key, value) in patch_obj {
                    base_obj.insert(key.clone(), value.clone());
                }
            }
            serde_json::from_value(base).map_err(|e| format!("Invalid {label} config: {e}"))
        }

        match section {
            "notifications" => {
                config.notifications =
                    merge_patch(&config.notifications, data, "notifications")?;
            }
            "general" => {
                config.general = merge_patch(&config.general, data, "general")?;
            }
            "display" => {
                config.display = merge_patch(&config.display, data, "display")?;
            }
            "sessions" => {
                config.sessions = merge_patch(&config.sessions, data, "sessions")?;
            }
            _ => return Err(format!("Unknown config section: {section}")),
        }

        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    // =========================================================================
    // Ignore regex
    // =========================================================================

    pub fn add_ignore_regex(&self, pattern: String) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        if !config.notifications.ignored_regex.contains(&pattern) {
            config.notifications.ignored_regex.push(pattern);
        }
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn remove_ignore_regex(&self, pattern: &str) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        config.notifications.ignored_regex.retain(|p| p != pattern);
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    // =========================================================================
    // Ignore repository
    // =========================================================================

    pub fn add_ignore_repository(&self, repository_id: String) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        if !config.notifications.ignored_repositories.contains(&repository_id) {
            config.notifications.ignored_repositories.push(repository_id);
        }
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn remove_ignore_repository(&self, repository_id: &str) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        config
            .notifications
            .ignored_repositories
            .retain(|id| id != repository_id);
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    // =========================================================================
    // Snooze
    // =========================================================================

    pub fn snooze(&self, minutes: u32) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        let now_ms = chrono::Utc::now().timestamp_millis() as f64;
        let snooze_until = now_ms + (minutes as f64 * 60.0 * 1000.0);
        config.notifications.snoozed_until = Some(snooze_until);
        config.notifications.snooze_minutes = minutes;
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn clear_snooze(&self) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        config.notifications.snoozed_until = None;
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    // =========================================================================
    // Triggers
    // =========================================================================

    pub fn add_trigger(&self, trigger: NotificationTrigger) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        config.notifications.triggers.push(trigger);
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn update_trigger(
        &self,
        id: &str,
        updates: serde_json::Value,
    ) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;

        let trigger = config
            .notifications
            .triggers
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Trigger not found: {id}"))?;

        // Serialize current trigger to JSON, merge updates, deserialize back
        let mut trigger_json = serde_json::to_value(&*trigger)
            .map_err(|e| format!("Failed to serialize trigger: {e}"))?;

        if let (Some(base), Some(patch)) = (trigger_json.as_object_mut(), updates.as_object()) {
            for (key, value) in patch {
                base.insert(key.clone(), value.clone());
            }
        }

        let updated: NotificationTrigger = serde_json::from_value(trigger_json)
            .map_err(|e| format!("Failed to deserialize updated trigger: {e}"))?;
        *trigger = updated;

        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn remove_trigger(&self, id: &str) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        config.notifications.triggers.retain(|t| t.id != id);
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn get_triggers(&self) -> Vec<NotificationTrigger> {
        self.config
            .read()
            .expect("Config read lock poisoned")
            .notifications
            .triggers
            .clone()
    }

    // =========================================================================
    // Pin / Unpin sessions
    // =========================================================================

    pub fn pin_session(&self, project_id: &str, session_id: &str) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        let pinned = config
            .sessions
            .pinned_sessions
            .entry(project_id.to_string())
            .or_default();

        if !pinned.iter().any(|p| p.session_id == session_id) {
            pinned.push(PinnedSession {
                session_id: session_id.to_string(),
                pinned_at: chrono::Utc::now().timestamp_millis() as f64,
            });
        }

        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn unpin_session(&self, project_id: &str, session_id: &str) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        if let Some(pinned) = config.sessions.pinned_sessions.get_mut(project_id) {
            pinned.retain(|p| p.session_id != session_id);
        }
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    // =========================================================================
    // Hide / Unhide sessions
    // =========================================================================

    pub fn hide_session(&self, project_id: &str, session_id: &str) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        let hidden = config
            .sessions
            .hidden_sessions
            .entry(project_id.to_string())
            .or_default();

        if !hidden.iter().any(|h| h.session_id == session_id) {
            hidden.push(HiddenSession {
                session_id: session_id.to_string(),
                hidden_at: chrono::Utc::now().timestamp_millis() as f64,
            });
        }

        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn unhide_session(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        if let Some(hidden) = config.sessions.hidden_sessions.get_mut(project_id) {
            hidden.retain(|h| h.session_id != session_id);
        }
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn hide_sessions(
        &self,
        project_id: &str,
        session_ids: &[String],
    ) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        let hidden = config
            .sessions
            .hidden_sessions
            .entry(project_id.to_string())
            .or_default();

        let now = chrono::Utc::now().timestamp_millis() as f64;
        for sid in session_ids {
            if !hidden.iter().any(|h| h.session_id == *sid) {
                hidden.push(HiddenSession {
                    session_id: sid.clone(),
                    hidden_at: now,
                });
            }
        }

        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    pub fn unhide_sessions(
        &self,
        project_id: &str,
        session_ids: &[String],
    ) -> Result<AppConfig, String> {
        let mut config = self.config.write().map_err(|e| format!("Write lock error: {e}"))?;
        if let Some(hidden) = config.sessions.hidden_sessions.get_mut(project_id) {
            hidden.retain(|h| !session_ids.contains(&h.session_id));
        }
        let snapshot = config.clone();
        drop(config);
        self.save()?;
        Ok(snapshot)
    }

    /// Returns the default config file path for macOS.
    /// `~/Library/Application Support/com.claudedevtools.app/config.json`
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.claudedevtools.app")
            .join("config.json")
    }
}

/// Ensure built-in triggers are present in the loaded config.
/// If a built-in trigger ID is missing, insert the default version.
fn merge_builtin_triggers(config: &mut AppConfig) {
    let defaults = super::defaults::default_triggers();
    for default_trigger in defaults {
        let exists = config
            .notifications
            .triggers
            .iter()
            .any(|t| t.id == default_trigger.id);
        if !exists {
            config.notifications.triggers.push(default_trigger);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_temp_manager() -> (TempDir, ConfigManager) {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let manager = ConfigManager::new(config_path);
        (tmp, manager)
    }

    #[test]
    fn test_new_has_defaults() {
        let (_tmp, manager) = make_temp_manager();
        let config = manager.get();
        assert!(config.notifications.enabled);
        assert_eq!(config.general.theme, "dark");
        assert!(!config.notifications.triggers.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");

        // Create and save
        let manager = ConfigManager::new(config_path.clone());
        manager.save().unwrap();
        assert!(config_path.exists());

        // Load back
        let loaded = ConfigManager::load(&config_path).unwrap();
        let config = loaded.get();
        assert!(config.notifications.enabled);
        assert_eq!(config.general.theme, "dark");
    }

    #[test]
    fn test_load_nonexistent_file_uses_defaults() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("nonexistent.json");
        let manager = ConfigManager::load(&config_path).unwrap();
        let config = manager.get();
        assert!(config.notifications.enabled);
    }

    #[test]
    fn test_add_and_remove_trigger() {
        let (_tmp, manager) = make_temp_manager();
        let initial_count = manager.get_triggers().len();

        let trigger = NotificationTrigger {
            id: "test-trigger-1".to_string(),
            name: "Test Trigger".to_string(),
            enabled: true,
            content_type: "text".to_string(),
            tool_name: None,
            is_builtin: None,
            ignore_patterns: None,
            mode: "content_match".to_string(),
            require_error: None,
            match_field: None,
            match_pattern: Some("error".to_string()),
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        };

        let config = manager.add_trigger(trigger).unwrap();
        assert_eq!(config.notifications.triggers.len(), initial_count + 1);
        assert!(config.notifications.triggers.iter().any(|t| t.id == "test-trigger-1"));

        let config = manager.remove_trigger("test-trigger-1").unwrap();
        assert_eq!(config.notifications.triggers.len(), initial_count);
        assert!(!config.notifications.triggers.iter().any(|t| t.id == "test-trigger-1"));
    }

    #[test]
    fn test_remove_nonexistent_trigger_is_noop() {
        let (_tmp, manager) = make_temp_manager();
        let before = manager.get_triggers().len();
        let config = manager.remove_trigger("does-not-exist").unwrap();
        assert_eq!(config.notifications.triggers.len(), before);
    }

    #[test]
    fn test_snooze_and_clear() {
        let (_tmp, manager) = make_temp_manager();

        // Initially no snooze
        let config = manager.get();
        assert!(config.notifications.snoozed_until.is_none());

        // Snooze for 30 minutes
        let config = manager.snooze(30).unwrap();
        assert!(config.notifications.snoozed_until.is_some());
        assert_eq!(config.notifications.snooze_minutes, 30);

        let snooze_until = config.notifications.snoozed_until.unwrap();
        let now_ms = chrono::Utc::now().timestamp_millis() as f64;
        // Should be roughly 30 minutes in the future (within 5 seconds tolerance)
        let expected = now_ms + 30.0 * 60.0 * 1000.0;
        assert!((snooze_until - expected).abs() < 5000.0);

        // Clear snooze
        let config = manager.clear_snooze().unwrap();
        assert!(config.notifications.snoozed_until.is_none());
    }

    #[test]
    fn test_add_and_remove_ignore_regex() {
        let (_tmp, manager) = make_temp_manager();

        let config = manager.add_ignore_regex("test-pattern.*".to_string()).unwrap();
        assert!(config.notifications.ignored_regex.contains(&"test-pattern.*".to_string()));

        // Adding duplicate should be idempotent
        let config = manager.add_ignore_regex("test-pattern.*".to_string()).unwrap();
        let count = config.notifications.ignored_regex.iter()
            .filter(|p| *p == "test-pattern.*")
            .count();
        assert_eq!(count, 1);

        let config = manager.remove_ignore_regex("test-pattern.*").unwrap();
        assert!(!config.notifications.ignored_regex.contains(&"test-pattern.*".to_string()));
    }

    #[test]
    fn test_pin_and_unpin_session() {
        let (_tmp, manager) = make_temp_manager();

        let config = manager.pin_session("proj1", "sess1").unwrap();
        let pinned = config.sessions.pinned_sessions.get("proj1").unwrap();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].session_id, "sess1");

        // Pinning same session again should be idempotent
        let config = manager.pin_session("proj1", "sess1").unwrap();
        let pinned = config.sessions.pinned_sessions.get("proj1").unwrap();
        assert_eq!(pinned.len(), 1);

        let config = manager.unpin_session("proj1", "sess1").unwrap();
        let pinned = config.sessions.pinned_sessions.get("proj1").unwrap();
        assert!(pinned.is_empty());
    }

    #[test]
    fn test_hide_and_unhide_sessions() {
        let (_tmp, manager) = make_temp_manager();

        let config = manager.hide_sessions("proj1", &[
            "sess1".to_string(),
            "sess2".to_string(),
        ]).unwrap();
        let hidden = config.sessions.hidden_sessions.get("proj1").unwrap();
        assert_eq!(hidden.len(), 2);

        let config = manager.unhide_sessions("proj1", &["sess1".to_string()]).unwrap();
        let hidden = config.sessions.hidden_sessions.get("proj1").unwrap();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].session_id, "sess2");
    }

    #[test]
    fn test_update_section() {
        let (_tmp, manager) = make_temp_manager();

        let data = serde_json::json!({
            "theme": "light",
            "defaultTab": "sessions",
            "claudeRootPath": null
        });
        let config = manager.update("general", data).unwrap();
        assert_eq!(config.general.theme, "light");
        assert_eq!(config.general.default_tab, "sessions");
    }

    #[test]
    fn test_update_unknown_section_errors() {
        let (_tmp, manager) = make_temp_manager();
        let result = manager.update("bogus", serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown config section"));
    }

    #[test]
    fn test_persistence_survives_reload() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");

        let manager = ConfigManager::new(config_path.clone());
        manager.pin_session("proj1", "sess1").unwrap();
        manager.snooze(15).unwrap();

        // Reload from disk
        let reloaded = ConfigManager::load(&config_path).unwrap();
        let config = reloaded.get();
        assert!(config.notifications.snoozed_until.is_some());
        assert_eq!(config.notifications.snooze_minutes, 15);
        let pinned = config.sessions.pinned_sessions.get("proj1").unwrap();
        assert_eq!(pinned[0].session_id, "sess1");
    }
}
