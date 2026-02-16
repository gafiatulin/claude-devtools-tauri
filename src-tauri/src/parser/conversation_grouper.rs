use crate::models::chunks::{ConversationGroup, Process, TaskExecution, ToolExecution};
use crate::models::domain::SessionMetrics;
use crate::models::messages::ParsedMessage;

use super::message_classifier::classify_message;
use super::tool_linker;

/// Build conversation groups from messages and processes.
///
/// Groups by real user message boundaries (not system/meta messages).
/// Each group contains: user message + all AI responses until next user message.
/// Aggregates metrics per group and links task executions.
pub fn build_conversation_groups(
    messages: &[ParsedMessage],
    processes: &[Process],
) -> Vec<ConversationGroup> {
    let mut groups: Vec<ConversationGroup> = Vec::new();
    let mut group_counter: u32 = 0;

    let tool_executions = tool_linker::link_tool_calls(messages);
    let task_executions = tool_linker::build_task_executions(messages, processes);

    let mut current_user_msg: Option<ParsedMessage> = None;
    let mut current_ai_responses: Vec<ParsedMessage> = Vec::new();

    for msg in messages {
        let category = classify_message(msg);

        match category {
            crate::models::domain::MessageCategory::HardNoise => continue,
            crate::models::domain::MessageCategory::User => {
                // Flush previous group
                if let Some(user_msg) = current_user_msg.take() {
                    group_counter += 1;
                    let group = build_group(
                        group_counter,
                        user_msg,
                        std::mem::take(&mut current_ai_responses),
                        processes,
                        &tool_executions,
                        &task_executions,
                    );
                    groups.push(group);
                }
                current_user_msg = Some(msg.clone());
            }
            crate::models::domain::MessageCategory::Ai => {
                current_ai_responses.push(msg.clone());
            }
            _ => {
                // System and Compact messages are included in AI responses for grouping
                current_ai_responses.push(msg.clone());
            }
        }
    }

    // Flush last group
    if let Some(user_msg) = current_user_msg {
        group_counter += 1;
        let group = build_group(
            group_counter,
            user_msg,
            current_ai_responses,
            processes,
            &tool_executions,
            &task_executions,
        );
        groups.push(group);
    }

    groups
}

fn build_group(
    group_id: u32,
    user_message: ParsedMessage,
    ai_responses: Vec<ParsedMessage>,
    processes: &[Process],
    all_tool_executions: &[ToolExecution],
    all_task_executions: &[TaskExecution],
) -> ConversationGroup {
    let start_time = user_message.timestamp.clone();
    let end_time = ai_responses
        .last()
        .map(|m| m.timestamp.clone())
        .unwrap_or_else(|| user_message.timestamp.clone());

    let duration_ms = compute_duration_ms(&start_time, &end_time);

    // Collect tool call IDs from this group's AI responses
    let group_tool_ids: Vec<&str> = ai_responses
        .iter()
        .flat_map(|r| r.tool_calls.iter().map(|tc| tc.id.as_str()))
        .collect();

    // Filter tool executions for this group
    let group_tool_executions: Vec<ToolExecution> = all_tool_executions
        .iter()
        .filter(|te| group_tool_ids.contains(&te.tool_call.id.as_str()))
        .cloned()
        .collect();

    // Filter task executions for this group
    let group_task_executions: Vec<TaskExecution> = all_task_executions
        .iter()
        .filter(|te| group_tool_ids.contains(&te.task_call.id.as_str()))
        .cloned()
        .collect();

    // Link processes to this group
    let linked_processes: Vec<Process> = processes
        .iter()
        .filter(|p| {
            if let Some(parent_id) = &p.parent_task_id {
                group_tool_ids.contains(&parent_id.as_str())
            } else {
                false
            }
        })
        .cloned()
        .collect();

    // Compute metrics
    let metrics = compute_group_metrics(&ai_responses);

    ConversationGroup {
        id: format!("group-{group_id}"),
        group_type: "user-ai-exchange".to_string(),
        user_message,
        ai_responses,
        processes: linked_processes,
        tool_executions: group_tool_executions,
        task_executions: group_task_executions,
        start_time,
        end_time,
        duration_ms,
        metrics,
    }
}

fn compute_group_metrics(responses: &[ParsedMessage]) -> SessionMetrics {
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

    let start = responses.first().map(|m| m.timestamp.as_str());
    let end = responses.last().map(|m| m.timestamp.as_str());
    let duration_ms = match (start, end) {
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
