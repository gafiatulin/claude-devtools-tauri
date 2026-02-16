use serde::{Deserialize, Serialize};

use super::domain::SessionMetrics;
use super::jsonl::ToolUseResultData;
use super::messages::{ParsedMessage, ToolCall, ToolResult};

// =============================================================================
// Process Types (Subagent Execution)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    pub id: String,
    pub file_path: String,
    pub messages: Vec<ParsedMessage>,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: f64,
    pub metrics: SessionMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    pub is_parallel: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_ongoing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_session_impact: Option<MainSessionImpact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainSessionImpact {
    pub call_tokens: u64,
    pub result_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamMetadata {
    pub team_name: String,
    pub member_name: String,
    pub member_color: String,
}

// =============================================================================
// Chunk Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "chunkType")]
pub enum Chunk {
    #[serde(rename = "user")]
    User(UserChunkData),
    #[serde(rename = "ai")]
    Ai(AiChunkData),
    #[serde(rename = "system")]
    System(SystemChunkData),
    #[serde(rename = "compact")]
    Compact(CompactChunkData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseChunkFields {
    pub id: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: f64,
    pub metrics: SessionMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub user_message: ParsedMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub responses: Vec<ParsedMessage>,
    pub processes: Vec<Process>,
    pub sidechain_messages: Vec<ParsedMessage>,
    pub tool_executions: Vec<ToolExecution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub message: ParsedMessage,
    pub command_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub message: ParsedMessage,
}

// =============================================================================
// Tool Execution
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    pub tool_call: ToolCall,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ToolResult>,
    pub start_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
}

// =============================================================================
// Conversation Group Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecution {
    pub task_call: ToolCall,
    pub task_call_timestamp: String,
    pub subagent: Process,
    pub tool_result: ParsedMessage,
    pub result_timestamp: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationGroup {
    pub id: String,
    #[serde(rename = "type")]
    pub group_type: String, // "user-ai-exchange"
    pub user_message: ParsedMessage,
    pub ai_responses: Vec<ParsedMessage>,
    pub processes: Vec<Process>,
    pub tool_executions: Vec<ToolExecution>,
    pub task_executions: Vec<TaskExecution>,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: f64,
    pub metrics: SessionMetrics,
}

// =============================================================================
// Semantic Step Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStepType {
    Thinking,
    ToolCall,
    ToolResult,
    Subagent,
    Output,
    Interruption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStep {
    pub id: String,
    #[serde(rename = "type")]
    pub step_type: SemanticStepType,
    pub start_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    pub duration_ms: f64,
    pub content: SemanticStepContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<StepTokens>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_parallel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    pub context: String, // "main" | "subagent"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_gap_filled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accumulated_context: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_breakdown: Option<StepTokenBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStepContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<ToolUseResultData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interruption_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTokens {
    pub input: u64,
    pub output: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTokenBreakdown {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStepGroup {
    pub id: String,
    pub label: String,
    pub steps: Vec<SemanticStep>,
    pub is_grouped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_message_id: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub total_duration: f64,
}

// =============================================================================
// Enhanced Chunk Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "chunkType")]
pub enum EnhancedChunk {
    #[serde(rename = "user")]
    User(EnhancedUserChunkData),
    #[serde(rename = "ai")]
    Ai(EnhancedAiChunkData),
    #[serde(rename = "system")]
    System(EnhancedSystemChunkData),
    #[serde(rename = "compact")]
    Compact(EnhancedCompactChunkData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedUserChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub user_message: ParsedMessage,
    /// Marker field used by the frontend as a type-guard discriminant.
    /// The actual message data is in `user_message`.
    pub enhanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedAiChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub responses: Vec<ParsedMessage>,
    pub processes: Vec<Process>,
    pub sidechain_messages: Vec<ParsedMessage>,
    pub tool_executions: Vec<ToolExecution>,
    pub semantic_steps: Vec<SemanticStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_step_groups: Option<Vec<SemanticStepGroup>>,
    /// Marker field used by the frontend as a type-guard discriminant.
    /// The actual messages are in `responses` and `sidechain_messages`.
    pub enhanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedSystemChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub message: ParsedMessage,
    pub command_output: String,
    /// Marker field used by the frontend as a type-guard discriminant.
    /// The actual message data is in `message`.
    pub enhanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedCompactChunkData {
    #[serde(flatten)]
    pub base: BaseChunkFields,
    pub message: ParsedMessage,
    /// Marker field used by the frontend as a type-guard discriminant.
    /// The actual message data is in `message`.
    pub enhanced: bool,
}

// =============================================================================
// Session Detail
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: super::domain::Session,
    pub messages: Vec<ParsedMessage>,
    pub chunks: Vec<EnhancedChunk>,
    pub processes: Vec<Process>,
    pub metrics: SessionMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDetail {
    pub id: String,
    pub description: String,
    pub chunks: Vec<EnhancedChunk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_step_groups: Option<Vec<SemanticStepGroup>>,
    pub start_time: String,
    pub end_time: String,
    pub duration: f64,
    pub metrics: SubagentMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub thinking_tokens: u64,
    pub message_count: u32,
}

// =============================================================================
// File Change Event
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "add" | "change" | "unlink"
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub is_subagent: bool,
}
