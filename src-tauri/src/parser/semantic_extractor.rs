use crate::models::chunks::{
    AiChunkData, SemanticStep, SemanticStepContent, SemanticStepGroup, SemanticStepType,
    StepTokenBreakdown, StepTokens,
};
use crate::models::domain::MessageType;
use crate::models::jsonl::ContentBlock;
use crate::models::messages::ParsedMessage;

/// Extract semantic steps from an AI chunk.
///
/// For each assistant message in the chunk:
/// - Thinking content blocks -> Thinking steps
/// - ToolUse content blocks -> ToolCall steps
/// - Text content blocks -> Output steps
///
/// For each internal user message (tool results):
/// - ToolResult content blocks -> ToolResult steps
///
/// Interrupted messages produce Interruption steps.
pub fn extract_semantic_steps(
    chunk: &AiChunkData,
    messages: &[ParsedMessage],
) -> Vec<SemanticStep> {
    let mut steps: Vec<SemanticStep> = Vec::new();
    let mut step_counter: u32 = 0;
    let mut accumulated_context: u64 = 0;

    for response in &chunk.responses {
        match response.message_type {
            MessageType::Assistant => {
                let msg_steps = extract_steps_from_assistant(
                    response,
                    &mut step_counter,
                    &mut accumulated_context,
                );
                steps.extend(msg_steps);
            }
            MessageType::User if response.is_meta => {
                // Internal user messages contain tool results
                let result_steps =
                    extract_steps_from_tool_results(response, &mut step_counter, messages);
                steps.extend(result_steps);
            }
            _ => {}
        }
    }

    // Apply gap-filling logic for timing
    apply_gap_filling(&mut steps);

    // Inject synthetic Subagent steps for team-based processes that have no
    // corresponding Task tool call in the semantic steps (i.e. their parentTaskId
    // is None so they were linked by timestamp rather than tool call ID).
    let matched_ids: std::collections::HashSet<String> = steps
        .iter()
        .filter_map(|s| s.content.subagent_id.clone())
        .collect();

    for process in &chunk.processes {
        if process.parent_task_id.is_none() && !matched_ids.contains(&process.id) {
            steps.push(SemanticStep {
                id: format!("step-agent-{}", process.id),
                step_type: SemanticStepType::Subagent,
                start_time: process.start_time.clone(),
                end_time: Some(process.end_time.clone()),
                duration_ms: process.duration_ms,
                content: SemanticStepContent {
                    thinking_text: None,
                    tool_name: None,
                    tool_input: None,
                    tool_result_content: None,
                    is_error: None,
                    tool_use_result: None,
                    token_count: None,
                    subagent_id: Some(process.id.clone()),
                    subagent_description: process.description.clone(),
                    output_text: None,
                    source_model: None,
                    interruption_text: None,
                },
                tokens: None,
                is_parallel: Some(process.is_parallel),
                group_id: None,
                context: "main".to_string(),
                agent_id: None,
                source_message_id: None,
                effective_end_time: None,
                effective_duration_ms: None,
                is_gap_filled: None,
                context_tokens: None,
                accumulated_context: None,
                token_breakdown: None,
            });
        }
    }

    steps
}

/// Group semantic steps by their source assistant message.
pub fn group_semantic_steps(
    steps: Vec<SemanticStep>,
    _messages: &[ParsedMessage],
) -> Vec<SemanticStepGroup> {
    if steps.is_empty() {
        return Vec::new();
    }

    let mut groups: Vec<SemanticStepGroup> = Vec::new();
    let mut current_group_steps: Vec<SemanticStep> = Vec::new();
    let mut current_source_id: Option<String> = None;
    let mut group_counter: u32 = 0;

    for step in steps {
        let step_source = step.source_message_id.clone();

        if current_source_id.as_ref() != step_source.as_ref() && !current_group_steps.is_empty() {
            // Flush current group
            group_counter += 1;
            groups.push(build_step_group(
                group_counter,
                std::mem::take(&mut current_group_steps),
                &current_source_id,
            ));
        }

        current_source_id = step_source;
        current_group_steps.push(step);
    }

    // Flush last group
    if !current_group_steps.is_empty() {
        group_counter += 1;
        groups.push(build_step_group(
            group_counter,
            current_group_steps,
            &current_source_id,
        ));
    }

    groups
}

