use crate::models::chunks::{
    AiChunkData, BaseChunkFields, Chunk, CompactChunkData, EnhancedAiChunkData, EnhancedChunk,
    EnhancedCompactChunkData, EnhancedSystemChunkData, EnhancedUserChunkData, Process,
    SystemChunkData, ToolExecution, UserChunkData,
};
use crate::models::domain::{MessageCategory, SessionMetrics};
use crate::models::messages::ParsedMessage;

use super::message_classifier::classify_message;
use super::semantic_extractor;
use super::tool_linker;

/// Build chunks from a sequence of parsed messages and associated processes.
///
/// Algorithm:
/// 1. Iterate messages in order
/// 2. Classify each message
/// 3. HardNoise -> skip
/// 4. User -> create UserChunk
/// 5. System -> create SystemChunk
/// 6. Compact -> create CompactChunk
/// 7. AI -> accumulate into current AIChunk; start new AIChunk on first AI after non-AI
/// 8. Link processes to AIChunks by matching parentTaskId to tool calls
pub fn build_chunks(messages: &[ParsedMessage], processes: &[Process]) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut current_ai_responses: Vec<ParsedMessage> = Vec::new();
    let mut current_ai_sidechain: Vec<ParsedMessage> = Vec::new();
    let mut chunk_counter: u32 = 0;

    // Pre-compute tool executions for linking
    let tool_executions = tool_linker::link_tool_calls(messages);

    for msg in messages {
        let category = classify_message(msg);

        match category {
            MessageCategory::HardNoise => continue,
            MessageCategory::User => {
                // Flush any accumulated AI messages
                if !current_ai_responses.is_empty() {
                    chunk_counter += 1;
                    let ai_chunk = build_ai_chunk(
                        chunk_counter,
                        &current_ai_responses,
                        &current_ai_sidechain,
                        processes,
                        &tool_executions,
                    );
                    chunks.push(ai_chunk);
                    current_ai_responses.clear();
                    current_ai_sidechain.clear();
                }

                chunk_counter += 1;
                chunks.push(build_user_chunk(chunk_counter, msg));
            }
            MessageCategory::System => {
                // Flush any accumulated AI messages
                if !current_ai_responses.is_empty() {
                    chunk_counter += 1;
                    let ai_chunk = build_ai_chunk(
                        chunk_counter,
                        &current_ai_responses,
                        &current_ai_sidechain,
                        processes,
                        &tool_executions,
                    );
                    chunks.push(ai_chunk);
                    current_ai_responses.clear();
                    current_ai_sidechain.clear();
                }

                chunk_counter += 1;
                chunks.push(build_system_chunk(chunk_counter, msg));
            }
            MessageCategory::Compact => {
                // Flush any accumulated AI messages
                if !current_ai_responses.is_empty() {
                    chunk_counter += 1;
                    let ai_chunk = build_ai_chunk(
                        chunk_counter,
                        &current_ai_responses,
                        &current_ai_sidechain,
                        processes,
                        &tool_executions,
                    );
                    chunks.push(ai_chunk);
                    current_ai_responses.clear();
                    current_ai_sidechain.clear();
                }

                chunk_counter += 1;
                chunks.push(build_compact_chunk(chunk_counter, msg));
            }
            MessageCategory::Ai => {
                if msg.is_sidechain {
                    current_ai_sidechain.push(msg.clone());
                } else {
                    current_ai_responses.push(msg.clone());
                }
            }
        }
    }

    // Flush remaining AI messages
    if !current_ai_responses.is_empty() {
        chunk_counter += 1;
        let ai_chunk = build_ai_chunk(
            chunk_counter,
            &current_ai_responses,
            &current_ai_sidechain,
            processes,
            &tool_executions,
        );
        chunks.push(ai_chunk);
    }

    // Post-processing: link team-based processes (no parentTaskId) to the last
    // AI chunk that ended before each process started.
    let unlinked: Vec<&Process> = processes
        .iter()
        .filter(|p| p.parent_task_id.is_none())
        .collect();

    if !unlinked.is_empty() {
        // Collect (start_ms, end_ms, index) for all AI chunks
        let ai_chunk_ranges: Vec<(i64, i64, usize)> = chunks
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                if let Chunk::Ai(data) = c {
                    Some((
                        parse_ts_ms(&data.base.start_time),
                        parse_ts_ms(&data.base.end_time),
                        i,
                    ))
                } else {
                    None
                }
            })
            .collect();

        for process in unlinked {
            let p_start = parse_ts_ms(&process.start_time);

            // Prefer the AI chunk whose time range contains the process start
            // (concurrent agents that run during an AI turn).
            let containing = ai_chunk_ranges
                .iter()
                .find(|(s, e, _)| p_start >= *s && p_start <= *e);

            // Fall back to the last AI chunk that ended before the process started
            // (agents spawned right after a turn ends).
            let preceding = ai_chunk_ranges
                .iter()
                .filter(|(_, e, _)| *e <= p_start)
                .max_by_key(|(_, e, _)| *e);

            let best = containing.or(preceding);

            if let Some((_, _, chunk_idx)) = best {
                if let Chunk::Ai(data) = &mut chunks[*chunk_idx] {
                    data.processes.push(process.clone());
                }
            }
        }
    }

    chunks
}

