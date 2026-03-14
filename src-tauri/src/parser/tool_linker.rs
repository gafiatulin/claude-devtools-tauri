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
                tool_call_map.insert(tc.id.clone(), (tc.clone(), msg.timestamp.clone()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domain::{MessageType, TokenUsage};
    use crate::models::jsonl::StringOrBlocks;
    use std::collections::HashMap;

    fn make_assistant_msg(uuid: &str, ts: &str, tool_calls: Vec<ToolCall>) -> ParsedMessage {
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
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls,
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
            plan_content: None,
        }
    }

    fn make_user_result_msg(uuid: &str, ts: &str, tool_results: Vec<ToolResult>) -> ParsedMessage {
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
            tool_results,
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            tool_use_result: None,
            is_compact_summary: None,
            plan_content: None,
        }
    }

    fn make_tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            input: serde_json::json!({"command": "ls"}),
            is_task: false,
            task_description: None,
            task_subagent_type: None,
        }
    }

    fn make_tool_result(tool_use_id: &str, content: &str) -> ToolResult {
        ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: serde_json::Value::String(content.to_string()),
            is_error: false,
        }
    }

    #[test]
    fn test_link_tool_calls_basic() {
        let tc = make_tool_call("tc1", "Bash");
        let tr = make_tool_result("tc1", "file1.txt\nfile2.txt");

        let messages = vec![
            make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc]),
            make_user_result_msg("u1", "2025-01-01T00:00:01Z", vec![tr]),
        ];

        let executions = link_tool_calls(&messages, &HashMap::new());
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].tool_call.id, "tc1");
        assert_eq!(executions[0].tool_call.name, "Bash");
        assert!(executions[0].result.is_some());
        assert_eq!(executions[0].duration_ms, Some(1000.0));
    }

    #[test]
    fn test_link_tool_calls_unmatched() {
        let tc = make_tool_call("tc1", "Read");
        let messages = vec![make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc])];

        let executions = link_tool_calls(&messages, &HashMap::new());
        assert_eq!(executions.len(), 1);
        assert!(executions[0].result.is_none());
        assert!(executions[0].end_time.is_none());
        assert!(executions[0].duration_ms.is_none());
    }

    #[test]
    fn test_link_tool_calls_with_progress() {
        let tc = make_tool_call("tc1", "Bash");
        let messages = vec![make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc])];

        let mut progress_map = HashMap::new();
        progress_map.insert(
            "tc1".to_string(),
            ToolProgress::Bash {
                full_output: "running...".to_string(),
                elapsed_time_seconds: 5.0,
                total_lines: 10,
                timeout_ms: 120000,
            },
        );

        let executions = link_tool_calls(&messages, &progress_map);
        assert_eq!(executions.len(), 1);
        assert!(executions[0].result.is_none());
        assert!(executions[0].progress.is_some());
    }

    #[test]
    fn test_link_tool_calls_multiple() {
        let tc1 = make_tool_call("tc1", "Bash");
        let tc2 = make_tool_call("tc2", "Read");
        let tr1 = make_tool_result("tc1", "output1");
        let tr2 = make_tool_result("tc2", "output2");

        let messages = vec![
            make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc1, tc2]),
            make_user_result_msg("u1", "2025-01-01T00:00:02Z", vec![tr1, tr2]),
        ];

        let executions = link_tool_calls(&messages, &HashMap::new());
        assert_eq!(executions.len(), 2);
        assert!(executions.iter().all(|e| e.result.is_some()));
    }

    #[test]
    fn test_link_tool_calls_sorted_by_start_time() {
        let tc1 = make_tool_call("tc1", "Bash");
        let tc2 = make_tool_call("tc2", "Read");
        let tr1 = make_tool_result("tc1", "output1");
        let tr2 = make_tool_result("tc2", "output2");

        let messages = vec![
            make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc1]),
            make_user_result_msg("u1", "2025-01-01T00:00:01Z", vec![tr1]),
            make_assistant_msg("a2", "2025-01-01T00:00:02Z", vec![tc2]),
            make_user_result_msg("u2", "2025-01-01T00:00:03Z", vec![tr2]),
        ];

        let executions = link_tool_calls(&messages, &HashMap::new());
        assert_eq!(executions.len(), 2);
        assert!(executions[0].start_time <= executions[1].start_time);
    }

    #[test]
    fn test_link_tool_calls_by_source_tool_use_id() {
        let tc = make_tool_call("tc1", "Skill");

        let mut result_msg = make_user_result_msg("u1", "2025-01-01T00:00:01Z", vec![]);
        result_msg.source_tool_use_id = Some("tc1".to_string());
        result_msg.content = StringOrBlocks::String("skill output".to_string());

        let messages = vec![
            make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc]),
            result_msg,
        ];

        let executions = link_tool_calls(&messages, &HashMap::new());
        assert_eq!(executions.len(), 1);
        assert!(executions[0].result.is_some());
        assert_eq!(executions[0].duration_ms, Some(1000.0));
    }

    #[test]
    fn test_link_tool_calls_empty() {
        let executions = link_tool_calls(&[], &HashMap::new());
        assert!(executions.is_empty());
    }

    #[test]
    fn test_compute_duration_ms_valid() {
        let d = compute_duration_ms("2025-01-01T00:00:00Z", "2025-01-01T00:00:05Z");
        assert_eq!(d, 5000.0);
    }

    #[test]
    fn test_compute_duration_ms_invalid() {
        let d = compute_duration_ms("not-a-date", "also-not");
        assert_eq!(d, 0.0);
    }

    #[test]
    fn test_build_task_executions_basic() {
        let tc = ToolCall {
            id: "task1".to_string(),
            name: "Task".to_string(),
            input: serde_json::json!({"prompt": "do stuff"}),
            is_task: true,
            task_description: Some("do stuff".to_string()),
            task_subagent_type: None,
        };

        let tr = make_tool_result("task1", "task result");

        let process = Process {
            id: "proc1".to_string(),
            file_path: "/tmp/agent.jsonl".to_string(),
            messages: vec![],
            start_time: "2025-01-01T00:00:00Z".to_string(),
            end_time: "2025-01-01T00:00:10Z".to_string(),
            duration_ms: 10000.0,
            metrics: crate::models::domain::SessionMetrics {
                duration_ms: 10000.0,
                total_tokens: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                message_count: 0,
                cost_usd: None,
            },
            description: Some("do stuff".to_string()),
            subagent_type: None,
            is_parallel: false,
            parent_task_id: Some("task1".to_string()),
            is_ongoing: None,
            main_session_impact: None,
            team: None,
        };

        let messages = vec![
            make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc]),
            make_user_result_msg("u1", "2025-01-01T00:00:10Z", vec![tr]),
        ];

        let task_execs = build_task_executions(&messages, &[process]);
        assert_eq!(task_execs.len(), 1);
        assert_eq!(task_execs[0].task_call.id, "task1");
        assert_eq!(task_execs[0].subagent.id, "proc1");
        assert_eq!(task_execs[0].duration_ms, 10000.0);
    }

    #[test]
    fn test_build_task_executions_no_match() {
        let tc = ToolCall {
            id: "task1".to_string(),
            name: "Task".to_string(),
            input: serde_json::json!({}),
            is_task: true,
            task_description: None,
            task_subagent_type: None,
        };

        let messages = vec![make_assistant_msg("a1", "2025-01-01T00:00:00Z", vec![tc])];

        // No processes match
        let task_execs = build_task_executions(&messages, &[]);
        assert!(task_execs.is_empty());
    }
}
