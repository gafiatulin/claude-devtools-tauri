use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::notifications::NotificationTrigger;

// =============================================================================
// Application Configuration
// =============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub notifications: NotificationsConfig,
    pub general: GeneralConfig,
    pub display: DisplayConfig,
    pub sessions: SessionsConfig,
    // httpServer dropped — local-only, no HTTP sidecar server
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsConfig {
    pub enabled: bool,
    pub sound_enabled: bool,
    pub ignored_regex: Vec<String>,
    pub ignored_repositories: Vec<String>,
    pub snoozed_until: Option<f64>,
    pub snooze_minutes: u32,
    pub include_subagent_errors: bool,
    pub triggers: Vec<NotificationTrigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConfig {
    // launchAtLogin dropped — Electron-specific
    // showDockIcon dropped — Electron-specific
    pub theme: String,       // "dark" | "light" | "system"
    pub default_tab: String, // "dashboard" | "last-session"
    pub claude_root_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayConfig {
    pub show_timestamps: bool,
    pub compact_mode: bool,
    pub syntax_highlighting: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsConfig {
    pub pinned_sessions: HashMap<String, Vec<PinnedSession>>,
    pub hidden_sessions: HashMap<String, Vec<HiddenSession>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedSession {
    pub session_id: String,
    pub pinned_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenSession {
    pub session_id: String,
    pub hidden_at: f64,
}

// =============================================================================
// Defaults
// =============================================================================

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: false,
            ignored_regex: Vec::new(),
            ignored_repositories: Vec::new(),
            snoozed_until: None,
            snooze_minutes: 30,
            include_subagent_errors: true,
            triggers: Vec::new(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            default_tab: "dashboard".to_string(),
            claude_root_path: None,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_timestamps: true,
            compact_mode: false,
            syntax_highlighting: true,
        }
    }
}
