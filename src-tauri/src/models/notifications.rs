use serde::{Deserialize, Serialize};

// =============================================================================
// Detected Error
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedError {
    pub id: String,
    pub timestamp: f64,
    pub session_id: String,
    pub project_id: String,
    pub file_path: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<String>,
    pub is_read: bool,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_color: Option<String>, // TriggerColor: preset key or hex string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_name: Option<String>,
    pub context: DetectedErrorContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedErrorContext {
    pub project_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

// =============================================================================
// Notifications Result
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsResult {
    pub notifications: Vec<DetectedError>,
    pub total: u32,
    pub total_count: u32,
    pub unread_count: u32,
    pub has_more: bool,
}

// =============================================================================
// Trigger Types
// =============================================================================

pub type TriggerContentType = String; // "tool_result" | "tool_use" | "thinking" | "text"
pub type TriggerMode = String; // "error_status" | "content_match" | "token_threshold"
pub type TriggerTokenType = String; // "input" | "output" | "total"
pub type TriggerMatchField = String; // "content" | "command" | "description" | etc.
pub type TriggerToolName = String;

pub const KNOWN_TOOL_NAMES: &[&str] = &[
    "Bash",
    "Task",
    "TodoWrite",
    "Read",
    "Write",
    "Edit",
    "Grep",
    "Glob",
    "WebFetch",
    "WebSearch",
    "LSP",
    "Skill",
    "NotebookEdit",
    "AskUserQuestion",
    "KillShell",
    "TaskOutput",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTrigger {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub content_type: TriggerContentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<TriggerToolName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_builtin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_patterns: Option<Vec<String>>,

    pub mode: TriggerMode,

    // Mode: error_status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_error: Option<bool>,

    // Mode: content_match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_field: Option<TriggerMatchField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_pattern: Option<String>,

    // Mode: token_threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_threshold: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<TriggerTokenType>,

    // Repository scope
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_ids: Option<Vec<String>>,

    // Display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>, // TriggerColor
}

// =============================================================================
// Trigger Test Result
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerTestResult {
    pub total_count: u32,
    pub errors: Vec<TriggerTestError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerTestError {
    pub id: String,
    pub session_id: String,
    pub project_id: String,
    pub message: String,
    pub timestamp: f64,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
    pub context: TriggerTestErrorContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerTestErrorContext {
    pub project_name: String,
}
