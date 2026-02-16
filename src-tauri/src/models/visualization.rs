use serde::{Deserialize, Serialize};

use super::domain::TokenUsage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterfallItem {
    pub id: String,
    pub label: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: f64,
    pub token_usage: TokenUsage,
    pub level: u32,
    #[serde(rename = "type")]
    pub item_type: String, // "chunk" | "subagent" | "tool"
    pub is_parallel: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<WaterfallItemMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterfallItemMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterfallData {
    pub items: Vec<WaterfallItem>,
    pub min_time: String,
    pub max_time: String,
    pub total_duration_ms: f64,
}
