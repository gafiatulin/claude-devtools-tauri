use std::fs;
use std::path::Path;

use crate::models::chunks::Process;
use crate::models::domain::SessionMetrics;
use crate::models::messages::ParsedMessage;

/// Scan for subagent JSONL files associated with a session.
///
/// Handles two directory structures:
///
/// **New structure (preferred):**
/// ```text
/// {project_dir}/{session_uuid}/subagents/agent-{agent_uuid}.jsonl
/// ```
///
/// **Legacy structure:**
/// ```text
/// {project_dir}/agent_{agent_uuid}.jsonl   (linked by sessionId field inside)
/// ```
pub fn scan_subagents(project_dir: &Path, session_id: &str) -> Result<Vec<Process>, String> {
    let mut processes = Vec::new();

    // 1. New structure: {session_uuid}/subagents/agent-*.jsonl
    let subagents_dir = project_dir.join(session_id).join("subagents");
    if subagents_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&subagents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };

                // Match agent-{id}.jsonl pattern
                if !file_name.starts_with("agent-") || !file_name.ends_with(".jsonl") {
                    continue;
                }

                let agent_id = file_name
                    .trim_start_matches("agent-")
                    .trim_end_matches(".jsonl")
                    .to_string();

                match parse_subagent_file(&path, &agent_id, session_id) {
                    Ok(process) => processes.push(process),
                    Err(e) => {
                        eprintln!(
                            "[scanner] Warning: failed to parse subagent {}: {e}",
                            path.display()
                        );
                    }
                }
            }
        }
    }

    // 2. Legacy structure: agent_*.jsonl at project root
    if let Ok(entries) = fs::read_dir(project_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            // Match agent_*.jsonl pattern (legacy)
            if !file_name.starts_with("agent_") || !file_name.ends_with(".jsonl") {
                continue;
            }

            let agent_id = file_name
                .trim_start_matches("agent_")
                .trim_end_matches(".jsonl")
                .to_string();

            // For legacy files, verify they belong to this session by checking
            // the sessionId field in the first entry
            if !legacy_agent_belongs_to_session(&path, session_id) {
                continue;
            }

            match parse_subagent_file(&path, &agent_id, session_id) {
                Ok(process) => processes.push(process),
                Err(e) => {
                    eprintln!(
                        "[scanner] Warning: failed to parse legacy subagent {}: {e}",
                        path.display()
                    );
                }
            }
        }
    }

    // Detect parallel execution and sort by start time
    detect_parallel_execution(&mut processes);
    processes.sort_by(|a, b| a.start_time.cmp(&b.start_time));

    Ok(processes)
}

/// Check if a legacy agent file belongs to a specific session.
///
/// Reads the first line of the file and checks the `sessionId` field.
fn legacy_agent_belongs_to_session(path: &Path, session_id: &str) -> bool {
    use std::io::{BufRead, BufReader};

    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let reader = BufReader::new(file);
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse just the first valid line
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return value
                .get("sessionId")
                .and_then(|s| s.as_str())
                .map_or(false, |s| s == session_id);
        }
        break;
    }

    false
}

/// Parse a subagent JSONL file into a Process.
fn parse_subagent_file(
    path: &Path,
    agent_id: &str,
    _session_id: &str,
) -> Result<Process, String> {
    let entries = crate::jsonl::reader::read_jsonl_file(path)?;
    let messages = crate::jsonl::parser::parse_entries_to_messages(entries);

    if messages.is_empty() {
        return Err("Subagent file is empty".to_string());
    }

    let start_time = messages.first().map(|m| m.timestamp.clone()).unwrap_or_default();
    let end_time = messages.last().map(|m| m.timestamp.clone()).unwrap_or_default();

    // Compute duration
    let duration_ms = compute_duration_ms(&start_time, &end_time);

    // Compute metrics
    let metrics = compute_subagent_metrics(&messages);

    // Extract description from the first Task tool call's description field
    let description = extract_task_description(&messages);

    // Extract subagent type from the first Task tool call
    let subagent_type = extract_subagent_type(&messages);

    // Find parent task ID by looking at the first message's sourceToolUseID
    let parent_task_id = messages
        .first()
        .and_then(|m| m.source_tool_use_id.clone());

    // Detect if ongoing (last message is a user message waiting for response)
    let is_ongoing = messages
        .last()
        .map(|m| {
            m.message_type == crate::models::domain::MessageType::User && !m.is_meta
        });

    Ok(Process {
        id: agent_id.to_string(),
        file_path: path.to_string_lossy().to_string(),
        messages,
        start_time,
        end_time,
        duration_ms,
        metrics,
        description,
        subagent_type,
        is_parallel: false, // Will be set by detect_parallel_execution
        parent_task_id,
        is_ongoing,
        main_session_impact: None,
        team: None,
    })
}

