use std::collections::HashMap;

use crate::models::chunks::{Process, TaskExecution, ToolExecution, ToolProgress};
use crate::models::domain::MessageType;
use crate::models::messages::{ParsedMessage, ToolCall, ToolResult};

/// Link tool calls to their results by matching IDs.
///
/// Matches by:
/// 1. `tool_use.id` <-> `tool_result.tool_use_id` (in content blocks)
/// 2. `sourceToolUseID` field on user entries
///
/// Computes timing: startTime (tool call message time), endTime (tool result message time), durationMs.
pub fn link_tool_calls(
    messages: &[ParsedMessage],
    progress_map: &HashMap<String, ToolProgress>,
) -> Vec<ToolExecution> {
    // Build a map of tool call ID -> (ToolCall, timestamp)
    let mut tool_call_map: HashMap<String, (ToolCall, String)> = HashMap::new();

    for msg in messages {
        if msg.message_type == MessageType::Assistant {
            for tc in &msg.tool_calls {
                tool_call_map
                    .insert(tc.id.clone(), (tc.clone(), msg.timestamp.clone()));
            }
        }
    }

    // Build tool executions by finding matching results
    let mut executions: Vec<ToolExecution> = Vec::new();
    let mut matched_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for msg in messages {
        if msg.message_type != MessageType::User {
            continue;
        }

        // Match tool results from content blocks
        for tr in &msg.tool_results {
            if let Some((tool_call, call_time)) = tool_call_map.get(&tr.tool_use_id) {
                let duration_ms = compute_duration_ms(call_time, &msg.timestamp);
                executions.push(ToolExecution {
                    tool_call: tool_call.clone(),
                    result: Some(tr.clone()),
                    start_time: call_time.clone(),
                    end_time: Some(msg.timestamp.clone()),
                    duration_ms: Some(duration_ms),
                    progress: None,
                });
                matched_ids.insert(tr.tool_use_id.clone());
            }
        }

        // Match by sourceToolUseID field
        if let Some(source_id) = &msg.source_tool_use_id {
            if !matched_ids.contains(source_id) {
                if let Some((tool_call, call_time)) = tool_call_map.get(source_id) {
                    let duration_ms = compute_duration_ms(call_time, &msg.timestamp);

                    // Build a synthetic tool result from message content
                    let result_content = match &msg.content {
                        crate::models::jsonl::StringOrBlocks::String(s) => {
                            serde_json::Value::String(s.clone())
                        }
                        crate::models::jsonl::StringOrBlocks::Blocks(blocks) => {
                            serde_json::to_value(blocks).unwrap_or_default()
                        }
                    };

                    executions.push(ToolExecution {
                        tool_call: tool_call.clone(),
                        result: Some(ToolResult {
                            tool_use_id: source_id.clone(),
                            content: result_content,
                            is_error: false,
                        }),
                        start_time: call_time.clone(),
                        end_time: Some(msg.timestamp.clone()),
                        duration_ms: Some(duration_ms),
                        progress: None,
                    });
                    matched_ids.insert(source_id.clone());
                }
            }
        }
    }

    // Add unmatched tool calls (no result found yet) — attach progress if available
    for (id, (tool_call, call_time)) in &tool_call_map {
        if !matched_ids.contains(id) {
            executions.push(ToolExecution {
                tool_call: tool_call.clone(),
                result: None,
                start_time: call_time.clone(),
                end_time: None,
                duration_ms: None,
                progress: progress_map.get(id).cloned(),
            });
        }
    }

    // Sort by start time
    executions.sort_by(|a, b| a.start_time.cmp(&b.start_time));

    executions
}

/// Build task executions by matching Task tool calls to their subagent processes.
pub fn build_task_executions(
    messages: &[ParsedMessage],
    processes: &[Process],
) -> Vec<TaskExecution> {
    let mut task_executions: Vec<TaskExecution> = Vec::new();

    // Build a map of parentTaskId -> Process
    let mut process_map: HashMap<String, &Process> = HashMap::new();
    for process in processes {
        if let Some(parent_id) = &process.parent_task_id {
            process_map.insert(parent_id.clone(), process);
        }
    }

    // Find Task tool calls in assistant messages
    for msg in messages {
        if msg.message_type != MessageType::Assistant {
            continue;
        }

        for tc in &msg.tool_calls {
            if !tc.is_task {
                continue;
            }

            // Find matching process
            let process = match process_map.get(&tc.id) {
                Some(p) => (*p).clone(),
                None => continue,
            };

            // Find the tool result for this task call
            let tool_result = find_tool_result_message(messages, &tc.id);

            if let Some(result_msg) = tool_result {
                let duration_ms = compute_duration_ms(&msg.timestamp, &result_msg.timestamp);

                task_executions.push(TaskExecution {
                    task_call: tc.clone(),
                    task_call_timestamp: msg.timestamp.clone(),
                    subagent: process,
                    tool_result: result_msg.clone(),
                    result_timestamp: result_msg.timestamp.clone(),
                    duration_ms,
                });
            }
        }
    }

    // Sort by task call timestamp
    task_executions.sort_by(|a, b| a.task_call_timestamp.cmp(&b.task_call_timestamp));

    task_executions
}

/// Find the user message containing the tool result for a given tool_use_id.
fn find_tool_result_message<'a>(
    messages: &'a [ParsedMessage],
    tool_use_id: &str,
) -> Option<&'a ParsedMessage> {
    for msg in messages {
        if msg.message_type != MessageType::User {
            continue;
        }

        // Check tool_results
        for tr in &msg.tool_results {
            if tr.tool_use_id == tool_use_id {
                return Some(msg);
            }
        }

        // Check sourceToolUseID
        if let Some(source_id) = &msg.source_tool_use_id {
            if source_id == tool_use_id {
                return Some(msg);
            }
        }
    }

    None
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