/// Enhance chunks by adding rawMessages, semanticSteps, and semanticStepGroups.
pub fn enhance_chunks(chunks: Vec<Chunk>, messages: &[ParsedMessage]) -> Vec<EnhancedChunk> {
    chunks
        .into_iter()
        .map(|chunk| enhance_single_chunk(chunk, messages))
        .collect()
}

fn enhance_single_chunk(chunk: Chunk, messages: &[ParsedMessage]) -> EnhancedChunk {
    match chunk {
        Chunk::User(data) => EnhancedChunk::User(EnhancedUserChunkData {
            base: data.base,
            user_message: data.user_message,
            enhanced: true,
        }),
        Chunk::Ai(data) => {
            let semantic_steps = semantic_extractor::extract_semantic_steps(&data, messages);
            let semantic_step_groups = if semantic_steps.is_empty() {
                None
            } else {
                Some(semantic_extractor::group_semantic_steps(
                    semantic_steps.clone(),
                    messages,
                ))
            };

            EnhancedChunk::Ai(EnhancedAiChunkData {
                base: data.base,
                responses: data.responses,
                processes: data.processes,
                sidechain_messages: data.sidechain_messages,
                tool_executions: data.tool_executions,
                semantic_steps,
                semantic_step_groups,
                enhanced: true,
            })
        }
        Chunk::System(data) => EnhancedChunk::System(EnhancedSystemChunkData {
            base: data.base,
            message: data.message,
            command_output: data.command_output,
            enhanced: true,
        }),
        Chunk::Compact(data) => EnhancedChunk::Compact(EnhancedCompactChunkData {
            base: data.base,
            message: data.message,
            enhanced: true,
        }),
    }
}

fn build_user_chunk(chunk_id: u32, msg: &ParsedMessage) -> Chunk {
    Chunk::User(UserChunkData {
        base: BaseChunkFields {
            id: format!("chunk-{chunk_id}"),
            start_time: msg.timestamp.clone(),
            end_time: msg.timestamp.clone(),
            duration_ms: 0.0,
            metrics: SessionMetrics::default(),
        },
        user_message: msg.clone(),
    })
}

fn build_system_chunk(chunk_id: u32, msg: &ParsedMessage) -> Chunk {
    let command_output = extract_command_output(msg);

    Chunk::System(SystemChunkData {
        base: BaseChunkFields {
            id: format!("chunk-{chunk_id}"),
            start_time: msg.timestamp.clone(),
            end_time: msg.timestamp.clone(),
            duration_ms: 0.0,
            metrics: SessionMetrics::default(),
        },
        message: msg.clone(),
        command_output,
    })
}

