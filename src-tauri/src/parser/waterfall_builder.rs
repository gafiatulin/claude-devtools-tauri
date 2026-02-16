use crate::models::chunks::{EnhancedChunk, SessionDetail};
use crate::models::domain::SessionMetrics;
use crate::models::visualization::{WaterfallData, WaterfallItem, WaterfallItemMetadata};

/// Build waterfall visualization data from a session detail.
///
/// Flattens chunks + subagents + tool executions into a list of WaterfallItems.
/// Each item has: id, type, label, startTime, endTime, durationMs, level, parentId.
/// level 0 = main session, 1+ = subagent depth.
pub fn build_waterfall(detail: &SessionDetail) -> WaterfallData {
    let mut items: Vec<WaterfallItem> = Vec::new();

    // Add chunks at level 0
    for chunk in &detail.chunks {
        match chunk {
            EnhancedChunk::User(data) => {
                items.push(WaterfallItem {
                    id: data.base.id.clone(),
                    label: "User message".to_string(),
                    start_time: data.base.start_time.clone(),
                    end_time: data.base.end_time.clone(),
                    duration_ms: data.base.duration_ms,
                    token_usage: default_token_usage(),
                    level: 0,
                    item_type: "chunk".to_string(),
                    is_parallel: false,
                    parent_id: None,
                    group_id: None,
                    metadata: None,
                });
            }
            EnhancedChunk::Ai(data) => {
                let chunk_id = data.base.id.clone();

                items.push(WaterfallItem {
                    id: chunk_id.clone(),
                    label: build_ai_chunk_label(data),
                    start_time: data.base.start_time.clone(),
                    end_time: data.base.end_time.clone(),
                    duration_ms: data.base.duration_ms,
                    token_usage: metrics_to_token_usage(&data.base.metrics),
                    level: 0,
                    item_type: "chunk".to_string(),
                    is_parallel: false,
                    parent_id: None,
                    group_id: None,
                    metadata: None,
                });

                // Add tool executions as children of this chunk
                for te in &data.tool_executions {
                    items.push(WaterfallItem {
                        id: format!("tool-{}", te.tool_call.id),
                        label: te.tool_call.name.clone(),
                        start_time: te.start_time.clone(),
                        end_time: te.end_time.clone().unwrap_or_else(|| te.start_time.clone()),
                        duration_ms: te.duration_ms.unwrap_or(0.0),
                        token_usage: default_token_usage(),
                        level: 1,
                        item_type: "tool".to_string(),
                        is_parallel: false,
                        parent_id: Some(chunk_id.clone()),
                        group_id: Some(chunk_id.clone()),
                        metadata: Some(WaterfallItemMetadata {
                            subagent_type: None,
                            tool_name: Some(te.tool_call.name.clone()),
                            message_count: None,
                        }),
                    });
                }

                // Add subagent processes as children of this chunk
                for process in &data.processes {
                    items.push(WaterfallItem {
                        id: format!("subagent-{}", process.id),
                        label: process
                            .description
                            .clone()
                            .unwrap_or_else(|| format!("Subagent {}", process.id)),
                        start_time: process.start_time.clone(),
                        end_time: process.end_time.clone(),
                        duration_ms: process.duration_ms,
                        token_usage: metrics_to_token_usage(&process.metrics),
                        level: 1,
                        item_type: "subagent".to_string(),
                        is_parallel: process.is_parallel,
                        parent_id: Some(chunk_id.clone()),
                        group_id: Some(chunk_id.clone()),
                        metadata: Some(WaterfallItemMetadata {
                            subagent_type: process.subagent_type.clone(),
                            tool_name: None,
                            message_count: Some(process.messages.len() as u32),
                        }),
                    });
                }
            }
            EnhancedChunk::System(data) => {
                items.push(WaterfallItem {
                    id: data.base.id.clone(),
                    label: "System output".to_string(),
                    start_time: data.base.start_time.clone(),
                    end_time: data.base.end_time.clone(),
                    duration_ms: data.base.duration_ms,
                    token_usage: default_token_usage(),
                    level: 0,
                    item_type: "chunk".to_string(),
                    is_parallel: false,
                    parent_id: None,
                    group_id: None,
                    metadata: None,
                });
            }
            EnhancedChunk::Compact(data) => {
                items.push(WaterfallItem {
                    id: data.base.id.clone(),
                    label: "Compaction boundary".to_string(),
                    start_time: data.base.start_time.clone(),
                    end_time: data.base.end_time.clone(),
                    duration_ms: data.base.duration_ms,
                    token_usage: default_token_usage(),
                    level: 0,
                    item_type: "chunk".to_string(),
                    is_parallel: false,
                    parent_id: None,
                    group_id: None,
                    metadata: None,
                });
            }
        }
    }

    // Compute min/max times
    let min_time = items
        .iter()
        .map(|i| i.start_time.as_str())
        .min()
        .unwrap_or("")
        .to_string();

    let max_time = items
        .iter()
        .map(|i| i.end_time.as_str())
        .max()
        .unwrap_or("")
        .to_string();

    let total_duration_ms = compute_duration_ms(&min_time, &max_time);

    WaterfallData {
        items,
        min_time,
        max_time,
        total_duration_ms,
    }
}

fn build_ai_chunk_label(data: &crate::models::chunks::EnhancedAiChunkData) -> String {
    let tool_names: Vec<&str> = data
        .responses
        .iter()
        .flat_map(|r| r.tool_calls.iter().map(|tc| tc.name.as_str()))
        .collect();

    if tool_names.is_empty() {
        "AI response".to_string()
    } else if tool_names.len() == 1 {
        format!("AI: {}", tool_names[0])
    } else {
        format!("AI: {} tools", tool_names.len())
    }
}

fn default_token_usage() -> crate::models::domain::TokenUsage {
    crate::models::domain::TokenUsage {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
    }
}

fn metrics_to_token_usage(metrics: &SessionMetrics) -> crate::models::domain::TokenUsage {
    crate::models::domain::TokenUsage {
        input_tokens: metrics.input_tokens,
        output_tokens: metrics.output_tokens,
        cache_read_input_tokens: Some(metrics.cache_read_tokens),
        cache_creation_input_tokens: Some(metrics.cache_creation_tokens),
    }
}

fn compute_duration_ms(start: &str, end: &str) -> f64 {
    if start.is_empty() || end.is_empty() {
        return 0.0;
    }

    let start_dt = chrono::DateTime::parse_from_rfc3339(start);
    let end_dt = chrono::DateTime::parse_from_rfc3339(end);

    match (start_dt, end_dt) {
        (Ok(s), Ok(e)) => {
            let diff = e.signed_duration_since(s);
            diff.num_milliseconds().max(0) as f64
        }
        _ => 0.0,
    }
}