fn build_step_group(
    group_id: u32,
    steps: Vec<SemanticStep>,
    source_message_id: &Option<String>,
) -> SemanticStepGroup {
    let start_time = steps
        .first()
        .map(|s| s.start_time.clone())
        .unwrap_or_default();
    let end_time = steps
        .last()
        .map(|s| {
            s.effective_end_time
                .clone()
                .or_else(|| s.end_time.clone())
                .unwrap_or_else(|| s.start_time.clone())
        })
        .unwrap_or_default();

    let total_duration: f64 = steps.iter().map(|s| s.duration_ms).sum();

    let label = build_group_label(&steps);

    SemanticStepGroup {
        id: format!("group-{group_id}"),
        label,
        steps,
        is_grouped: true,
        source_message_id: source_message_id.clone(),
        start_time,
        end_time,
        total_duration,
    }
}

fn build_group_label(steps: &[SemanticStep]) -> String {
    let tool_calls: Vec<&str> = steps
        .iter()
        .filter(|s| s.step_type == SemanticStepType::ToolCall)
        .filter_map(|s| s.content.tool_name.as_deref())
        .collect();

    if !tool_calls.is_empty() {
        if tool_calls.len() == 1 {
            format!("Called {}", tool_calls[0])
        } else {
            format!("Called {} tools", tool_calls.len())
        }
    } else {
        let has_thinking = steps
            .iter()
            .any(|s| s.step_type == SemanticStepType::Thinking);
        let has_output = steps
            .iter()
            .any(|s| s.step_type == SemanticStepType::Output);

        if has_thinking && has_output {
            "Thinking & Output".to_string()
        } else if has_thinking {
            "Thinking".to_string()
        } else if has_output {
            "Output".to_string()
        } else {
            "Response".to_string()
        }
    }
}

