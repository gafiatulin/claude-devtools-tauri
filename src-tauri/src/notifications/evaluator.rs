use std::collections::HashMap;

use regex::Regex;
use uuid::Uuid;

use crate::models::jsonl::{
    AssistantEntry, ChatHistoryEntry, ContentBlock, StringOrBlocks, UserEntry,
};
use crate::models::notifications::{DetectedError, DetectedErrorContext, NotificationTrigger};

/// Context needed to evaluate triggers against a JSONL entry.
pub struct EvalContext {
    pub project_id: String,
    pub session_id: String,
    pub file_path: String,
    pub project_name: String,
    /// Repository IDs the current session belongs to (for scope filtering).
    pub repository_ids: Vec<String>,
}

/// Pre-compiled regex cache for notification triggers.
///
/// Compiles match_pattern and ignore_patterns once upfront so they can be
/// reused across thousands of JSONL entry evaluations without re-compiling.
pub struct RegexCache {
    /// trigger_id -> compiled match_pattern regex
    match_patterns: HashMap<String, Regex>,
    /// trigger_id -> compiled ignore_patterns regexes
    ignore_patterns: HashMap<String, Vec<Regex>>,
}

impl RegexCache {
    /// Build a regex cache for the given triggers. Invalid patterns are silently skipped.
    pub fn new(triggers: &[NotificationTrigger]) -> Self {
        let mut match_patterns = HashMap::new();
        let mut ignore_patterns = HashMap::new();

        for trigger in triggers {
            if let Some(pattern) = &trigger.match_pattern {
                if let Ok(re) = Regex::new(pattern) {
                    match_patterns.insert(trigger.id.clone(), re);
                }
            }

            if let Some(patterns) = &trigger.ignore_patterns {
                let compiled: Vec<Regex> =
                    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
                if !compiled.is_empty() {
                    ignore_patterns.insert(trigger.id.clone(), compiled);
                }
            }
        }

        Self {
            match_patterns,
            ignore_patterns,
        }
    }