fn build_compact_chunk(chunk_id: u32, msg: &ParsedMessage) -> Chunk {
    Chunk::Compact(CompactChunkData {
        base: BaseChunkFields {
            id: format!("chunk-{chunk_id}"),
            start_time: msg.timestamp.clone(),
            end_time: msg.timestamp.clone(),
            duration_ms: 0.0,
            metrics: SessionMetrics::default(),
        },
        message: msg.clone(),
    })
}

fn build_ai_chunk(
    chunk_id: u32,
    responses: &[ParsedMessage],
    sidechain: &[ParsedMessage],
    processes: &[Process],
    all_tool_executions: &[ToolExecution],
) -> Chunk {
    let start_time = responses
        .first()
        .map(|m| m.timestamp.clone())
        .unwrap_or_default();
    let end_time = responses
        .last()
        .map(|m| m.timestamp.clone())
        .unwrap_or_default();
    let duration_ms = compute_duration_ms(&start_time, &end_time);

    // Compute metrics for this chunk
    let metrics = compute_chunk_metrics(responses);

    // Collect tool call IDs from all responses in this chunk
    let chunk_tool_ids: Vec<&str> = responses
        .iter()
        .flat_map(|r| r.tool_calls.iter().map(|tc| tc.id.as_str()))
        .collect();

    // Link processes that have an explicit parentTaskId matching a tool call in this chunk.
    // Processes without parentTaskId (team-based agents) are linked in a post-processing
    // pass in build_chunks() after all chunks are assembled.
    let linked_processes: Vec<Process> = processes
        .iter()
        .filter(|p| {
            p.parent_task_id
                .as_deref()
                .map(|id| chunk_tool_ids.contains(&id))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    // Filter tool executions that belong to this chunk
    let chunk_tool_executions: Vec<ToolExecution> = all_tool_executions
        .iter()
        .filter(|te| chunk_tool_ids.contains(&te.tool_call.id.as_str()))
        .cloned()
        .collect();

    Chunk::Ai(AiChunkData {
        base: BaseChunkFields {
            id: format!("chunk-{chunk_id}"),
            start_time,
            end_time,
            duration_ms,
            metrics,
        },
        responses: responses.to_vec(),
        processes: linked_processes,
        sidechain_messages: sidechain.to_vec(),
        tool_executions: chunk_tool_executions,
    })
}

/// Extract command output text from a system message.
fn extract_command_output(msg: &ParsedMessage) -> String {
    match &msg.content {
        crate::models::jsonl::StringOrBlocks::String(s) => {
            // Strip the XML tags to get raw output
            let stdout_tag = crate::models::constants::LOCAL_COMMAND_STDOUT_TAG;
            let stderr_tag = crate::models::constants::LOCAL_COMMAND_STDERR_TAG;

            if let Some(inner) = s.strip_prefix(stdout_tag) {
                let close_tag = "</local-command-stdout>";
                inner
                    .strip_suffix(close_tag)
                    .unwrap_or(inner)
                    .to_string()
            } else if let Some(inner) = s.strip_prefix(stderr_tag) {
                let close_tag = "</local-command-stderr>";
                inner
                    .strip_suffix(close_tag)
                    .unwrap_or(inner)
                    .to_string()
            } else {
                s.clone()
            }
        }
        crate::models::jsonl::StringOrBlocks::Blocks(blocks) => {
            for block in blocks {
                if let Some(text) = block.as_text() {
                    return text.to_string();
                }
            }
            String::new()
        }
    }
}

fn compute_chunk_metrics(responses: &[ParsedMessage]) -> SessionMetrics {
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_creation_tokens: u64 = 0;

    for msg in responses {
        if let Some(usage) = &msg.usage {
            input_tokens += usage.input_tokens;
            output_tokens += usage.output_tokens;
            cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
            cache_creation_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
        }
    }

    let total_tokens = input_tokens + output_tokens;
    let start_time = responses.first().map(|m| m.timestamp.as_str());
    let end_time = responses.last().map(|m| m.timestamp.as_str());

    let duration_ms = match (start_time, end_time) {
        (Some(s), Some(e)) => compute_duration_ms(s, e),
        _ => 0.0,
    };

    SessionMetrics {
        duration_ms,
        total_tokens,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        message_count: responses.len() as u32,
        cost_usd: None,
    }
}

/// Parse an ISO 8601 timestamp string into milliseconds since epoch.
/// Returns i64::MIN on parse failure so range checks fail safely.
fn parse_ts_ms(ts: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(i64::MIN)
}

fn compute_duration_ms(start: &str, end: &str) -> f64 {
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
    use crate::models::domain::{MessageType, TokenUsage};
    use crate::models::jsonl::StringOrBlocks;
    use crate::models::messages::ParsedMessage;

    fn make_user_msg(uuid: &str, ts: &str) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::User,
            timestamp: ts.to_string(),
            role: Some("user".to_string()),
            content: StringOrBlocks::String("Hello".to_string()),
            usage: None,
            model: None,
            cwd: Some("/tmp".to_string()),
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: Some("external".to_string()),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
        }
    }

    fn make_ai_msg(uuid: &str, ts: &str) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::Assistant,
            timestamp: ts.to_string(),
            role: Some("assistant".to_string()),
            content: StringOrBlocks::Blocks(vec![]),
            usage: Some(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            }),
            model: Some("claude-sonnet".to_string()),
            cwd: Some("/tmp".to_string()),
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: Some("external".to_string()),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
        }
    }

    fn make_system_msg(uuid: &str, ts: &str, is_meta: bool) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::System,
            timestamp: ts.to_string(),
            role: None,
            content: StringOrBlocks::String("system info".to_string()),
            usage: None,
            model: None,
            cwd: Some("/tmp".to_string()),
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta,
            user_type: Some("external".to_string()),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
        }
    }

    fn make_compact_msg(uuid: &str, ts: &str) -> ParsedMessage {
        // Compact messages use User type with is_compact_summary=true.
        // Summary type is always classified as HardNoise, so we use User
        // to exercise the Compact classification path in chunk_builder.
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::User,
            timestamp: ts.to_string(),
            role: Some("user".to_string()),
            content: StringOrBlocks::String("compacted".to_string()),
            usage: None,
            model: None,
            cwd: Some("/tmp".to_string()),
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: Some("external".to_string()),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: Some(true),
        }
    }

    #[test]
    fn test_build_chunks_user_ai_user() {
        let messages = vec![
            make_user_msg("u1", "2025-01-01T00:00:00Z"),
            make_ai_msg("a1", "2025-01-01T00:00:01Z"),
            make_ai_msg("a2", "2025-01-01T00:00:02Z"),
            make_user_msg("u2", "2025-01-01T00:00:03Z"),
            make_ai_msg("a3", "2025-01-01T00:00:04Z"),
        ];

        let chunks = build_chunks(&messages, &[]);

        // Expected: User(u1), Ai(a1+a2), User(u2), Ai(a3)
        assert_eq!(chunks.len(), 4);
        assert!(matches!(&chunks[0], Chunk::User(_)));
        assert!(matches!(&chunks[1], Chunk::Ai(_)));
        assert!(matches!(&chunks[2], Chunk::User(_)));
        assert!(matches!(&chunks[3], Chunk::Ai(_)));

        // Verify the AI chunk contains both responses
        if let Chunk::Ai(data) = &chunks[1] {
            assert_eq!(data.responses.len(), 2);
            assert_eq!(data.responses[0].uuid, "a1");
            assert_eq!(data.responses[1].uuid, "a2");
        }

        // Second AI chunk has one response
        if let Chunk::Ai(data) = &chunks[3] {
            assert_eq!(data.responses.len(), 1);
            assert_eq!(data.responses[0].uuid, "a3");
        }
    }

    #[test]
    fn test_build_chunks_empty() {
        let chunks = build_chunks(&[], &[]);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_build_chunks_only_ai() {
        let messages = vec![
            make_ai_msg("a1", "2025-01-01T00:00:00Z"),
            make_ai_msg("a2", "2025-01-01T00:00:01Z"),
        ];

        let chunks = build_chunks(&messages, &[]);
        // All AI messages should be grouped into a single AI chunk
        assert_eq!(chunks.len(), 1);
        assert!(matches!(&chunks[0], Chunk::Ai(_)));
        if let Chunk::Ai(data) = &chunks[0] {
            assert_eq!(data.responses.len(), 2);
        }
    }

    #[test]
    fn test_build_chunks_compact_message() {
        let messages = vec![
            make_user_msg("u1", "2025-01-01T00:00:00Z"),
            make_ai_msg("a1", "2025-01-01T00:00:01Z"),
            make_compact_msg("c1", "2025-01-01T00:00:02Z"),
            make_user_msg("u2", "2025-01-01T00:00:03Z"),
        ];

        let chunks = build_chunks(&messages, &[]);
        // Expected: User(u1), Ai(a1), Compact(c1), User(u2)
        assert_eq!(chunks.len(), 4);
        assert!(matches!(&chunks[0], Chunk::User(_)));
        assert!(matches!(&chunks[1], Chunk::Ai(_)));
        assert!(matches!(&chunks[2], Chunk::Compact(_)));
        assert!(matches!(&chunks[3], Chunk::User(_)));
    }

    #[test]
    fn test_build_chunks_system_message() {
        let messages = vec![
            make_user_msg("u1", "2025-01-01T00:00:00Z"),
            make_system_msg("s1", "2025-01-01T00:00:01Z", true),
            make_ai_msg("a1", "2025-01-01T00:00:02Z"),
        ];

        let chunks = build_chunks(&messages, &[]);
        // System messages classified as HardNoise are skipped.
        // is_meta=true system is HardNoise because MessageType::System is always HardNoise
        // in the classifier.
        // The result depends on the classifier: System type -> HardNoise
        // So: User(u1), Ai(a1) — the system msg is skipped
        assert_eq!(chunks.len(), 2);
        assert!(matches!(&chunks[0], Chunk::User(_)));
        assert!(matches!(&chunks[1], Chunk::Ai(_)));
    }

    #[test]
    fn test_build_chunks_ai_metrics() {
        let messages = vec![
            make_ai_msg("a1", "2025-01-01T00:00:00Z"),
            make_ai_msg("a2", "2025-01-01T00:00:01Z"),
        ];

        let chunks = build_chunks(&messages, &[]);
        if let Chunk::Ai(data) = &chunks[0] {
            assert_eq!(data.base.metrics.input_tokens, 200); // 100 * 2
            assert_eq!(data.base.metrics.output_tokens, 100); // 50 * 2
            assert_eq!(data.base.metrics.total_tokens, 300);
            assert_eq!(data.base.metrics.message_count, 2);
        } else {
            panic!("Expected Ai chunk");
        }
    }

    #[test]
    fn test_enhance_chunks_sets_enhanced_marker() {
        let messages = vec![
            make_user_msg("u1", "2025-01-01T00:00:00Z"),
            make_ai_msg("a1", "2025-01-01T00:00:01Z"),
        ];

        let chunks = build_chunks(&messages, &[]);
        let enhanced = enhance_chunks(chunks, &messages);

        assert_eq!(enhanced.len(), 2);
        if let EnhancedChunk::User(data) = &enhanced[0] {
            assert!(data.enhanced);
            assert_eq!(data.user_message.uuid, "u1");
        }
        if let EnhancedChunk::Ai(data) = &enhanced[1] {
            assert!(data.enhanced);
            assert_eq!(data.responses[0].uuid, "a1");
        }
    }
}