fn extract_steps_from_assistant(
    msg: &ParsedMessage,
    counter: &mut u32,
    accumulated_context: &mut u64,
) -> Vec<SemanticStep> {
    let mut steps = Vec::new();

    // Track token usage for attribution
    let usage = msg.usage.as_ref();
    let total_input = usage.map(|u| u.input_tokens).unwrap_or(0);
    let total_output = usage.map(|u| u.output_tokens).unwrap_or(0);
    let cache_read = usage.and_then(|u| u.cache_read_input_tokens).unwrap_or(0);
    let cache_creation = usage
        .and_then(|u| u.cache_creation_input_tokens)
        .unwrap_or(0);

    *accumulated_context += total_input + cache_read;

    // Count content blocks for token distribution
    let blocks = match &msg.content {
        crate::models::jsonl::StringOrBlocks::Blocks(b) => b.as_slice(),
        crate::models::jsonl::StringOrBlocks::String(_) => &[],
    };

    let content_block_count = blocks.len().max(1) as u64;

    for block in blocks {
        *counter += 1;
        let step_id = format!("step-{counter}");

        match block {
            ContentBlock::Thinking { thinking, .. } => {
                let per_block_output = total_output / content_block_count;
                steps.push(SemanticStep {
                    id: step_id,
                    step_type: SemanticStepType::Thinking,
                    start_time: msg.timestamp.clone(),
                    end_time: None,
                    duration_ms: 0.0,
                    content: SemanticStepContent {
                        thinking_text: Some(thinking.clone()),
                        tool_name: None,
                        tool_input: None,
                        tool_result_content: None,
                        is_error: None,
                        tool_use_result: None,
                        token_count: None,
                        subagent_id: None,
                        subagent_description: None,
                        output_text: None,
                        source_model: msg.model.clone(),
                        interruption_text: None,
                    },
                    tokens: Some(StepTokens {
                        input: total_input / content_block_count,
                        output: per_block_output,
                        cached: Some(cache_read / content_block_count),
                    }),
                    is_parallel: None,
                    group_id: None,
                    context: "main".to_string(),
                    agent_id: msg.agent_id.clone(),
                    source_message_id: Some(msg.uuid.clone()),
                    effective_end_time: None,
                    effective_duration_ms: None,
                    is_gap_filled: None,
                    context_tokens: Some(total_input),
                    accumulated_context: Some(*accumulated_context),
                    token_breakdown: Some(StepTokenBreakdown {
                        input: total_input / content_block_count,
                        output: per_block_output,
                        cache_read: cache_read / content_block_count,
                        cache_creation: cache_creation / content_block_count,
                    }),
                });
            }
            ContentBlock::ToolUse { id, name, input } => {
                let is_task = name == "Task";
                let subagent_desc = if is_task {
                    input
                        .get("description")
                        .or_else(|| input.get("prompt"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                };

                let step_type = if is_task {
                    SemanticStepType::Subagent
                } else {
                    SemanticStepType::ToolCall
                };

                // Use the actual tool_use ID so the frontend can link calls to results
                steps.push(SemanticStep {
                    id: id.clone(),
                    step_type,
                    start_time: msg.timestamp.clone(),
                    end_time: None,
                    duration_ms: 0.0,
                    content: SemanticStepContent {
                        thinking_text: None,
                        tool_name: Some(name.clone()),
                        tool_input: Some(input.clone()),
                        tool_result_content: None,
                        is_error: None,
                        tool_use_result: None,
                        token_count: None,
                        subagent_id: if is_task { Some(id.clone()) } else { None },
                        subagent_description: subagent_desc,
                        output_text: None,
                        source_model: msg.model.clone(),
                        interruption_text: None,
                    },
                    tokens: Some(StepTokens {
                        input: total_input / content_block_count,
                        output: total_output / content_block_count,
                        cached: Some(cache_read / content_block_count),
                    }),
                    is_parallel: None,
                    group_id: None,
                    context: "main".to_string(),
                    agent_id: msg.agent_id.clone(),
                    source_message_id: Some(msg.uuid.clone()),
                    effective_end_time: None,
                    effective_duration_ms: None,
                    is_gap_filled: None,
                    context_tokens: None,
                    accumulated_context: Some(*accumulated_context),
                    token_breakdown: None,
                });
            }
            ContentBlock::Text { text } if !text.trim().is_empty() => {
                steps.push(SemanticStep {
                    id: step_id,
                    step_type: SemanticStepType::Output,
                    start_time: msg.timestamp.clone(),
                    end_time: None,
                    duration_ms: 0.0,
                    content: SemanticStepContent {
                        thinking_text: None,
                        tool_name: None,
                        tool_input: None,
                        tool_result_content: None,
                        is_error: None,
                        tool_use_result: None,
                        token_count: Some(text.len() as u64 / 4), // rough token estimate
                        subagent_id: None,
                        subagent_description: None,
                        output_text: Some(text.clone()),
                        source_model: msg.model.clone(),
                        interruption_text: None,
                    },
                    tokens: Some(StepTokens {
                        input: total_input / content_block_count,
                        output: total_output / content_block_count,
                        cached: Some(cache_read / content_block_count),
                    }),
                    is_parallel: None,
                    group_id: None,
                    context: "main".to_string(),
                    agent_id: msg.agent_id.clone(),
                    source_message_id: Some(msg.uuid.clone()),
                    effective_end_time: None,
                    effective_duration_ms: None,
                    is_gap_filled: None,
                    context_tokens: None,
                    accumulated_context: Some(*accumulated_context),
                    token_breakdown: None,
                });
            }
            _ => {} // Empty text, Image, ToolResult blocks in assistant message
        }
    }

    // Check for interruption - if stop_reason is not "end_turn" or content was truncated
    if let crate::models::jsonl::StringOrBlocks::String(s) = &msg.content {
        if s.contains("[Request interrupted by user") {
            *counter += 1;
            steps.push(SemanticStep {
                id: format!("step-{counter}"),
                step_type: SemanticStepType::Interruption,
                start_time: msg.timestamp.clone(),
                end_time: None,
                duration_ms: 0.0,
                content: SemanticStepContent {
                    thinking_text: None,
                    tool_name: None,
                    tool_input: None,
                    tool_result_content: None,
                    is_error: None,
                    tool_use_result: None,
                    token_count: None,
                    subagent_id: None,
                    subagent_description: None,
                    output_text: None,
                    source_model: msg.model.clone(),
                    interruption_text: Some(s.clone()),
                },
                tokens: None,
                is_parallel: None,
                group_id: None,
                context: "main".to_string(),
                agent_id: msg.agent_id.clone(),
                source_message_id: Some(msg.uuid.clone()),
                effective_end_time: None,
                effective_duration_ms: None,
                is_gap_filled: None,
                context_tokens: None,
                accumulated_context: None,
                token_breakdown: None,
            });
        }
    }

    steps
}

fn extract_steps_from_tool_results(
    msg: &ParsedMessage,
    counter: &mut u32,
    _messages: &[ParsedMessage],
) -> Vec<SemanticStep> {
    let mut steps = Vec::new();

    for tool_result in &msg.tool_results {
        *counter += 1;

        let result_text = match &tool_result.content {
            serde_json::Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        };

        // Use the tool_use_id so the frontend can link this result to its call
        steps.push(SemanticStep {
            id: tool_result.tool_use_id.clone(),
            step_type: SemanticStepType::ToolResult,
            start_time: msg.timestamp.clone(),
            end_time: None,
            duration_ms: 0.0,
            content: SemanticStepContent {
                thinking_text: None,
                tool_name: None,
                tool_input: None,
                tool_result_content: result_text,
                is_error: Some(tool_result.is_error),
                tool_use_result: msg.tool_use_result.clone(),
                token_count: None,
                subagent_id: None,
                subagent_description: None,
                output_text: None,
                source_model: None,
                interruption_text: None,
            },
            tokens: None,
            is_parallel: None,
            group_id: None,
            context: "main".to_string(),
            agent_id: msg.agent_id.clone(),
            source_message_id: Some(msg.uuid.clone()),
            effective_end_time: None,
            effective_duration_ms: None,
            is_gap_filled: None,
            context_tokens: None,
            accumulated_context: None,
            token_breakdown: None,
        });
    }

    steps
}

/// Apply gap-filling logic: when a step has no end time, use the next step's
/// start time. Mark gap-filled steps with isGapFilled: true.
fn apply_gap_filling(steps: &mut [SemanticStep]) {
    if steps.len() < 2 {
        return;
    }

    for i in 0..steps.len() - 1 {
        if steps[i].end_time.is_none() {
            let next_start = steps[i + 1].start_time.clone();
            let effective_duration = compute_duration_ms(&steps[i].start_time, &next_start);

            steps[i].effective_end_time = Some(next_start);
            steps[i].effective_duration_ms = Some(effective_duration);
            steps[i].is_gap_filled = Some(true);
        }
    }
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
    use crate::models::chunks::{AiChunkData, BaseChunkFields};
    use crate::models::domain::{MessageType, SessionMetrics, TokenUsage};
    use crate::models::jsonl::{ContentBlock, StringOrBlocks};
    use crate::models::messages::{ParsedMessage, ToolResult};

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

    fn make_ai_msg_with_blocks(uuid: &str, ts: &str, blocks: Vec<ContentBlock>) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::Assistant,
            timestamp: ts.to_string(),
            role: Some("assistant".to_string()),
            content: StringOrBlocks::Blocks(blocks),
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

    fn make_tool_result_msg(
        uuid: &str,
        ts: &str,
        tool_use_id: &str,
        content: &str,
    ) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.to_string(),
            parent_uuid: None,
            message_type: MessageType::User,
            timestamp: ts.to_string(),
            role: Some("user".to_string()),
            content: StringOrBlocks::String("".to_string()),
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: true,
            user_type: None,
            tool_calls: Vec::new(),
            tool_results: vec![ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: serde_json::Value::String(content.to_string()),
                is_error: false,
            }],
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
            plan_content: None,
        }
    }

    fn make_chunk(responses: Vec<ParsedMessage>) -> AiChunkData {
        AiChunkData {
            base: BaseChunkFields {
                id: "chunk-1".to_string(),
                start_time: responses
                    .first()
                    .map(|r| r.timestamp.clone())
                    .unwrap_or_default(),
                end_time: responses
                    .last()
                    .map(|r| r.timestamp.clone())
                    .unwrap_or_default(),
                duration_ms: 0.0,
                metrics: default_metrics(),
            },
            responses,
            processes: Vec::new(),
            sidechain_messages: Vec::new(),
            tool_executions: Vec::new(),
        }
    }

    #[test]
    fn test_extract_thinking_step() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::Thinking {
                thinking: "Let me think...".to_string(),
                signature: String::new(),
            }],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_type, SemanticStepType::Thinking);
        assert_eq!(
            steps[0].content.thinking_text.as_deref(),
            Some("Let me think...")
        );
    }

    #[test]
    fn test_extract_tool_call_step() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::ToolUse {
                id: "tu1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            }],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_type, SemanticStepType::ToolCall);
        assert_eq!(steps[0].content.tool_name.as_deref(), Some("Bash"));
        assert_eq!(steps[0].id, "tu1");
    }

    #[test]
    fn test_extract_text_output_step() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::Text {
                text: "Here is the answer.".to_string(),
            }],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_type, SemanticStepType::Output);
        assert_eq!(
            steps[0].content.output_text.as_deref(),
            Some("Here is the answer.")
        );
    }

    #[test]
    fn test_extract_empty_text_skipped() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::Text {
                text: "   ".to_string(),
            }],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert!(steps.is_empty());
    }

    #[test]
    fn test_extract_tool_result_step() {
        let ai_msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::ToolUse {
                id: "tu1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({}),
            }],
        );
        let result_msg = make_tool_result_msg("u1", "2025-01-01T00:00:01Z", "tu1", "file1.txt");

        let chunk = make_chunk(vec![ai_msg, result_msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].step_type, SemanticStepType::ToolCall);
        assert_eq!(steps[1].step_type, SemanticStepType::ToolResult);
        assert_eq!(
            steps[1].content.tool_result_content.as_deref(),
            Some("file1.txt")
        );
    }

    #[test]
    fn test_extract_task_as_subagent() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::ToolUse {
                id: "task1".to_string(),
                name: "Task".to_string(),
                input: serde_json::json!({"description": "Run tests"}),
            }],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_type, SemanticStepType::Subagent);
        assert_eq!(
            steps[0].content.subagent_description.as_deref(),
            Some("Run tests")
        );
    }

    #[test]
    fn test_extract_interruption_step() {
        let mut msg = make_ai_msg_with_blocks("a1", "2025-01-01T00:00:00Z", vec![]);
        msg.content = StringOrBlocks::String("[Request interrupted by user]".to_string());

        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_type, SemanticStepType::Interruption);
    }

    #[test]
    fn test_gap_filling() {
        let msg1 = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::Thinking {
                thinking: "hmm".to_string(),
                signature: String::new(),
            }],
        );
        let msg2 = make_ai_msg_with_blocks(
            "a2",
            "2025-01-01T00:00:05Z",
            vec![ContentBlock::Text {
                text: "answer".to_string(),
            }],
        );
        let chunk = make_chunk(vec![msg1, msg2]);
        let steps = extract_semantic_steps(&chunk, &[]);

        assert_eq!(steps.len(), 2);
        // First step should be gap-filled with second step's start time
        assert_eq!(steps[0].is_gap_filled, Some(true));
        assert_eq!(
            steps[0].effective_end_time.as_deref(),
            Some("2025-01-01T00:00:05Z")
        );
        assert_eq!(steps[0].effective_duration_ms, Some(5000.0));
    }

    #[test]
    fn test_group_semantic_steps_single_source() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![
                ContentBlock::Thinking {
                    thinking: "hmm".to_string(),
                    signature: String::new(),
                },
                ContentBlock::Text {
                    text: "answer".to_string(),
                },
            ],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);
        let groups = group_semantic_steps(steps, &[]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].steps.len(), 2);
        assert_eq!(groups[0].label, "Thinking & Output");
    }

    #[test]
    fn test_group_semantic_steps_tool_label() {
        let msg = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::ToolUse {
                id: "tu1".to_string(),
                name: "Read".to_string(),
                input: serde_json::json!({}),
            }],
        );
        let chunk = make_chunk(vec![msg]);
        let steps = extract_semantic_steps(&chunk, &[]);
        let groups = group_semantic_steps(steps, &[]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "Called Read");
    }

    #[test]
    fn test_group_semantic_steps_multiple_sources() {
        let msg1 = make_ai_msg_with_blocks(
            "a1",
            "2025-01-01T00:00:00Z",
            vec![ContentBlock::Text {
                text: "first".to_string(),
            }],
        );
        let msg2 = make_ai_msg_with_blocks(
            "a2",
            "2025-01-01T00:00:01Z",
            vec![ContentBlock::Text {
                text: "second".to_string(),
            }],
        );
        let chunk = make_chunk(vec![msg1, msg2]);
        let steps = extract_semantic_steps(&chunk, &[]);
        let groups = group_semantic_steps(steps, &[]);

        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_group_semantic_steps_empty() {
        let groups = group_semantic_steps(vec![], &[]);
        assert!(groups.is_empty());
    }
}
