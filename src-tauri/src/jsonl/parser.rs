use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::models::chunks::ToolProgress;
use crate::models::domain::MessageType;
use crate::models::jsonl::{
    AssistantEntry, ChatHistoryEntry, ContentBlock, ProgressData, StringOrBlocks, SystemEntry,
    UserEntry,
};
use crate::models::messages::{ParsedMessage, ToolCall, ToolResult};

/// Parse a single JSONL line into a `ChatHistoryEntry`.
pub fn parse_entry(line: &str) -> Result<ChatHistoryEntry, String> {
    serde_json::from_str(line).map_err(|e| e.to_string())
}

/// Convert raw JSONL entries into enriched `ParsedMessage` objects.
///
/// This extracts tool calls from assistant messages, tool results from user
/// messages, sets classification flags, and handles timestamp parsing.
/// Unknown entry types are silently filtered out.
///
/// Compact boundary detection: Claude injects a user message carrying the
/// compacted context summary immediately after each `system/compact_boundary`
/// entry.  That user message has `parentUuid` pointing to the
/// compact_boundary's UUID.  We detect this relationship here and set
/// `is_compact_summary: Some(true)` on the user message so it renders as a
/// `CompactBoundary` instead of a regular user message.
pub fn parse_entries_to_messages(entries: Vec<ChatHistoryEntry>) -> Vec<ParsedMessage> {
    let mut compact_boundary_uuids: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for entry in entries {
        // Track compact_boundary UUIDs so we can identify the following user message.
        if let ChatHistoryEntry::System(sys) = &entry {
            if sys.subtype == "compact_boundary" {
                if let Some(uuid) = &sys.common.uuid {
                    compact_boundary_uuids.insert(uuid.clone());
                }
            }
        }

        if let Some(mut msg) = entry_to_parsed_message(entry) {
            // The user message whose parentUuid matches a compact_boundary carries
            // the compacted context summary — mark it as a compact message.
            if msg.message_type == MessageType::User {
                if let Some(parent) = &msg.parent_uuid {
                    if compact_boundary_uuids.contains(parent.as_str()) {
                        msg.is_compact_summary = Some(true);
                    }
                }
            }
            result.push(msg);
        }
    }

    result
}

fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn fallback_timestamp() -> String {
    Utc::now().to_rfc3339()
}

/// Convert a single `ChatHistoryEntry` into a `ParsedMessage`.
/// Returns `None` for unknown/unhandled entry types.
fn entry_to_parsed_message(entry: ChatHistoryEntry) -> Option<ParsedMessage> {
    match entry {
        ChatHistoryEntry::User(e) => Some(user_entry_to_message(e)),
        ChatHistoryEntry::Assistant(e) => Some(assistant_entry_to_message(e)),
        ChatHistoryEntry::System(e) => Some(system_entry_to_message(e)),
        ChatHistoryEntry::Summary(e) => Some(ParsedMessage {
            uuid: e.uuid.unwrap_or_else(generate_uuid),
            parent_uuid: None,
            message_type: MessageType::Summary,
            timestamp: e.timestamp.unwrap_or_else(fallback_timestamp),
            role: None,
            content: StringOrBlocks::String(e.summary),
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
            is_compact_summary: Some(true),
            plan_content: None,
        }),
        ChatHistoryEntry::FileHistorySnapshot(e) => Some(ParsedMessage {
            uuid: e.uuid.unwrap_or_else(generate_uuid),
            parent_uuid: None,
            message_type: MessageType::FileHistorySnapshot,
            timestamp: e.timestamp.unwrap_or_else(fallback_timestamp),
            role: None,
            content: StringOrBlocks::String(String::new()),
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
        }),
        ChatHistoryEntry::QueueOperation(e) => Some(ParsedMessage {
            uuid: e.uuid.unwrap_or_else(generate_uuid),
            parent_uuid: None,
            message_type: MessageType::QueueOperation,
            timestamp: e.timestamp.unwrap_or_else(fallback_timestamp),
            role: None,
            content: StringOrBlocks::String(e.operation),
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
        }),
        // Progress and unknown entry types are silently skipped.
        ChatHistoryEntry::Progress(_) | ChatHistoryEntry::Unknown => None,
    }
}

