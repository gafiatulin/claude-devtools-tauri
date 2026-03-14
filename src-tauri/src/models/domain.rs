use serde::{Deserialize, Serialize};

use super::jsonl::UsageMetadata;

/// Token usage statistics (alias for API compatibility).
pub type TokenUsage = UsageMetadata;

/// Message type classification for parsed messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MessageType {
    User,
    Assistant,
    System,
    Summary,
    FileHistorySnapshot,
    QueueOperation,
}

/// Message category for chunk building.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MessageCategory {
    User,
    System,
    HardNoise,
    Ai,
    Compact,
}

// =============================================================================
// Project & Session Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: String,
    pub sessions: Vec<String>,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_recent_session: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionMetadataLevel {
    Light,
    Deep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTokenBreakdown {
    pub phase_number: u32,
    pub contribution: u64,
    pub peak_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_compaction: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub project_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todo_data: Option<serde_json::Value>,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_timestamp: Option<String>,
    pub has_subagents: bool,
    pub message_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_ongoing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_level: Option<SessionMetadataLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_consumption: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_breakdown: Option<Vec<PhaseTokenBreakdown>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_plan_content: Option<bool>,
    /// Session name set via `/rename` (from custom-title JSONL entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetrics {
    pub duration_ms: f64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub message_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

impl Default for SessionMetrics {
    fn default() -> Self {
        Self {
            duration_ms: 0.0,
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            message_count: 0,
            cost_usd: None,
        }
    }
}

// =============================================================================
// Repository & Worktree Grouping Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeSource {
    VibeKanban,
    Conductor,
    AutoClaude,
    #[serde(rename = "21st")]
    TwentyFirst,
    ClaudeDesktop,
    Ccswitch,
    Git,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryIdentity {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    pub main_git_dir: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    pub is_main_worktree: bool,
    pub source: WorktreeSource,
    pub sessions: Vec<String>,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_recent_session: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGroup {
    pub id: String,
    pub identity: Option<RepositoryIdentity>,
    pub worktrees: Vec<Worktree>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_recent_session: Option<f64>,
    pub total_sessions: u32,
}

// =============================================================================
// Search Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub session_id: String,
    pub project_id: String,
    pub session_title: String,
    pub matched_text: String,
    pub context: String,
    pub message_type: String, // "user" | "assistant"
    pub timestamp: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>, // "user" | "ai"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_index_in_item: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_start_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSessionsResult {
    pub results: Vec<SearchResult>,
    pub total_matches: u32,
    pub sessions_searched: u32,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_partial: Option<bool>,
}

// =============================================================================
// Pagination Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCursor {
    pub timestamp: f64,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedSessionsResult {
    pub sessions: Vec<Session>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsPaginationOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_total_count: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefilter_all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_level: Option<SessionMetadataLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsByIdsOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_level: Option<SessionMetadataLevel>,
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    /// Verify chrono DateTime<Utc> serializes as ISO 8601 with the T separator
    /// that the frontend's reviveDates() regex expects: /^\d{4}-\d{2}-\d{2}T/
    #[test]
    fn test_datetime_serializes_as_iso8601() {
        use chrono::{TimeZone, Utc};

        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 12, 30, 0).unwrap();
        let json = serde_json::to_string(&dt).unwrap();
        println!("Serialized DateTime<Utc>: {}", json);
        assert!(json.contains("2024-01-15"), "Should contain date");
        assert!(
            json.contains("T12:30:00"),
            "Should contain time with T separator"
        );
        assert!(
            json.ends_with("Z\"") || json.contains("+00:00"),
            "Should have UTC indicator, got: {}",
            json
        );
    }

    /// Verify Utc::now().to_rfc3339() (the fallback_timestamp format used in
    /// jsonl/parser.rs) produces frontend-compatible ISO 8601.
    #[test]
    fn test_fallback_timestamp_format() {
        let ts = chrono::Utc::now().to_rfc3339();
        println!("Fallback timestamp (to_rfc3339): {}", ts);
        // Frontend regex: /^\d{4}-\d{2}-\d{2}T/
        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}").unwrap();
        assert!(
            re.is_match(&ts),
            "to_rfc3339() must match frontend date regex, got: {}",
            ts
        );
        assert!(
            ts.contains("+00:00") || ts.ends_with('Z'),
            "Must have UTC offset, got: {}",
            ts
        );
    }

    /// Verify that JSONL source timestamps pass through unchanged as strings.
    /// The ParsedMessage.timestamp field is a String, not DateTime, so it
    /// serializes as-is with no transformation.
    #[test]
    fn test_timestamp_string_passthrough() {
        use crate::models::jsonl::StringOrBlocks;
        use crate::models::messages::ParsedMessage;

        let original_ts = "2024-06-15T09:30:00.123Z";
        let msg = ParsedMessage {
            uuid: "test-uuid".to_string(),
            parent_uuid: None,
            message_type: MessageType::User,
            timestamp: original_ts.to_string(),
            role: Some("user".to_string()),
            content: StringOrBlocks::String("hello".to_string()),
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
            plan_content: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        println!("ParsedMessage JSON (timestamp field): {}", json);
        assert!(
            json.contains(original_ts),
            "Timestamp string must pass through unchanged, got: {}",
            json
        );
    }

    /// Verify Session.createdAt serializes as a numeric millisecond timestamp (f64),
    /// matching what the TypeScript frontend expects for sorting and display.
    #[test]
    fn test_session_created_at_is_numeric() {
        let session = Session {
            id: "test-id".to_string(),
            project_id: "test-project".to_string(),
            project_path: "/tmp".to_string(),
            todo_data: None,
            created_at: 1705312200000.0, // 2024-01-15T12:30:00Z in epoch ms
            first_message: Some("hello".to_string()),
            message_timestamp: Some("2024-01-15T12:30:00Z".to_string()),
            has_subagents: false,
            message_count: 5,
            is_ongoing: Some(false),
            git_branch: Some("main".to_string()),
            metadata_level: Some(SessionMetadataLevel::Deep),
            context_consumption: Some(10000),
            compaction_count: None,
            phase_breakdown: None,
            slug: None,
            has_plan_content: None,
            session_name: None,
        };

        let json = serde_json::to_string(&session).unwrap();
        println!("Session JSON: {}", json);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        // createdAt must be a number (epoch ms), not a string
        assert!(
            value["createdAt"].is_number(),
            "createdAt must serialize as a number, got: {}",
            value["createdAt"]
        );
        // messageTimestamp must be a string (ISO 8601)
        assert!(
            value["messageTimestamp"].is_string(),
            "messageTimestamp must serialize as a string, got: {}",
            value["messageTimestamp"]
        );
        // Verify camelCase field naming
        assert!(
            value.get("projectId").is_some(),
            "Fields must use camelCase (projectId), got keys: {:?}",
            value.as_object().unwrap().keys().collect::<Vec<_>>()
        );
        assert!(
            value.get("project_id").is_none(),
            "Fields must NOT use snake_case"
        );
    }

    /// Verify SessionMetrics roundtrip serialization matches frontend expectations.
    #[test]
    fn test_session_metrics_serde_roundtrip() {
        let metrics = SessionMetrics {
            duration_ms: 12345.6,
            total_tokens: 50000,
            input_tokens: 30000,
            output_tokens: 20000,
            cache_read_tokens: 5000,
            cache_creation_tokens: 1000,
            message_count: 42,
            cost_usd: Some(0.15),
        };

        let json = serde_json::to_string(&metrics).unwrap();
        println!("SessionMetrics JSON: {}", json);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["durationMs"], 12345.6);
        assert_eq!(value["totalTokens"], 50000);
        assert_eq!(value["messageCount"], 42);
        assert_eq!(value["costUsd"], 0.15);

        // Roundtrip
        let deserialized: SessionMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_tokens, metrics.total_tokens);
        assert_eq!(deserialized.message_count, metrics.message_count);
    }

    /// Verify Optional fields with skip_serializing_if are omitted when None,
    /// matching the Node.js behavior where undefined fields are not included.
    #[test]
    fn test_optional_fields_omitted_when_none() {
        let session = Session {
            id: "s1".to_string(),
            project_id: "p1".to_string(),
            project_path: "/tmp".to_string(),
            todo_data: None,
            created_at: 1000.0,
            first_message: None,
            message_timestamp: None,
            has_subagents: false,
            message_count: 0,
            is_ongoing: None,
            git_branch: None,
            metadata_level: None,
            context_consumption: None,
            compaction_count: None,
            phase_breakdown: None,
            slug: None,
            has_plan_content: None,
            session_name: None,
        };

        let json = serde_json::to_string(&session).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        println!("Session with Nones: {}", json);

        // These None fields should be absent from JSON (matching Node.js undefined)
        assert!(
            value.get("firstMessage").is_none(),
            "None firstMessage should be omitted"
        );
        assert!(
            value.get("gitBranch").is_none(),
            "None gitBranch should be omitted"
        );
        assert!(
            value.get("isOngoing").is_none(),
            "None isOngoing should be omitted"
        );
        assert!(
            value.get("todoData").is_none(),
            "None todoData should be omitted"
        );

        // These required fields must always be present
        assert!(value.get("id").is_some(), "id must always be present");
        assert!(
            value.get("hasSubagents").is_some(),
            "hasSubagents must always be present"
        );
        assert!(
            value.get("messageCount").is_some(),
            "messageCount must always be present"
        );
    }
}
