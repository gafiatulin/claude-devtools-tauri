use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::models::domain::{SearchResult, SearchSessionsResult};
use crate::models::jsonl::{ChatHistoryEntry, ContentBlock, StringOrBlocks};

/// Maximum number of matches to collect from a single session file.
const MAX_MATCHES_PER_SESSION: usize = 5;

/// Number of characters of context to include around a match.
const CONTEXT_CHARS: usize = 100;

/// Search across all session JSONL files in a project directory.
///
/// Files are sorted by modification time (most recent first) so that
/// recent sessions are searched before older ones. Scanning stops once
/// `max_results` matches have been collected. If not all sessions were
/// scanned, `is_partial` is set to `true`.
pub fn search_sessions(
    project_dir: &Path,
    project_id: &str,
    query: &str,
    max_results: usize,
) -> SearchSessionsResult {
    let query_lower = query.to_lowercase();

    if query_lower.is_empty() {
        return SearchSessionsResult {
            results: Vec::new(),
            total_matches: 0,
            sessions_searched: 0,
            query: query.to_string(),
            is_partial: Some(false),
        };
    }

    // Collect all session JSONL files (excluding agent_*.jsonl subagent files).
    let session_files = match collect_session_files(project_dir) {
        Ok(files) => files,
        Err(_) => {
            return SearchSessionsResult {
                results: Vec::new(),
                total_matches: 0,
                sessions_searched: 0,
                query: query.to_string(),
                is_partial: Some(false),
            };
        }
    };

    let total_sessions = session_files.len() as u32;

    // Pair each file with its mtime (one stat per file), then sort.
    // This avoids O(N log N) stat calls from the previous sort comparator.
    let mut files_with_mtime: Vec<_> = session_files
        .into_iter()
        .map(|path| {
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (path, mtime)
        })
        .collect();
    files_with_mtime.sort_by(|a, b| b.1.cmp(&a.1));

    let mut results = Vec::new();
    let mut sessions_searched: u32 = 0;

    for (file_path, _) in &files_with_mtime {
        sessions_searched += 1;

        let session_id = extract_session_id(file_path);
        let matches = search_session_file(
            file_path,
            &session_id,
            project_id,
            &query_lower,
            max_results.saturating_sub(results.len()),
        );

        results.extend(matches);

        if results.len() >= max_results {
            break;
        }
    }

    let is_partial = sessions_searched < total_sessions;

    SearchSessionsResult {
        total_matches: results.len() as u32,
        results,
        sessions_searched,
        query: query.to_string(),
        is_partial: Some(is_partial),
    }
}

/// Collect all top-level .jsonl files in the project directory, excluding
/// agent_*.jsonl subagent files. Also recurses one level into session UUID
/// subdirectories to find session JSONL files there, but skips agent files.
fn collect_session_files(project_dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let entries = fs::read_dir(project_dir)
        .map_err(|e| format!("Failed to read project dir: {e}"))?;

    let mut files = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            if file_name.ends_with(".jsonl")
                && !file_name.starts_with("agent_")
                && !file_name.starts_with("agent-")
            {
                files.push(path);
            }
        } else if path.is_dir() {
            // New directory structure: {session_uuid}/session.jsonl
            // Look for non-agent .jsonl files inside session UUID directories.
            if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub_entry in sub_entries.flatten() {
                    let sub_path = sub_entry.path();
                    if sub_path.is_file() {
                        let sub_name = match sub_path.file_name().and_then(|n| n.to_str()) {
                            Some(n) => n,
                            None => continue,
                        };
                        if sub_name.ends_with(".jsonl")
                            && !sub_name.starts_with("agent_")
                            && !sub_name.starts_with("agent-")
                        {
                            files.push(sub_path);
                        }
                    }
                }
            }
        }
    }

    Ok(files)
}

/// Extract the session ID from a JSONL file path.
///
/// For top-level files: `{uuid}.jsonl` -> `{uuid}`
/// For nested files: `{uuid}/something.jsonl` -> `{uuid}`
fn extract_session_id(path: &Path) -> String {
    // First try the file stem (e.g. "abc-def.jsonl" -> "abc-def")
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // If this looks like a nested session dir structure, use the parent dir name
    if let Some(parent) = path.parent() {
        if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
            // If the parent looks like a UUID (contains hyphens and is long), use it
            if parent_name.len() > 30 && parent_name.contains('-') {
                return parent_name.to_string();
            }
        }
    }

    file_stem
}