    /// Validate all patterns in a set of triggers. Returns Err with details for invalid regex.
    pub fn validate(triggers: &[NotificationTrigger]) -> Result<(), String> {
        for trigger in triggers {
            if let Some(pattern) = &trigger.match_pattern {
                if let Err(e) = Regex::new(pattern) {
                    return Err(format!(
                        "Invalid match_pattern in trigger '{}': {e}",
                        trigger.name
                    ));
                }
            }
            if let Some(patterns) = &trigger.ignore_patterns {
                for (i, p) in patterns.iter().enumerate() {
                    if let Err(e) = Regex::new(p) {
                        return Err(format!(
                            "Invalid ignore_pattern #{} in trigger '{}': {e}",
                            i, trigger.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn get_match_regex(&self, trigger_id: &str) -> Option<&Regex> {
        self.match_patterns.get(trigger_id)
    }

    fn should_ignore(&self, text: &str, trigger: &NotificationTrigger) -> bool {
        if let Some(regexes) = self.ignore_patterns.get(&trigger.id) {
            return regexes.iter().any(|re| re.is_match(text));
        }
        false
    }
}

/// Evaluate all triggers against a single JSONL entry.
/// Returns a `DetectedError` for each trigger that fires.
pub fn evaluate_triggers(
    entry: &ChatHistoryEntry,
    triggers: &[NotificationTrigger],
    context: &EvalContext,
    line_number: Option<u32>,
) -> Vec<DetectedError> {
    let cache = RegexCache::new(triggers);
    evaluate_triggers_cached(entry, triggers, context, line_number, &cache)
}

/// Evaluate all triggers using a pre-compiled regex cache.
/// Use this when evaluating many entries against the same triggers.
pub fn evaluate_triggers_cached(
    entry: &ChatHistoryEntry,
    triggers: &[NotificationTrigger],
    context: &EvalContext,
    line_number: Option<u32>,
    cache: &RegexCache,
) -> Vec<DetectedError> {
    let now_ms = chrono::Utc::now().timestamp_millis() as f64;

    triggers
        .iter()
        .filter(|t| t.enabled)
        .filter(|t| !is_snoozed(t, now_ms))
        .filter(|t| is_in_scope(t, &context.repository_ids))
        .filter_map(|trigger| {
            evaluate_single_cached(entry, trigger, context, line_number, now_ms, cache)
        })
        .collect()
}

/// Check whether the trigger's global snooze is active.
fn is_snoozed(_trigger: &NotificationTrigger, _now_ms: f64) -> bool {
    // Individual trigger snooze is not modeled in the current schema.
    // Global snooze is handled at the caller level (config.notifications.snoozedUntil).
    false
}

/// Check whether the trigger's repository scope matches the current session.
fn is_in_scope(trigger: &NotificationTrigger, session_repo_ids: &[String]) -> bool {
    match &trigger.repository_ids {
        Some(ids) if !ids.is_empty() => {
            // Trigger is scoped: at least one of the session's repos must be in the list.
            session_repo_ids.iter().any(|r| ids.contains(r))
        }
        _ => true,
    }
}

/// Evaluate one trigger against one entry using a pre-compiled regex cache.
fn evaluate_single_cached(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    match trigger.mode.as_str() {
        "error_status" => {
            evaluate_error_status(entry, trigger, context, line_number, now_ms, cache)
        }
        "content_match" => {
            evaluate_content_match(entry, trigger, context, line_number, now_ms, cache)
        }
        "token_threshold" => evaluate_token_threshold(entry, trigger, context, line_number, now_ms),
        _ => None,
    }
}

// =============================================================================
// Mode: error_status
// =============================================================================

fn evaluate_error_status(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    // error_status mode only applies to tool_result content type
    if trigger.content_type != "tool_result" {
        return None;
    }

    // Look for tool_result content blocks with is_error=true in user entries
    // (tool results are delivered as content blocks inside user messages)
    let ChatHistoryEntry::User(user) = entry else {
        return None;
    };

    let blocks = match &user.message.content {
        StringOrBlocks::Blocks(blocks) => blocks,
        StringOrBlocks::String(_) => return None,
    };

    for block in blocks {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = block
        {
            if is_error == &Some(true) {
                let message = extract_string_from_content(content);

                // Check ignore patterns via cache
                if cache.should_ignore(&message, trigger) {
                    continue;
                }

                let timestamp = parse_timestamp_from_user(user);

                return Some(build_detected_error(
                    trigger,
                    context,
                    &message,
                    "ToolResult",
                    Some(tool_use_id.clone()),
                    user.common.session_id.clone(),
                    line_number,
                    timestamp,
                    now_ms,
                ));
            }
        }
    }

    None
}

// =============================================================================
// Mode: content_match
// =============================================================================

fn evaluate_content_match(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    let regex = cache.get_match_regex(&trigger.id)?;
    let match_field = trigger.match_field.as_deref().unwrap_or("content");

    match trigger.content_type.as_str() {
        "tool_result" => match_tool_result(
            entry,
            trigger,
            regex,
            match_field,
            context,
            line_number,
            now_ms,
            cache,
        ),
        "tool_use" => match_tool_use(
            entry,
            trigger,
            regex,
            match_field,
            context,
            line_number,
            now_ms,
            cache,
        ),
        "thinking" => match_thinking(entry, trigger, regex, context, line_number, now_ms, cache),
        "text" => match_text(entry, trigger, regex, context, line_number, now_ms, cache),
        _ => None,
    }
}

fn match_tool_result(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    regex: &Regex,
    _match_field: &str,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    let ChatHistoryEntry::User(user) = entry else {
        return None;
    };

    let blocks = match &user.message.content {
        StringOrBlocks::Blocks(blocks) => blocks,
        StringOrBlocks::String(_) => return None,
    };

    for block in blocks {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } = block
        {
            let text = extract_string_from_content(content);
            if regex.is_match(&text) {
                if cache.should_ignore(&text, trigger) {
                    continue;
                }

                let timestamp = parse_timestamp_from_user(user);
                let snippet = truncate_message(&text, 200);

                return Some(build_detected_error(
                    trigger,
                    context,
                    &snippet,
                    "ToolResult",
                    Some(tool_use_id.clone()),
                    user.common.session_id.clone(),
                    line_number,
                    timestamp,
                    now_ms,
                ));
            }
        }
    }

    None
}

fn match_tool_use(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    regex: &Regex,
    match_field: &str,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    let ChatHistoryEntry::Assistant(assistant) = entry else {
        return None;
    };

    for block in &assistant.message.content {
        if let ContentBlock::ToolUse { id, name, input } = block {
            // Filter by tool name if specified
            if let Some(required_tool) = &trigger.tool_name {
                if name != required_tool {
                    continue;
                }
            }

            let field_value = extract_tool_use_field(name, input, match_field);
            if regex.is_match(&field_value) {
                if cache.should_ignore(&field_value, trigger) {
                    continue;
                }

                let timestamp = parse_timestamp_from_assistant(assistant);
                let snippet = truncate_message(&field_value, 200);

                return Some(build_detected_error(
                    trigger,
                    context,
                    &snippet,
                    name,
                    Some(id.clone()),
                    assistant.common.session_id.clone(),
                    line_number,
                    timestamp,
                    now_ms,
                ));
            }
        }
    }

    None
}

fn match_thinking(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    regex: &Regex,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    let ChatHistoryEntry::Assistant(assistant) = entry else {
        return None;
    };

    for block in &assistant.message.content {
        if let ContentBlock::Thinking { thinking, .. } = block {
            if regex.is_match(thinking) {
                if cache.should_ignore(thinking, trigger) {
                    continue;
                }

                let timestamp = parse_timestamp_from_assistant(assistant);
                let snippet = truncate_message(thinking, 200);

                return Some(build_detected_error(
                    trigger,
                    context,
                    &snippet,
                    "thinking",
                    None,
                    assistant.common.session_id.clone(),
                    line_number,
                    timestamp,
                    now_ms,
                ));
            }
        }
    }

    None
}

fn match_text(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    regex: &Regex,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
    cache: &RegexCache,
) -> Option<DetectedError> {
    let ChatHistoryEntry::Assistant(assistant) = entry else {
        return None;
    };

    for block in &assistant.message.content {
        if let ContentBlock::Text { text } = block {
            if regex.is_match(text) {
                if cache.should_ignore(text, trigger) {
                    continue;
                }

                let timestamp = parse_timestamp_from_assistant(assistant);
                let snippet = truncate_message(text, 200);

                return Some(build_detected_error(
                    trigger,
                    context,
                    &snippet,
                    "assistant",
                    None,
                    assistant.common.session_id.clone(),
                    line_number,
                    timestamp,
                    now_ms,
                ));
            }
        }
    }

    None
}

// =============================================================================
// Mode: token_threshold
// =============================================================================

fn evaluate_token_threshold(
    entry: &ChatHistoryEntry,
    trigger: &NotificationTrigger,
    context: &EvalContext,
    line_number: Option<u32>,
    now_ms: f64,
) -> Option<DetectedError> {
    let threshold = trigger.token_threshold?;
    let token_type = trigger.token_type.as_deref().unwrap_or("total");

    let ChatHistoryEntry::Assistant(assistant) = entry else {
        return None;
    };

    let usage = &assistant.message.usage;
    let token_count = match token_type {
        "input" => usage.input_tokens,
        "output" => usage.output_tokens,
        "total" => usage.input_tokens + usage.output_tokens,
        _ => return None,
    };

    if token_count >= threshold {
        let timestamp = parse_timestamp_from_assistant(assistant);
        let message = format!(
            "{} tokens ({}) exceeded threshold of {}",
            token_count, token_type, threshold
        );

        return Some(build_detected_error(
            trigger,
            context,
            &message,
            "assistant",
            None,
            assistant.common.session_id.clone(),
            line_number,
            timestamp,
            now_ms,
        ));
    }

    None
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract a named field value from a tool_use input object.
fn extract_tool_use_field(tool_name: &str, input: &serde_json::Value, match_field: &str) -> String {
    // For tool_use, the match_field refers to a key in the input JSON.
    // Common mappings:
    //   Bash: "command", "description"
    //   Task: "description", "prompt", "subagent_type"
    //   Read: "file_path"
    //   Write: "file_path", "content"
    //   Edit: "file_path", "old_string", "new_string"
    //   Glob: "pattern", "path"
    //   Grep: "pattern", "path", "glob"
    //   WebFetch: "url", "prompt"
    //   WebSearch: "query"
    //   Skill: "skill", "args"
    //
    // If the field is "toolName", return the tool name itself.
    if match_field == "toolName" {
        return tool_name.to_string();
    }

    // Otherwise look up the field in the input object
    input
        .get(match_field)
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            _ => Some(v.to_string()),
        })
        .unwrap_or_default()
}

/// Extract text content from a ToolResult's content (string or blocks).
fn extract_string_from_content(content: &StringOrBlocks) -> String {
    match content {
        StringOrBlocks::String(s) => s.clone(),
        StringOrBlocks::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| b.as_text())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn parse_timestamp_from_user(user: &UserEntry) -> f64 {
    user.common
        .timestamp
        .as_deref()
        .and_then(parse_iso_timestamp)
        .unwrap_or(0.0)
}

fn parse_timestamp_from_assistant(assistant: &AssistantEntry) -> f64 {
    assistant
        .common
        .timestamp
        .as_deref()
        .and_then(parse_iso_timestamp)
        .unwrap_or(0.0)
}

fn parse_iso_timestamp(ts: &str) -> Option<f64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis() as f64)
}

fn truncate_message(msg: &str, max_len: usize) -> String {
    if msg.len() <= max_len {
        msg.to_string()
    } else {
        format!("{}...", &msg[..max_len])
    }
}

fn build_detected_error(
    trigger: &NotificationTrigger,
    context: &EvalContext,
    message: &str,
    source: &str,
    tool_use_id: Option<String>,
    _session_id_from_entry: String,
    line_number: Option<u32>,
    timestamp: f64,
    now_ms: f64,
) -> DetectedError {
    DetectedError {
        id: Uuid::new_v4().to_string(),
        timestamp,
        session_id: context.session_id.clone(),
        project_id: context.project_id.clone(),
        file_path: context.file_path.clone(),
        source: source.to_string(),
        message: message.to_string(),
        line_number,
        tool_use_id,
        subagent_id: None,
        is_read: false,
        created_at: now_ms,
        trigger_color: trigger.color.clone(),
        trigger_id: Some(trigger.id.clone()),
        trigger_name: Some(trigger.name.clone()),
        context: DetectedErrorContext {
            project_name: context.project_name.clone(),
            cwd: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::jsonl::*;

    fn make_eval_context() -> EvalContext {
        EvalContext {
            project_id: "test-project".to_string(),
            session_id: "test-session".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            project_name: "Test Project".to_string(),
            repository_ids: vec![],
        }
    }

    fn make_error_trigger() -> NotificationTrigger {
        NotificationTrigger {
            id: "test-error".to_string(),
            name: "Test Error".to_string(),
            enabled: true,
            content_type: "tool_result".to_string(),
            tool_name: None,
            is_builtin: None,
            ignore_patterns: None,
            mode: "error_status".to_string(),
            require_error: Some(true),
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }
    }

    fn make_user_entry_with_error(is_error: bool) -> ChatHistoryEntry {
        ChatHistoryEntry::User(UserEntry {
            common: ConversationalFields {
                timestamp: Some("2025-01-01T00:00:00Z".to_string()),
                uuid: Some("u1".to_string()),
                parent_uuid: None,
                is_sidechain: false,
                user_type: "external".to_string(),
                cwd: "/tmp".to_string(),
                session_id: "test-session".to_string(),
                version: "2.1".to_string(),
                git_branch: "main".to_string(),
                slug: None,
            },
            message: UserMessage {
                role: "user".to_string(),
                content: StringOrBlocks::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "tu1".to_string(),
                    content: StringOrBlocks::String("Error: something went wrong".to_string()),
                    is_error: Some(is_error),
                }]),
            },
            is_meta: None,
            agent_id: None,
            tool_use_result: None,
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            plan_content: None,
            is_visible_in_transcript_only: false,
        })
    }

    #[test]
    fn test_error_status_trigger_fires_on_error() {
        let ctx = make_eval_context();
        let trigger = make_error_trigger();
        let entry = make_user_entry_with_error(true);

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(1));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "ToolResult");
        assert!(results[0].message.contains("something went wrong"));
    }

    #[test]
    fn test_error_status_trigger_ignores_non_error() {
        let ctx = make_eval_context();
        let trigger = make_error_trigger();
        let entry = make_user_entry_with_error(false);

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(1));
        assert!(results.is_empty());
    }

