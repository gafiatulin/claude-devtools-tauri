use crate::models::config::{
    AppConfig, DisplayConfig, GeneralConfig, NotificationsConfig, SessionsConfig,
};
use crate::models::notifications::NotificationTrigger;
use std::collections::HashMap;

/// Returns the default set of built-in notification triggers.
///
/// These match the TypeScript defaults from `useSettingsHandlers.ts`:
/// 1. Tool Result Error — fires on any tool result with `is_error: true`
/// 2. Bash Command Alert for .env files — fires when Bash commands reference `.env`
pub fn default_triggers() -> Vec<NotificationTrigger> {
    vec![
        NotificationTrigger {
            id: "builtin-tool-result-error".to_string(),
            name: "Tool Result Error".to_string(),
            enabled: true,
            content_type: "tool_result".to_string(),
            tool_name: None,
            is_builtin: Some(true),
            ignore_patterns: Some(vec![
                r"The user doesn't want to proceed with this tool use\.".to_string(),
            ]),
            mode: "error_status".to_string(),
            require_error: Some(true),
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        },
        NotificationTrigger {
            id: "builtin-bash-command".to_string(),
            name: "Bash Command Alert for .env files".to_string(),
            enabled: true,
            content_type: "tool_use".to_string(),
            tool_name: Some("Bash".to_string()),
            is_builtin: Some(true),
            ignore_patterns: None,
            mode: "content_match".to_string(),
            require_error: None,
            match_field: Some("command".to_string()),
            match_pattern: Some("/.env".to_string()),
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        },
    ]
}

/// Returns the default ignored regex patterns for notifications.
pub fn default_ignored_regex() -> Vec<String> {
    vec![r"The user doesn't want to proceed with this tool use\.".to_string()]
}

/// Returns the full default application configuration.
///
/// Matches the TypeScript defaults from `useSettingsHandlers.ts` `handleResetToDefaults`.
pub fn default_config() -> AppConfig {
    AppConfig {
        notifications: NotificationsConfig {
            enabled: true,
            sound_enabled: true,
            ignored_regex: default_ignored_regex(),
            ignored_repositories: Vec::new(),
            snoozed_until: None,
            snooze_minutes: 30,
            include_subagent_errors: true,
            triggers: default_triggers(),
        },
        general: GeneralConfig {
            theme: "dark".to_string(),
            default_tab: "dashboard".to_string(),
            claude_root_path: None,
        },
        display: DisplayConfig {
            show_timestamps: true,
            compact_mode: false,
            syntax_highlighting: true,
        },
        sessions: SessionsConfig {
            pinned_sessions: HashMap::new(),
            hidden_sessions: HashMap::new(),
        },
    }
}