fn user_entry_to_message(entry: UserEntry) -> ParsedMessage {
    let tool_results = extract_tool_results(&entry);
    // Claude Code JSONL often omits isMeta for tool result messages.
    // Infer is_meta = true when the message contains tool_result blocks,
    // so these get classified as AI-context (not filtered as noise).
    let is_meta = entry.is_meta.unwrap_or(!tool_results.is_empty());

    ParsedMessage {
        uuid: entry.common.uuid.unwrap_or_else(generate_uuid),
        parent_uuid: entry.common.parent_uuid,
        message_type: MessageType::User,
        timestamp: entry.common.timestamp.unwrap_or_else(fallback_timestamp),
        role: Some("user".to_string()),
        content: entry.message.content,
        usage: None,
        model: None,
        cwd: Some(entry.common.cwd),
        git_branch: Some(entry.common.git_branch),
        agent_id: entry.agent_id,
        is_sidechain: entry.common.is_sidechain,
        is_meta,
        user_type: Some(entry.common.user_type),
        tool_calls: Vec::new(),
        tool_results,
        source_tool_use_id: entry.source_tool_use_id,
        source_tool_assistant_uuid: entry.source_tool_assistant_uuid,
        tool_use_result: entry.tool_use_result,
        is_compact_summary: None,
        plan_content: entry.plan_content,
    }
}

fn assistant_entry_to_message(entry: AssistantEntry) -> ParsedMessage {
    let tool_calls = extract_tool_calls(&entry);

    ParsedMessage {
        uuid: entry.common.uuid.unwrap_or_else(generate_uuid),
        parent_uuid: entry.common.parent_uuid,
        message_type: MessageType::Assistant,
        timestamp: entry.common.timestamp.unwrap_or_else(fallback_timestamp),
        role: Some("assistant".to_string()),
        content: StringOrBlocks::Blocks(entry.message.content.clone()),
        usage: Some(entry.message.usage),
        model: Some(entry.message.model),
        cwd: Some(entry.common.cwd),
        git_branch: Some(entry.common.git_branch),
        agent_id: entry.agent_id,
        is_sidechain: entry.common.is_sidechain,
        is_meta: false,
        user_type: Some(entry.common.user_type),
        tool_calls,
        tool_results: Vec::new(),
        source_tool_use_id: None,
        source_tool_assistant_uuid: None,
        tool_use_result: None,
        is_compact_summary: None,
        plan_content: None,
    }
}

fn system_entry_to_message(entry: SystemEntry) -> ParsedMessage {
    // Use the content field if present (for local_command, compact_boundary subtypes),
    // otherwise default to empty string.
    let content_str = entry.content.unwrap_or_default();

    ParsedMessage {
        uuid: entry.common.uuid.unwrap_or_else(generate_uuid),
        parent_uuid: entry.common.parent_uuid,
        message_type: MessageType::System,
        timestamp: entry.common.timestamp.unwrap_or_else(fallback_timestamp),
        role: None,
        content: StringOrBlocks::String(content_str),
        usage: None,
        model: None,
        cwd: Some(entry.common.cwd),
        git_branch: Some(entry.common.git_branch),
        agent_id: None,
        is_sidechain: entry.common.is_sidechain,
        is_meta: entry.is_meta,
        user_type: Some(entry.common.user_type),
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
        source_tool_use_id: None,
        source_tool_assistant_uuid: None,
        tool_use_result: None,
        is_compact_summary: Some(entry.subtype == "compact_boundary"),
        plan_content: None,
    }
}

