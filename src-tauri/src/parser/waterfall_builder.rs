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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::*;
    use crate::models::domain::{MessageType, Session, SessionMetrics, TokenUsage};
    use crate::models::jsonl::StringOrBlocks;
    use crate::models::messages::{ParsedMessage, ToolCall};

    fn default_metrics() -> SessionMetrics {
        SessionMetrics {
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

    fn make_base(id: &str, start: &str, end: &str) -> BaseChunkFields {
        BaseChunkFields {
            id: id.to_string(),
            start_time: start.to_string(),
            end_time: end.to_string(),
            duration_ms: 1000.0,
            metrics: default_metrics(),
        }
    }

    fn make_user_msg(uuid: &str, ts: &str) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::User,
            timestamp: ts.to_string(),
            role: None,
            content: StringOrBlocks::String("Hello".to_string()),
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
        }
    }

    fn make_ai_msg(uuid: &str, ts: &str) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::Assistant,
            timestamp: ts.to_string(),
            role: None,
            content: StringOrBlocks::Blocks(vec![]),
            usage: Some(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            }),
            model: Some("claude-sonnet".to_string()),
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
        }
    }

    fn make_session_detail(chunks: Vec<EnhancedChunk>) -> SessionDetail {
        SessionDetail {
            session: Session {
                id: "sess1".to_string(),
                project_id: "proj1".to_string(),
                project_path: "/tmp".to_string(),
                todo_data: None,
                created_at: 0.0,
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
            },
            messages: Vec::new(),
            chunks,
            processes: Vec::new(),
            metrics: default_metrics(),
        }
    }

    #[test]
    fn test_build_waterfall_empty() {
        let detail = make_session_detail(vec![]);
        let waterfall = build_waterfall(&detail);
        assert!(waterfall.items.is_empty());
        assert_eq!(waterfall.total_duration_ms, 0.0);
    }

    #[test]
    fn test_build_waterfall_user_chunk() {
        let detail = make_session_detail(vec![
            EnhancedChunk::User(EnhancedUserChunkData {
                base: make_base("c1", "2025-01-01T00:00:00Z", "2025-01-01T00:00:01Z"),
                user_message: make_user_msg("u1", "2025-01-01T00:00:00Z"),
                enhanced: true,
            }),
        ]);

        let waterfall = build_waterfall(&detail);
        assert_eq!(waterfall.items.len(), 1);
        assert_eq!(waterfall.items[0].label, "User message");
        assert_eq!(waterfall.items[0].level, 0);
        assert_eq!(waterfall.items[0].item_type, "chunk");
    }

    #[test]
    fn test_build_waterfall_ai_with_tools() {
        let mut ai_msg = make_ai_msg("a1", "2025-01-01T00:00:00Z");
        ai_msg.tool_calls = vec![ToolCall {
            id: "tc1".to_string(),
            name: "Bash".to_string(),
            input: serde_json::json!({}),
            is_task: false,
            task_description: None,
            task_subagent_type: None,
        }];

        let te = ToolExecution {
            tool_call: ai_msg.tool_calls[0].clone(),
            result: None,
            start_time: "2025-01-01T00:00:00Z".to_string(),
            end_time: Some("2025-01-01T00:00:02Z".to_string()),
            duration_ms: Some(2000.0),
            progress: None,
        };

        let detail = make_session_detail(vec![
            EnhancedChunk::Ai(EnhancedAiChunkData {
                base: make_base("c1", "2025-01-01T00:00:00Z", "2025-01-01T00:00:02Z"),
                responses: vec![ai_msg],
                processes: Vec::new(),
                sidechain_messages: Vec::new(),
                tool_executions: vec![te],
                semantic_steps: Vec::new(),
                semantic_step_groups: None,
                enhanced: true,
            }),
        ]);

        let waterfall = build_waterfall(&detail);
        assert_eq!(waterfall.items.len(), 2); // chunk + tool
        assert_eq!(waterfall.items[0].level, 0);
        assert_eq!(waterfall.items[1].level, 1);
        assert_eq!(waterfall.items[1].item_type, "tool");
        assert_eq!(waterfall.items[1].label, "Bash");
        assert_eq!(waterfall.items[1].parent_id.as_deref(), Some("c1"));
    }

    #[test]
    fn test_build_waterfall_ai_label_no_tools() {
        let detail = make_session_detail(vec![
            EnhancedChunk::Ai(EnhancedAiChunkData {
                base: make_base("c1", "2025-01-01T00:00:00Z", "2025-01-01T00:00:01Z"),
                responses: vec![make_ai_msg("a1", "2025-01-01T00:00:00Z")],
                processes: Vec::new(),
                sidechain_messages: Vec::new(),
                tool_executions: Vec::new(),
                semantic_steps: Vec::new(),
                semantic_step_groups: None,
                enhanced: true,
            }),
        ]);

        let waterfall = build_waterfall(&detail);
        assert_eq!(waterfall.items[0].label, "AI response");
    }

    #[test]
    fn test_build_waterfall_compact_chunk() {
        let detail = make_session_detail(vec![
            EnhancedChunk::Compact(EnhancedCompactChunkData {
                base: make_base("c1", "2025-01-01T00:00:00Z", "2025-01-01T00:00:00Z"),
                message: make_user_msg("u1", "2025-01-01T00:00:00Z"),
                enhanced: true,
            }),
        ]);

        let waterfall = build_waterfall(&detail);
        assert_eq!(waterfall.items.len(), 1);
        assert_eq!(waterfall.items[0].label, "Compaction boundary");
    }

    #[test]
    fn test_build_waterfall_total_duration() {
        let detail = make_session_detail(vec![
            EnhancedChunk::User(EnhancedUserChunkData {
                base: make_base("c1", "2025-01-01T00:00:00Z", "2025-01-01T00:00:01Z"),
                user_message: make_user_msg("u1", "2025-01-01T00:00:00Z"),
                enhanced: true,
            }),
            EnhancedChunk::Ai(EnhancedAiChunkData {
                base: make_base("c2", "2025-01-01T00:00:01Z", "2025-01-01T00:00:10Z"),
                responses: vec![make_ai_msg("a1", "2025-01-01T00:00:01Z")],
                processes: Vec::new(),
                sidechain_messages: Vec::new(),
                tool_executions: Vec::new(),
                semantic_steps: Vec::new(),
                semantic_step_groups: None,
                enhanced: true,
            }),
        ]);

        let waterfall = build_waterfall(&detail);
        assert_eq!(waterfall.total_duration_ms, 10000.0);
        assert_eq!(waterfall.min_time, "2025-01-01T00:00:00Z");
        assert_eq!(waterfall.max_time, "2025-01-01T00:00:10Z");
    }
}