/// Compute the duration in milliseconds between two ISO 8601 timestamps.
fn compute_duration_ms(start: &str, end: &str) -> f64 {
    let start_dt = chrono::DateTime::parse_from_rfc3339(start);
    let end_dt = chrono::DateTime::parse_from_rfc3339(end);

    match (start_dt, end_dt) {
        (Ok(s), Ok(e)) => {
            let diff = e.signed_duration_since(s);
            diff.num_milliseconds() as f64
        }
        _ => 0.0,
    }
}

/// Compute session metrics from subagent messages.
fn compute_subagent_metrics(messages: &[ParsedMessage]) -> SessionMetrics {
    crate::scanner::sessions::compute_session_metrics(messages)
}

/// Extract the `summary` attribute from a `<teammate-message summary="...">` XML block.
fn extract_teammate_summary(s: &str) -> Option<String> {
    let tag_start = s.find("<teammate-message")?;
    let tag_end = s[tag_start..].find('>')?;
    let attrs = &s[tag_start..tag_start + tag_end];

    // Find summary="..."
    let key = "summary=\"";
    let val_start = attrs.find(key)? + key.len();
    let val_end = attrs[val_start..].find('"')?;
    let summary = attrs[val_start..val_start + val_end].trim();
    if summary.is_empty() {
        None
    } else {
        Some(summary.to_string())
    }
}

/// Extract the Task description from messages.
///
/// For team-based agents the first user message is a `<teammate-message>` block;
/// we pull the `summary` attribute rather than the raw XML. For native Task
/// subagents we use the plain text content.
fn extract_task_description(messages: &[ParsedMessage]) -> Option<String> {
    for msg in messages {
        if msg.message_type == crate::models::domain::MessageType::User && !msg.is_meta {
            let raw_text = match &msg.content {
                crate::models::jsonl::StringOrBlocks::String(s) => Some(s.as_str()),
                crate::models::jsonl::StringOrBlocks::Blocks(blocks) => {
                    blocks.iter().find_map(|b| b.as_text())
                }
            };

            if let Some(text) = raw_text {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Prefer the summary= attribute from <teammate-message> blocks
                if let Some(summary) = extract_teammate_summary(trimmed) {
                    return Some(summary);
                }
                // Fallback: plain text content
                return Some(truncate_string(trimmed, 200));
            }
        }
    }
    None
}

/// Extract the subagent type from messages (e.g., "code", "research").
fn extract_subagent_type(_messages: &[ParsedMessage]) -> Option<String> {
    // The subagent type is typically set by the parent Task tool call's "type" input,
    // not in the subagent's own messages. We would need parent context to extract this.
    // For now, return None and let the parent session linker fill this in.
    None
}

/// Detect parallel execution among subagent processes.
///
/// Two processes are considered parallel if their time ranges overlap.
fn detect_parallel_execution(processes: &mut [Process]) {
    if processes.len() < 2 {
        return;
    }

    // Parse all start/end times
    let ranges: Vec<(f64, f64)> = processes
        .iter()
        .map(|p| {
            let start = chrono::DateTime::parse_from_rfc3339(&p.start_time)
                .map(|t| t.timestamp_millis() as f64)
                .unwrap_or(0.0);
            let end = chrono::DateTime::parse_from_rfc3339(&p.end_time)
                .map(|t| t.timestamp_millis() as f64)
                .unwrap_or(start);
            (start, end)
        })
        .collect();

    for i in 0..processes.len() {
        for j in (i + 1)..processes.len() {
            let (start_i, end_i) = ranges[i];
            let (start_j, end_j) = ranges[j];

            // Check for overlap
            if start_i < end_j && start_j < end_i {
                processes[i].is_parallel = true;
                processes[j].is_parallel = true;
            }
        }
    }
}

/// Truncate a string to a maximum number of characters.
fn truncate_string(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        let mut end = max_chars;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