/// Extract `ToolCall` entries from an assistant message's content blocks.
fn extract_tool_calls(entry: &AssistantEntry) -> Vec<ToolCall> {
    entry
        .message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => {
                let is_task = name == "Task";
                let task_description = if is_task {
                    input
                        .get("description")
                        .or_else(|| input.get("prompt"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                };
                let task_subagent_type = if is_task {
                    input
                        .get("type")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                };

                Some(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    is_task,
                    task_description,
                    task_subagent_type,
                })
            }
            _ => None,
        })
        .collect()
}

/// Extract `ToolResult` entries from a user message's content blocks.
fn extract_tool_results(entry: &UserEntry) -> Vec<ToolResult> {
    match &entry.message.content {
        StringOrBlocks::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => Some(ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: match content {
                        StringOrBlocks::String(s) => serde_json::Value::String(s.clone()),
                        StringOrBlocks::Blocks(b) => serde_json::to_value(b).unwrap_or_default(),
                    },
                    is_error: is_error.unwrap_or(false),
                }),
                _ => None,
            })
            .collect(),
        StringOrBlocks::String(_) => Vec::new(),
    }
}

/// Maximum characters to keep from bash full_output to limit IPC payload size.
const MAX_PROGRESS_OUTPUT_CHARS: usize = 10_000;