/// Search a single session JSONL file line by line.
///
/// Parses only user and assistant entries, scanning their text content for
/// case-insensitive matches. Returns up to `max_matches` results.
fn search_session_file(
    path: &Path,
    session_id: &str,
    project_id: &str,
    query_lower: &str,
    max_matches: usize,
) -> Vec<SearchResult> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);
    let mut results = Vec::new();
    let mut session_title = String::new();
    let mut first_user_message_seen = false;

    for line_result in reader.lines() {
        if results.len() >= max_matches.min(MAX_MATCHES_PER_SESSION) {
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse the entry
        let entry: ChatHistoryEntry = match serde_json::from_str(trimmed) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match entry {
            ChatHistoryEntry::User(ref user_entry) => {
                let text = extract_text_from_string_or_blocks(&user_entry.message.content);

                // Capture the first user message as session title.
                if !first_user_message_seen {
                    first_user_message_seen = true;
                    session_title = truncate_string(&text, 120);
                }

                let timestamp = user_entry
                    .common
                    .timestamp
                    .as_deref()
                    .and_then(parse_timestamp_to_epoch)
                    .unwrap_or(0.0);
                let message_uuid = user_entry.common.uuid.clone();

                find_matches_in_text(
                    &text,
                    query_lower,
                    session_id,
                    project_id,
                    &session_title,
                    "user",
                    "user",
                    timestamp,
                    message_uuid.as_deref(),
                    &mut results,
                    max_matches.min(MAX_MATCHES_PER_SESSION),
                );
            }
            ChatHistoryEntry::Assistant(ref asst_entry) => {
                let text = extract_text_from_content_blocks(&asst_entry.message.content);

                let timestamp = asst_entry
                    .common
                    .timestamp
                    .as_deref()
                    .and_then(parse_timestamp_to_epoch)
                    .unwrap_or(0.0);
                let message_uuid = asst_entry.common.uuid.clone();

                find_matches_in_text(
                    &text,
                    query_lower,
                    session_id,
                    project_id,
                    &session_title,
                    "assistant",
                    "ai",
                    timestamp,
                    message_uuid.as_deref(),
                    &mut results,
                    max_matches.min(MAX_MATCHES_PER_SESSION),
                );
            }
            _ => {}
        }
    }

    results
}

/// Find all occurrences of `query_lower` in `text` (case-insensitive) and
/// append `SearchResult` entries to `results`, up to `max` total results.
fn find_matches_in_text(
    text: &str,
    query_lower: &str,
    session_id: &str,
    project_id: &str,
    session_title: &str,
    message_type: &str,
    item_type: &str,
    timestamp: f64,
    message_uuid: Option<&str>,
    results: &mut Vec<SearchResult>,
    max: usize,
) {
    let text_lower = text.to_lowercase();
    let mut search_start = 0;
    let mut match_index: u32 = 0;

    while let Some(pos) = text_lower[search_start..].find(query_lower) {
        if results.len() >= max {
            break;
        }

        let abs_pos = search_start + pos;
        let context = build_context_snippet(text, abs_pos, query_lower.len());
        let matched_text = text[abs_pos..abs_pos + query_lower.len()].to_string();

        results.push(SearchResult {
            session_id: session_id.to_string(),
            project_id: project_id.to_string(),
            session_title: session_title.to_string(),
            matched_text,
            context,
            message_type: message_type.to_string(),
            timestamp,
            group_id: message_uuid.map(|s| s.to_string()),
            item_type: Some(item_type.to_string()),
            match_index_in_item: Some(match_index),
            match_start_offset: Some(abs_pos as u32),
            message_uuid: message_uuid.map(|s| s.to_string()),
        });

        match_index += 1;
        search_start = abs_pos + query_lower.len();
    }
}

/// Build a context snippet of approximately `CONTEXT_CHARS` characters
/// centered around the match position.
fn build_context_snippet(text: &str, match_start: usize, match_len: usize) -> String {
    let half_context = CONTEXT_CHARS / 2;

    let snippet_start = match_start.saturating_sub(half_context);
    let snippet_end = (match_start + match_len + half_context).min(text.len());

    // Adjust to character boundaries (avoid splitting multi-byte chars).
    let snippet_start = text
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= snippet_start)
        .unwrap_or(snippet_start);
    let snippet_end = text
        .char_indices()
        .map(|(i, _)| i)
        .rfind(|&i| i <= snippet_end)
        .map(|i| {
            // Advance past this character to include it.
            i + text[i..].chars().next().map_or(0, |c| c.len_utf8())
        })
        .unwrap_or(snippet_end);

    let mut snippet = String::new();
    if snippet_start > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(&text[snippet_start..snippet_end]);
    if snippet_end < text.len() {
        snippet.push_str("...");
    }

    snippet
}

/// Extract all text content from a `StringOrBlocks` value.
fn extract_text_from_string_or_blocks(content: &StringOrBlocks) -> String {
    match content {
        StringOrBlocks::String(s) => s.clone(),
        StringOrBlocks::Blocks(blocks) => extract_text_from_content_blocks(blocks),
    }
}

/// Extract all text from a slice of `ContentBlock` values, joining text
/// blocks with newlines.
fn extract_text_from_content_blocks(blocks: &[ContentBlock]) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        if let Some(text) = block.as_text() {
            parts.push(text);
        }
    }
    parts.join("\n")
}