    #[test]
    fn test_disabled_trigger_is_skipped() {
        let ctx = make_eval_context();
        let mut trigger = make_error_trigger();
        trigger.enabled = false;
        let entry = make_user_entry_with_error(true);

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(1));
        assert!(results.is_empty());
    }

    #[test]
    fn test_repository_scope_filters() {
        let mut ctx = make_eval_context();
        ctx.repository_ids = vec!["repo-a".to_string()];

        let mut trigger = make_error_trigger();
        trigger.repository_ids = Some(vec!["repo-b".to_string()]);
        let entry = make_user_entry_with_error(true);

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(1));
        assert!(results.is_empty());
    }

    #[test]
    fn test_ignore_pattern_suppresses() {
        let ctx = make_eval_context();
        let mut trigger = make_error_trigger();
        trigger.ignore_patterns = Some(vec!["something went wrong".to_string()]);
        let entry = make_user_entry_with_error(true);

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(1));
        assert!(results.is_empty());
    }

    #[test]
    fn test_content_match_trigger() {
        let ctx = make_eval_context();
        let trigger = NotificationTrigger {
            id: "test-content".to_string(),
            name: "Bash .env".to_string(),
            enabled: true,
            content_type: "tool_use".to_string(),
            tool_name: Some("Bash".to_string()),
            is_builtin: None,
            ignore_patterns: None,
            mode: "content_match".to_string(),
            require_error: None,
            match_field: Some("command".to_string()),
            match_pattern: Some(r"\.env".to_string()),
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        };

        let entry = ChatHistoryEntry::Assistant(AssistantEntry {
            common: ConversationalFields {
                timestamp: Some("2025-01-01T00:00:00Z".to_string()),
                uuid: Some("a1".to_string()),
                parent_uuid: None,
                is_sidechain: false,
                user_type: "external".to_string(),
                cwd: "/tmp".to_string(),
                session_id: "test-session".to_string(),
                version: "2.1".to_string(),
                git_branch: "main".to_string(),
                slug: None,
            },
            message: AssistantMessage {
                role: "assistant".to_string(),
                model: "claude".to_string(),
                id: "m1".to_string(),
                message_type: "message".to_string(),
                content: vec![ContentBlock::ToolUse {
                    id: "tu1".to_string(),
                    name: "Bash".to_string(),
                    input: serde_json::json!({"command": "cat .env.local"}),
                }],
                stop_reason: Some("tool_use".to_string()),
                stop_sequence: None,
                usage: UsageMetadata::default(),
            },
            request_id: "r1".to_string(),
            agent_id: None,
        });

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(5));
        assert_eq!(results.len(), 1);
        assert!(results[0].message.contains(".env"));
    }

    /// Live integration test: parse a real JSONL file from ~/.claude and
    /// evaluate the default triggers against every entry.
    /// Run with: cargo test test_live_trigger_evaluation -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_live_trigger_evaluation() {
        let home = dirs::home_dir().unwrap();
        let claude_root = home.join(".claude");
        if !claude_root.exists() {
            println!("No ~/.claude directory, skipping");
            return;
        }

        // Load default triggers
        let triggers = crate::config::defaults::default_triggers();
        println!("Testing {} default triggers", triggers.len());
        for t in &triggers {
            println!(
                "  - {} (mode: {}, content_type: {})",
                t.name, t.mode, t.content_type
            );
        }

        // Find a real JSONL file to test against
        let projects_dir = claude_root.join("projects");
        let mut tested = false;

        if let Ok(project_entries) = std::fs::read_dir(&projects_dir) {
            for project_entry in project_entries.filter_map(|e| e.ok()) {
                let project_dir = project_entry.path();
                if !project_dir.is_dir() {
                    continue;
                }

                let project_id = project_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if let Ok(session_entries) = std::fs::read_dir(&project_dir) {
                    for session_entry in session_entries.filter_map(|e| e.ok()) {
                        let session_path = session_entry.path();
                        if session_path
                            .extension()
                            .map(|e| e == "jsonl")
                            .unwrap_or(false)
                        {
                            // Skip subagent files
                            let fname = session_path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy();
                            if fname.starts_with("agent_") {
                                continue;
                            }

                            println!("\nTesting against: {}", session_path.display());
                            let entries =
                                crate::jsonl::reader::read_jsonl_file(&session_path).unwrap();
                            println!("  Parsed {} entries", entries.len());

                            let ctx = EvalContext {
                                project_id: project_id.clone(),
                                session_id: fname.to_string(),
                                file_path: session_path.to_string_lossy().to_string(),
                                project_name: project_id.clone(),
                                repository_ids: vec![],
                            };

                            let mut total_triggered = 0;
                            for (i, entry) in entries.iter().enumerate() {
                                let results =
                                    evaluate_triggers(entry, &triggers, &ctx, Some((i + 1) as u32));
                                for r in &results {
                                    if total_triggered < 10 {
                                        println!(
                                            "  [line {}] {} — {} (trigger: {})",
                                            r.line_number.unwrap_or(0),
                                            r.source,
                                            &r.message[..r.message.len().min(100)],
                                            r.trigger_name.as_deref().unwrap_or("?"),
                                        );
                                    }
                                    total_triggered += 1;
                                }
                            }
                            println!("  Triggers fired: {}", total_triggered);
                            tested = true;

                            // Test just one session per project, and at most 3 projects
                            break;
                        }
                    }
                }

                if tested {
                    break;
                }
            }
        }

        if !tested {
            println!("No JSONL files found to test against");
        }
    }

    #[test]
    fn test_regex_cache_reuses_compiled_patterns() {
        let triggers = vec![
            NotificationTrigger {
                id: "t1".to_string(),
                name: "Match env".to_string(),
                enabled: true,
                content_type: "tool_use".to_string(),
                tool_name: Some("Bash".to_string()),
                is_builtin: None,
                ignore_patterns: Some(vec![r"\.env\.example".to_string()]),
                mode: "content_match".to_string(),
                require_error: None,
                match_field: Some("command".to_string()),
                match_pattern: Some(r"\.env".to_string()),
                token_threshold: None,
                token_type: None,
                repository_ids: None,
                color: None,
            },
            NotificationTrigger {
                id: "t2".to_string(),
                name: "Invalid regex".to_string(),
                enabled: true,
                content_type: "tool_use".to_string(),
                tool_name: None,
                is_builtin: None,
                ignore_patterns: None,
                mode: "content_match".to_string(),
                require_error: None,
                match_field: None,
                match_pattern: Some(r"[invalid".to_string()),
                token_threshold: None,
                token_type: None,
                repository_ids: None,
                color: None,
            },
        ];

        let cache = RegexCache::new(&triggers);

        // Valid pattern should be cached
        assert!(cache.get_match_regex("t1").is_some());
        // Invalid pattern should be silently skipped
        assert!(cache.get_match_regex("t2").is_none());
        // Ignore patterns should be cached
        assert!(cache.should_ignore(".env.example", &triggers[0]));
        assert!(!cache.should_ignore(".env.local", &triggers[0]));
    }

    #[test]
    fn test_regex_cache_validate_returns_error_for_invalid() {
        let triggers = vec![NotificationTrigger {
            id: "bad".to_string(),
            name: "Bad regex".to_string(),
            enabled: true,
            content_type: "tool_use".to_string(),
            tool_name: None,
            is_builtin: None,
            ignore_patterns: None,
            mode: "content_match".to_string(),
            require_error: None,
            match_field: None,
            match_pattern: Some(r"[invalid".to_string()),
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }];

        let result = RegexCache::validate(&triggers);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Bad regex"));
    }

    #[test]
    fn test_evaluate_triggers_cached_matches_non_cached() {
        let ctx = make_eval_context();
        let trigger = NotificationTrigger {
            id: "test-content".to_string(),
            name: "Bash .env".to_string(),
            enabled: true,
            content_type: "tool_use".to_string(),
            tool_name: Some("Bash".to_string()),
            is_builtin: None,
            ignore_patterns: None,
            mode: "content_match".to_string(),
            require_error: None,
            match_field: Some("command".to_string()),
            match_pattern: Some(r"\.env".to_string()),
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        };
        let triggers = vec![trigger];

        let entry = ChatHistoryEntry::Assistant(AssistantEntry {
            common: ConversationalFields {
                timestamp: Some("2025-01-01T00:00:00Z".to_string()),
                uuid: Some("a1".to_string()),
                parent_uuid: None,
                is_sidechain: false,
                user_type: "external".to_string(),
                cwd: "/tmp".to_string(),
                session_id: "test-session".to_string(),
                version: "2.1".to_string(),
                git_branch: "main".to_string(),
                slug: None,
            },
            message: AssistantMessage {
                role: "assistant".to_string(),
                model: "claude".to_string(),
                id: "m1".to_string(),
                message_type: "message".to_string(),
                content: vec![ContentBlock::ToolUse {
                    id: "tu1".to_string(),
                    name: "Bash".to_string(),
                    input: serde_json::json!({"command": "cat .env.local"}),
                }],
                stop_reason: Some("tool_use".to_string()),
                stop_sequence: None,
                usage: UsageMetadata::default(),
            },
            request_id: "r1".to_string(),
            agent_id: None,
        });

        // Both cached and non-cached should produce results
        let cache = RegexCache::new(&triggers);
        let cached_results = evaluate_triggers_cached(&entry, &triggers, &ctx, Some(1), &cache);
        let non_cached_results = evaluate_triggers(&entry, &triggers, &ctx, Some(1));

        assert_eq!(cached_results.len(), 1);
        assert_eq!(non_cached_results.len(), 1);
        assert_eq!(cached_results[0].source, non_cached_results[0].source);
    }

    #[test]
    fn test_token_threshold_trigger() {
        let ctx = make_eval_context();
        let trigger = NotificationTrigger {
            id: "test-tokens".to_string(),
            name: "High Tokens".to_string(),
            enabled: true,
            content_type: "tool_result".to_string(),
            tool_name: None,
            is_builtin: None,
            ignore_patterns: None,
            mode: "token_threshold".to_string(),
            require_error: None,
            match_field: None,
            match_pattern: None,
            token_threshold: Some(100),
            token_type: Some("total".to_string()),
            repository_ids: None,
            color: None,
        };

        let entry = ChatHistoryEntry::Assistant(AssistantEntry {
            common: ConversationalFields {
                timestamp: Some("2025-01-01T00:00:00Z".to_string()),
                uuid: Some("a1".to_string()),
                parent_uuid: None,
                is_sidechain: false,
                user_type: "external".to_string(),
                cwd: "/tmp".to_string(),
                session_id: "test-session".to_string(),
                version: "2.1".to_string(),
                git_branch: "main".to_string(),
                slug: None,
            },
            message: AssistantMessage {
                role: "assistant".to_string(),
                model: "claude".to_string(),
                id: "m1".to_string(),
                message_type: "message".to_string(),
                content: vec![ContentBlock::Text {
                    text: "hello".to_string(),
                }],
                stop_reason: Some("end_turn".to_string()),
                stop_sequence: None,
                usage: UsageMetadata {
                    input_tokens: 80,
                    output_tokens: 30,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                },
            },
            request_id: "r1".to_string(),
            agent_id: None,
        });

        let results = evaluate_triggers(&entry, &[trigger], &ctx, Some(10));
        assert_eq!(results.len(), 1);
        assert!(results[0].message.contains("110 tokens"));
    }
}