/// Extract a progress map from JSONL entries, keyed by parent_tool_use_id.
/// Last-wins semantics: if multiple progress entries exist for the same tool,
/// only the most recent one is kept.
pub fn extract_progress_map(entries: &[ChatHistoryEntry]) -> HashMap<String, ToolProgress> {
    let mut map = HashMap::new();

    for entry in entries {
        if let Some(progress) = entry.as_progress() {
            let tool_progress = match &progress.data {
                ProgressData::BashProgress {
                    full_output,
                    elapsed_time_seconds,
                    total_lines,
                    timeout_ms,
                } => {
                    // Truncate to last N chars to limit IPC payload
                    let truncated = if full_output.len() > MAX_PROGRESS_OUTPUT_CHARS {
                        full_output[full_output.len() - MAX_PROGRESS_OUTPUT_CHARS..].to_string()
                    } else {
                        full_output.clone()
                    };
                    Some(ToolProgress::Bash {
                        full_output: truncated,
                        elapsed_time_seconds: *elapsed_time_seconds,
                        total_lines: *total_lines,
                        timeout_ms: *timeout_ms,
                    })
                }
                ProgressData::HookProgress {
                    hook_event,
                    hook_name,
                    command,
                } => Some(ToolProgress::Hook {
                    hook_event: hook_event.clone(),
                    hook_name: hook_name.clone(),
                    command: command.clone(),
                }),
                ProgressData::McpProgress {
                    status,
                    server_name,
                    tool_name,
                    elapsed_time_ms,
                } => Some(ToolProgress::Mcp {
                    status: status.clone(),
                    server_name: server_name.clone(),
                    tool_name: tool_name.clone(),
                    elapsed_time_ms: *elapsed_time_ms,
                }),
                ProgressData::WaitingForTask {
                    task_description,
                    task_type,
                } => Some(ToolProgress::Waiting {
                    task_description: task_description.clone(),
                    task_type: task_type.clone(),
                }),
                // Skip agent progress and unknown types
                ProgressData::AgentProgress {} | ProgressData::Unknown => None,
            };

            if let Some(tp) = tool_progress {
                map.insert(progress.parent_tool_use_id.clone(), tp);
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::jsonl::ChatHistoryEntry;

    #[test]
    fn test_parse_user_entry() {
        let json = r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hello world"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::User(_)));

        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, MessageType::User);
        assert_eq!(msgs[0].uuid, "u1");
        if let StringOrBlocks::String(s) = &msgs[0].content {
            assert_eq!(s, "hello world");
        } else {
            panic!("Expected String content");
        }
    }

    #[test]
    fn test_parse_assistant_entry() {
        let json = r#"{"type":"assistant","parentUuid":"u1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"assistant","model":"claude-sonnet","id":"m1","type":"message","content":[{"type":"text","text":"hi there"}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5}},"requestId":"r1","timestamp":"2025-01-01T00:00:01Z","uuid":"a1"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::Assistant(_)));

        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, MessageType::Assistant);
        assert_eq!(msgs[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(msgs[0].usage.as_ref().unwrap().input_tokens, 10);
    }

    #[test]
    fn test_parse_system_entry() {
        let json = r#"{"type":"system","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","subtype":"local_command","isMeta":false,"content":"output text","timestamp":"2025-01-01T00:00:02Z","uuid":"s1"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::System(_)));

        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, MessageType::System);
        if let StringOrBlocks::String(s) = &msgs[0].content {
            assert_eq!(s, "output text");
        }
    }

    #[test]
    fn test_parse_system_compact_boundary() {
        let json = r#"{"type":"system","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","subtype":"compact_boundary","isMeta":true,"timestamp":"2025-01-01T00:00:03Z","uuid":"s2"}"#;
        let entry = parse_entry(json).unwrap();
        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs[0].is_compact_summary, Some(true));
    }

    #[test]
    fn test_compact_boundary_marks_following_user_message() {
        // When a system/compact_boundary entry is followed by a user message
        // whose parentUuid points to the compact_boundary, that user message
        // should be marked as is_compact_summary = true.
        let compact_boundary_json = r#"{"type":"system","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","subtype":"compact_boundary","isMeta":false,"timestamp":"2025-01-01T00:00:03Z","uuid":"boundary-uuid-1"}"#;
        let summary_user_json = r#"{"type":"user","parentUuid":"boundary-uuid-1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"This session is being continued from a previous conversation that ran out of context."},"timestamp":"2025-01-01T00:00:04Z","uuid":"summary-user-1"}"#;
        let normal_user_json = r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"normal message"},"timestamp":"2025-01-01T00:00:05Z","uuid":"normal-u1"}"#;

        let entries: Vec<ChatHistoryEntry> = vec![
            compact_boundary_json,
            summary_user_json,
            normal_user_json,
        ]
        .into_iter()
        .map(|j| parse_entry(j).unwrap())
        .collect();

        let msgs = parse_entries_to_messages(entries);
        // compact_boundary system entry + summary user + normal user = 3 messages
        assert_eq!(msgs.len(), 3);
        // compact_boundary system entry still gets is_compact_summary = true
        assert_eq!(msgs[0].message_type, MessageType::System);
        assert_eq!(msgs[0].is_compact_summary, Some(true));
        // the summary user message is now marked as compact
        assert_eq!(msgs[1].message_type, MessageType::User);
        assert_eq!(msgs[1].is_compact_summary, Some(true));
        // normal user message is unaffected
        assert_eq!(msgs[2].message_type, MessageType::User);
        assert_eq!(msgs[2].is_compact_summary, None);
    }

    #[test]
    fn test_parse_summary_entry() {
        let json = r#"{"type":"summary","summary":"This is a summary","leafUuid":"leaf1","timestamp":"2025-01-01T00:00:00Z","uuid":"sum1"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::Summary(_)));

        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, MessageType::Summary);
        assert_eq!(msgs[0].is_compact_summary, Some(true));
    }

    #[test]
    fn test_parse_file_history_snapshot() {
        let json = r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{"messageId":"m1","trackedFileBackups":{},"timestamp":"2025-01-01T00:00:00Z"},"isSnapshotUpdate":false}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::FileHistorySnapshot(_)));

        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, MessageType::FileHistorySnapshot);
    }

    #[test]
    fn test_parse_queue_operation() {
        let json = r#"{"type":"queue-operation","timestamp":"2025-01-01T00:00:04Z","uuid":"q1","operation":"enqueue"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::QueueOperation(_)));

        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, MessageType::QueueOperation);
        if let StringOrBlocks::String(s) = &msgs[0].content {
            assert_eq!(s, "enqueue");
        }
    }

    #[test]
    fn test_parse_progress_type() {
        let json = r#"{"type":"progress","parentToolUseID":"tc1","data":{"type":"bash_progress","fullOutput":"hello","elapsedTimeSeconds":1.5,"totalLines":10,"timeoutMs":120000}}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::Progress(_)));
    }

    #[test]
    fn test_parse_unknown_type_becomes_unknown() {
        let json = r#"{"type":"some_future_type","data":{}}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::Unknown));
    }

    #[test]
    fn test_parse_entries_filters_unknown() {
        let entries = vec![
            ChatHistoryEntry::Unknown,
            ChatHistoryEntry::Unknown,
        ];
        let msgs = parse_entries_to_messages(entries);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_parse_malformed_json_returns_error() {
        let result = parse_entry("not valid json {{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_string_returns_error() {
        let result = parse_entry("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_missing_type_field() {
        let result = parse_entry(r#"{"data":"no type field"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_tool_calls_from_assistant() {
        let json = r#"{"type":"assistant","parentUuid":"u1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"assistant","model":"claude","id":"m1","type":"message","content":[{"type":"tool_use","id":"tc1","name":"Read","input":{"path":"/tmp/foo.txt"}},{"type":"text","text":"reading file"}],"stop_reason":"tool_use","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5}},"requestId":"r1","timestamp":"2025-01-01T00:00:01Z","uuid":"a1"}"#;
        let entry = parse_entry(json).unwrap();
        let msgs = parse_entries_to_messages(vec![entry]);
        assert_eq!(msgs[0].tool_calls.len(), 1);
        assert_eq!(msgs[0].tool_calls[0].name, "Read");
        assert_eq!(msgs[0].tool_calls[0].id, "tc1");
        assert!(!msgs[0].tool_calls[0].is_task);
    }

    #[test]
    fn test_extract_task_tool_call() {
        let json = r#"{"type":"assistant","parentUuid":"u1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"assistant","model":"claude","id":"m1","type":"message","content":[{"type":"tool_use","id":"tc2","name":"Task","input":{"description":"research topic","type":"research"}}],"stop_reason":"tool_use","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5}},"requestId":"r1","timestamp":"2025-01-01T00:00:01Z","uuid":"a1"}"#;
        let entry = parse_entry(json).unwrap();
        let msgs = parse_entries_to_messages(vec![entry]);
        let tc = &msgs[0].tool_calls[0];
        assert!(tc.is_task);
        assert_eq!(tc.task_description.as_deref(), Some("research topic"));
        assert_eq!(tc.task_subagent_type.as_deref(), Some("research"));
    }

    #[test]
    fn test_mixed_entries_dispatch() {
        let user_json = r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hi"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#;
        let progress_json = r#"{"type":"progress","parentToolUseID":"tc1","data":{"type":"bash_progress","fullOutput":"output","elapsedTimeSeconds":1.0,"totalLines":5,"timeoutMs":120000}}"#;
        let system_json = r#"{"type":"system","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","subtype":"turn_duration","durationMs":500.0,"isMeta":true,"timestamp":"2025-01-01T00:00:01Z","uuid":"s1"}"#;

        let entries: Vec<ChatHistoryEntry> = vec![user_json, progress_json, system_json]
            .into_iter()
            .map(|j| parse_entry(j).unwrap())
            .collect();

        assert_eq!(entries.len(), 3);
        let msgs = parse_entries_to_messages(entries);
        // progress is filtered out, user + system remain
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].message_type, MessageType::User);
        assert_eq!(msgs[1].message_type, MessageType::System);
    }

    #[test]
    fn test_extract_progress_map_bash() {
        let json = r#"{"type":"progress","parentToolUseID":"tc1","data":{"type":"bash_progress","fullOutput":"hello world","elapsedTimeSeconds":2.5,"totalLines":10,"timeoutMs":120000}}"#;
        let entry = parse_entry(json).unwrap();
        let map = extract_progress_map(&[entry]);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("tc1"));
        match &map["tc1"] {
            crate::models::chunks::ToolProgress::Bash { full_output, elapsed_time_seconds, .. } => {
                assert_eq!(full_output, "hello world");
                assert!((elapsed_time_seconds - 2.5).abs() < f64::EPSILON);
            }
            _ => panic!("Expected Bash progress"),
        }
    }

    #[test]
    fn test_extract_progress_map_last_wins() {
        let json1 = r#"{"type":"progress","parentToolUseID":"tc1","data":{"type":"bash_progress","fullOutput":"first","elapsedTimeSeconds":1.0,"totalLines":1,"timeoutMs":120000}}"#;
        let json2 = r#"{"type":"progress","parentToolUseID":"tc1","data":{"type":"bash_progress","fullOutput":"second","elapsedTimeSeconds":2.0,"totalLines":2,"timeoutMs":120000}}"#;
        let entries: Vec<ChatHistoryEntry> = vec![json1, json2]
            .into_iter()
            .map(|j| parse_entry(j).unwrap())
            .collect();
        let map = extract_progress_map(&entries);
        assert_eq!(map.len(), 1);
        match &map["tc1"] {
            crate::models::chunks::ToolProgress::Bash { full_output, .. } => {
                assert_eq!(full_output, "second");
            }
            _ => panic!("Expected Bash progress"),
        }
    }

    #[test]
    fn test_parse_real_world_bash_progress() {
        // Exact format from a real Claude Code session — includes extra fields
        let json = r#"{"parentUuid":"5561cd77","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1.44","gitBranch":"main","slug":"test","type":"progress","data":{"type":"bash_progress","output":"short","fullOutput":"full line1\nfull line2","elapsedTimeSeconds":15,"totalLines":24,"timeoutMs":300000},"toolUseID":"bash-progress-13","parentToolUseID":"toolu_01BQ","uuid":"b6f7e044","timestamp":"2026-02-17T13:46:20.050Z"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::Progress(_)));
        let map = extract_progress_map(&[entry]);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("toolu_01BQ"));
        match &map["toolu_01BQ"] {
            crate::models::chunks::ToolProgress::Bash { full_output, elapsed_time_seconds, total_lines, timeout_ms } => {
                assert_eq!(full_output, "full line1\nfull line2");
                assert!((*elapsed_time_seconds - 15.0).abs() < f64::EPSILON);
                assert_eq!(*total_lines, 24);
                assert_eq!(*timeout_ms, 300000);
            }
            _ => panic!("Expected Bash progress"),
        }
    }

    #[test]
    fn test_parse_real_world_hook_progress() {
        let json = r#"{"parentUuid":"5bddd7aa","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1.44","gitBranch":"main","slug":"test","type":"progress","data":{"type":"hook_progress","hookEvent":"PostToolUse","hookName":"PostToolUse:Read","command":"callback"},"parentToolUseID":"toolu_01Hvq","toolUseID":"toolu_01Hvq","timestamp":"2026-02-17T13:59:11.825Z","uuid":"55dd9dc5"}"#;
        let entry = parse_entry(json).unwrap();
        assert!(matches!(entry, ChatHistoryEntry::Progress(_)));
        let map = extract_progress_map(&[entry]);
        assert_eq!(map.len(), 1);
        match &map["toolu_01Hvq"] {
            crate::models::chunks::ToolProgress::Hook { hook_event, hook_name, command } => {
                assert_eq!(hook_event, "PostToolUse");
                assert_eq!(hook_name, "PostToolUse:Read");
                assert_eq!(command, "callback");
            }
            _ => panic!("Expected Hook progress"),
        }
    }

    #[test]
    fn test_extract_progress_map_skips_agent_progress() {
        let json = r#"{"type":"progress","parentToolUseID":"tc1","data":{"type":"agent_progress"}}"#;
        let entry = parse_entry(json).unwrap();
        let map = extract_progress_map(&[entry]);
        assert!(map.is_empty());
    }
}
