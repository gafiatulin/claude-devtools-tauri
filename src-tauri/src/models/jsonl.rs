use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================================
// Content Blocks
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: StringOrBlocks,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    /// Document block (e.g., PDF or image attachment via document API).
    #[serde(rename = "document")]
    Document { source: ImageSource },
    /// Catch-all for unknown block types.
    /// Prevents parse failures when Claude adds new content block types.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String,  // "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    pub data: String,
}

/// Represents `string | ContentBlock[]` in TypeScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrBlocks {
    String(String),
    Blocks(Vec<ContentBlock>),
}

// =============================================================================
// Usage Metadata
// =============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageMetadata {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
}

// =============================================================================
// Messages (inner message payloads within entries)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: String, // "user"
    pub content: StringOrBlocks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: String, // "assistant"
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub message_type: String, // "message"
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    #[serde(default)]
    pub usage: UsageMetadata,
}

// =============================================================================
// Tool Use Result Data
// =============================================================================

pub type ToolUseResultData = serde_json::Value;

// =============================================================================
// JSONL Entries
// =============================================================================

/// All known JSONL entry types. Unknown types (e.g., "progress") are captured
/// by the `Other` variant so deserialization never fails on new entry types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatHistoryEntry {
    #[serde(rename = "user")]
    User(UserEntry),
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(rename = "system")]
    System(SystemEntry),
    #[serde(rename = "summary")]
    Summary(SummaryEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(FileHistorySnapshotEntry),
    #[serde(rename = "queue-operation")]
    QueueOperation(QueueOperationEntry),
    /// Catch-all for unknown entry types (e.g., "progress").
    /// Prevents deserialization failures when new entry types appear.
    #[serde(other)]
    Unknown,
}

/// Shared fields for conversational entries (user, assistant, system).
///
/// Many fields use `#[serde(default)]` because real JSONL data may omit them,
/// and we need deserialization to be resilient.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationalFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub is_sidechain: bool,
    #[serde(default)]
    pub user_type: String, // "external"
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub git_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntry {
    #[serde(flatten)]
    pub common: ConversationalFields,
    pub message: UserMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<ToolUseResultData>,
    #[serde(rename = "sourceToolUseID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tool_use_id: Option<String>,
    #[serde(rename = "sourceToolAssistantUUID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tool_assistant_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantEntry {
    #[serde(flatten)]
    pub common: ConversationalFields,
    pub message: AssistantMessage,
    #[serde(default)]
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

/// System entry covers multiple subtypes: "turn_duration", "init",
/// "local_command", "compact_boundary", etc.
///
/// Fields like `duration_ms` and `content` are optional since they only
/// appear for certain subtypes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemEntry {
    #[serde(flatten)]
    pub common: ConversationalFields,
    #[serde(default)]
    pub subtype: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    #[serde(default)]
    pub is_meta: bool,
    /// Content field present on local_command and compact_boundary subtypes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Log level (e.g., "info").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Compaction metadata for compact_boundary subtype.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub summary: String,
    pub leaf_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySnapshotEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub message_id: String,
    pub snapshot: FileHistorySnapshot,
    pub is_snapshot_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySnapshot {
    pub message_id: String,
    pub tracked_file_backups: HashMap<String, serde_json::Value>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueOperationEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub operation: String,
}

// =============================================================================
// Content helpers
// =============================================================================

impl ContentBlock {
    pub fn is_text(&self) -> bool {
        matches!(self, ContentBlock::Text { .. })
    }

    pub fn is_tool_result(&self) -> bool {
        matches!(self, ContentBlock::ToolResult { .. })
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }
}

impl ChatHistoryEntry {
    pub fn is_conversational(&self) -> bool {
        matches!(
            self,
            ChatHistoryEntry::User(_) | ChatHistoryEntry::Assistant(_) | ChatHistoryEntry::System(_)
        )
    }
}