/// Parse an ISO 8601 timestamp string to a Unix epoch (seconds) as f64.
fn parse_timestamp_to_epoch(ts: &str) -> Option<f64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis() as f64)
}

/// Truncate a string to at most `max_len` characters, appending "..." if truncated.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context_snippet_short_text() {
        let text = "Hello world";
        let snippet = build_context_snippet(text, 6, 5);
        assert_eq!(snippet, "Hello world");
    }

    #[test]
    fn test_build_context_snippet_long_text() {
        let text = "a".repeat(300);
        let snippet = build_context_snippet(&text, 150, 5);
        assert!(snippet.starts_with("..."));
        assert!(snippet.ends_with("..."));
        // The snippet should be roughly 2 * CONTEXT_CHARS + match_len + ellipsis chars
        assert!(snippet.len() > 50);
    }

    #[test]
    fn test_find_matches_in_text_case_insensitive() {
        let mut results = Vec::new();
        find_matches_in_text(
            "Hello World hello HELLO",
            "hello",
            "session-1",
            "project-1",
            "Test Session",
            "user",
            "user",
            1000.0,
            Some("uuid-1"),
            &mut results,
            10,
        );
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].match_start_offset, Some(0));
        assert_eq!(results[1].match_start_offset, Some(12));
        assert_eq!(results[2].match_start_offset, Some(18));
    }

    #[test]
    fn test_find_matches_respects_max() {
        let mut results = Vec::new();
        find_matches_in_text(
            "aaa aaa aaa aaa aaa",
            "aaa",
            "s1",
            "p1",
            "title",
            "user",
            "user",
            0.0,
            None,
            &mut results,
            2,
        );
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_extract_session_id_from_simple_path() {
        let path = Path::new("/some/dir/abc-def-123.jsonl");
        assert_eq!(extract_session_id(path), "abc-def-123");
    }

    #[test]
    fn test_truncate_string_short() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_string_long() {
        let result = truncate_string("hello world this is long", 10);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 13); // 10 + "..."
    }

    #[test]
    fn test_parse_timestamp_to_epoch() {
        let ts = "2025-01-01T00:00:00Z";
        let epoch = parse_timestamp_to_epoch(ts).unwrap();
        assert!(epoch > 0.0);
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let result = search_sessions(Path::new("/nonexistent"), "proj", "", 10);
        assert!(result.results.is_empty());
        assert_eq!(result.total_matches, 0);
        assert_eq!(result.is_partial, Some(false));
    }

    /// Integration test: search a real project directory if available.
    #[test]
    fn test_search_real_project() {
        let home = std::env::var("HOME").unwrap_or_default();
        let project_dir = std::path::PathBuf::from(&home)
            .join(".claude/projects/-Users-victor-Workspace-personal-claude-devtools-tauri");

        if !project_dir.exists() {
            eprintln!("Skipping real search test: project dir not found");
            return;
        }

        let result = search_sessions(
            &project_dir,
            "-Users-victor-Workspace-personal-claude-devtools-tauri",
            "tauri",
            5,
        );

        eprintln!(
            "Search found {} matches across {} sessions (partial: {:?})",
            result.total_matches, result.sessions_searched, result.is_partial
        );

        for r in &result.results {
            eprintln!(
                "  [{}] {} in session {} (offset {:?})",
                r.message_type, r.matched_text, r.session_id, r.match_start_offset
            );
        }
    }

    /// Live integration test: discover a real project under ~/.claude and search it.
    /// Run with: cargo test test_live_search -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_live_search() {
        let home = dirs::home_dir().unwrap();
        let claude_root = home.join(".claude");
        if !claude_root.exists() {
            println!("No ~/.claude directory, skipping");
            return;
        }

        // Find a project directory
        let projects_dir = claude_root.join("projects");
        if let Ok(mut entries) = std::fs::read_dir(&projects_dir) {
            if let Some(Ok(entry)) = entries.next() {
                let project_dir = entry.path();
                let project_id = project_dir.file_name().unwrap().to_string_lossy().to_string();

                println!("Searching in project: {}", project_id);

                // Count total session files for reporting
                let total_sessions = collect_session_files(&project_dir)
                    .map(|f| f.len())
                    .unwrap_or(0);

                // Search for a common word
                let result = search_sessions(&project_dir, &project_id, "the", 10);
                println!(
                    "Found {} results, is_partial: {:?}",
                    result.results.len(),
                    result.is_partial
                );
                println!(
                    "Sessions searched: {}/{}",
                    result.sessions_searched, total_sessions
                );

                for r in result.results.iter().take(3) {
                    println!("  Match in session {}: ...{}...", r.session_id, r.context);
                }

                assert!(
                    !result.results.is_empty() || total_sessions == 0,
                    "Search returned no results but sessions exist"
                );
            }
        }
    }
}
