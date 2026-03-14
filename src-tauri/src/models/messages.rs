use serde::{Deserialize, Serialize};

use super::constants::{HARD_NOISE_TAGS, SYSTEM_OUTPUT_TAGS};
use super::domain::{MessageType, TokenUsage};
use super::jsonl::{ContentBlock, StringOrBlocks, ToolUseResultData};

// =============================================================================
// Tool Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub is_task: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_subagent_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: serde_json::Value, // string | unknown[]
    pub is_error: bool,
}

// =============================================================================
// Parsed Message
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMessage {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub timestamp: String, // ISO 8601 string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub content: StringOrBlocks,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub is_sidechain: bool,
    pub is_meta: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_type: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    #[serde(rename = "sourceToolUseID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tool_use_id: Option<String>,
    #[serde(rename = "sourceToolAssistantUUID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tool_assistant_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<ToolUseResultData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_compact_summary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_content: Option<String>,
}

// =============================================================================
// Message Classification Functions
// =============================================================================

const TEAMMATE_MESSAGE_PREFIX: &str = "<teammate-message teammate_id=\"";

impl ParsedMessage {
    /// Check if this is a real user message (both old string and new array formats).
    pub fn is_parsed_real_user_message(&self) -> bool {
        if self.message_type != MessageType::User {
            return false;
        }
        if self.is_meta {
            return false;
        }
        match &self.content {
            StringOrBlocks::String(_) => true,
            StringOrBlocks::Blocks(blocks) => blocks
                .iter()
                .any(|b| b.is_text() || matches!(b, ContentBlock::Image { .. })),
        }
    }

    /// Check if this message should create a User chunk.
    pub fn is_parsed_user_chunk_message(&self) -> bool {
        if self.message_type != MessageType::User {
            return false;
        }
        if self.is_meta {
            return false;
        }
        if self.is_parsed_teammate_message() {
            return false;
        }

        match &self.content {
            StringOrBlocks::String(s) => {
                let trimmed = s.trim();
                for tag in SYSTEM_OUTPUT_TAGS {
                    if trimmed.starts_with(tag) {
                        return false;
                    }
                }
                !trimmed.is_empty()
            }
            StringOrBlocks::Blocks(blocks) => {
                let has_user_content = blocks
                    .iter()
                    .any(|b| b.is_text() || matches!(b, ContentBlock::Image { .. }));
                if !has_user_content {
                    return false;
                }

                // Filter interruption messages
                if blocks.len() == 1 {
                    if let Some(text) = blocks[0].as_text() {
                        if text.starts_with("[Request interrupted by user") {
                            return false;
                        }
                    }
                }

                // Check text blocks for excluded tags
                for block in blocks {
                    if let Some(text) = block.as_text() {
                        for tag in SYSTEM_OUTPUT_TAGS {
                            if text.starts_with(tag) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
        }
    }

    /// Check if this message should create a System chunk.
    pub fn is_parsed_system_chunk_message(&self) -> bool {
        if self.message_type != MessageType::User {
            return false;
        }
        match &self.content {
            StringOrBlocks::String(s) => {
                s.starts_with(super::constants::LOCAL_COMMAND_STDOUT_TAG)
                    || s.starts_with(super::constants::LOCAL_COMMAND_STDERR_TAG)
            }
            StringOrBlocks::Blocks(blocks) => blocks.iter().any(|block| {
                block
                    .as_text()
                    .is_some_and(|t| t.starts_with(super::constants::LOCAL_COMMAND_STDOUT_TAG))
            }),
        }
    }

    /// Check if this is an internal user message (tool results).
    pub fn is_parsed_internal_user_message(&self) -> bool {
        self.message_type == MessageType::User && self.is_meta
    }

    /// Check if this message should be filtered out entirely.
    pub fn is_parsed_hard_noise_message(&self) -> bool {
        match self.message_type {
            MessageType::System
            | MessageType::Summary
            | MessageType::FileHistorySnapshot
            | MessageType::QueueOperation => return true,
            _ => {}
        }

        // Filter synthetic assistant messages
        if self.message_type == MessageType::Assistant {
            if let Some(model) = &self.model {
                if model == "<synthetic>" {
                    return true;
                }
            }
        }

        if self.message_type == MessageType::User {
            match &self.content {
                StringOrBlocks::String(s) => {
                    let trimmed = s.trim();
                    for tag in HARD_NOISE_TAGS {
                        let close_tag = tag.replace('<', "</");
                        if trimmed.starts_with(tag) && trimmed.ends_with(&close_tag) {
                            return true;
                        }
                    }
                    if trimmed == super::constants::EMPTY_STDOUT
                        || trimmed == super::constants::EMPTY_STDERR
                    {
                        return true;
                    }
                    if trimmed.starts_with("[Request interrupted by user") {
                        return true;
                    }
                }
                StringOrBlocks::Blocks(blocks) => {
                    if blocks.len() == 1 {
                        if let Some(text) = blocks[0].as_text() {
                            if text.starts_with("[Request interrupted by user") {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Detect compact summary messages.
    pub fn is_parsed_compact_message(&self) -> bool {
        self.is_compact_summary.unwrap_or(false)
    }

    /// Detect teammate messages.
    fn is_parsed_teammate_message(&self) -> bool {
        if self.message_type != MessageType::User || self.is_meta {
            return false;
        }
        match &self.content {
            StringOrBlocks::String(s) => s.trim().starts_with(TEAMMATE_MESSAGE_PREFIX),
            StringOrBlocks::Blocks(blocks) => blocks.iter().any(|block| {
                block
                    .as_text()
                    .is_some_and(|t| t.trim().starts_with(TEAMMATE_MESSAGE_PREFIX))
            }),
        }
    }
}
