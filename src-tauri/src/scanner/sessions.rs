use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use rayon::prelude::*;

use crate::models::chunks::SessionDetail;
use crate::models::domain::{
    PaginatedSessionsResult, Session, SessionCursor, SessionMetadataLevel, SessionMetrics,
    SessionsByIdsOptions, SessionsPaginationOptions,
};

/// Type alias for the session metadata LRU cache.
/// Key: file path, Value: (mtime at cache time, cached Session wrapped in Arc)
pub type SessionCache = Mutex<lru::LruCache<PathBuf, (SystemTime, Arc<Session>)>>;

/// Scan session metadata with LRU cache support.
/// Checks the cache first (using file path + mtime as key). On miss, scans and inserts.
/// Returns an Arc<Session> — cache hits are a cheap pointer copy instead of a deep clone.
pub fn scan_session_metadata_cached(
    path: &Path,
    level: &SessionMetadataLevel,
    project_id: &str,
    project_path: &str,
    cache: &SessionCache,
) -> Result<Arc<Session>, String> {
    // Read metadata once — reuse for cache check AND session building
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Failed to stat {}: {e}", path.display()))?;
    let current_mtime = metadata.modified().ok();

    // Check cache — Arc::clone is a cheap pointer copy
    if let Some(mtime) = current_mtime {
        let path_buf = path.to_path_buf();
        if let Ok(mut cache_guard) = cache.lock() {
            if let Some((cached_mtime, cached_session)) = cache_guard.get(&path_buf) {
                if *cached_mtime == mtime {
                    return Ok(Arc::clone(cached_session));
                }
            }
        }
    }

    // Cache miss: scan from disk, reusing the metadata we already read
    let session = scan_session_metadata_with_meta(
        path, level, project_id, project_path, &metadata, current_mtime,
    )?;

    // Wrap in Arc and insert — no extra clone of Session needed
    let arc = Arc::new(session);
    if let Some(mtime) = current_mtime {
        if let Ok(mut cache_guard) = cache.lock() {
            cache_guard.put(path.to_path_buf(), (mtime, Arc::clone(&arc)));
        }
    }

    Ok(arc)
}

/// Evict all cache entries whose path matches the given path.
pub fn evict_session_cache(cache: &SessionCache, path: &Path) {
    if let Ok(mut cache_guard) = cache.lock() {
        let path_buf = path.to_path_buf();
        cache_guard.pop(&path_buf);
    }
}

/// Scan all sessions in a project directory.
///
/// Returns a list of `Session` objects with deep metadata (parses first
/// few lines of each JSONL file).
pub fn scan_sessions(project_dir: &Path, project_id: &str) -> Result<Vec<Session>, String> {
    scan_sessions_with_cache(project_dir, project_id, None)
}

pub fn scan_sessions_with_cache(
    project_dir: &Path,
    project_id: &str,
    cache: Option<&SessionCache>,
) -> Result<Vec<Session>, String> {
    let project_path = crate::scanner::projects::decode_project_id(project_id);
    let entries = fs::read_dir(project_dir)
        .map_err(|e| format!("Failed to read project dir: {e}"))?;

    // Collect paths first, then scan in parallel
    let paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let file_name = path.file_name()?.to_str()?;
            if !file_name.ends_with(".jsonl") {
                return None;
            }
            if file_name.starts_with("agent_") || file_name.starts_with("agent-") {
                return None;
            }
            Some(path)
        })
        .collect();

    let pool = crate::scanner::scan_pool();
    let mut sessions: Vec<Session> = pool.install(|| {
        paths
            .par_iter()
            .filter_map(|path| {
                if let Some(c) = cache {
                    scan_session_metadata_cached(
                        path,
                        &SessionMetadataLevel::Deep,
                        project_id,
                        &project_path,
                        c,
                    )
                    .ok()
                    .map(Arc::unwrap_or_clone)
                } else {
                    scan_session_metadata(
                        path,
                        &SessionMetadataLevel::Deep,
                        project_id,
                        &project_path,
                    )
                    .ok()
                }
            })
            .collect()
    });

    // Sort by createdAt descending
    sessions.sort_by(|a, b| {
        b.created_at
            .partial_cmp(&a.created_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(sessions)
}

/// Scan metadata for a single session JSONL file.
///
/// - **Light**: Only file metadata (birthtime, size). Fast but no content info.
/// - **Deep**: Parses first N lines to extract firstMessage, hasSubagents,
///   messageCount, isOngoing, gitBranch, and context consumption.
pub fn scan_session_metadata(
    path: &Path,
    level: &SessionMetadataLevel,
    project_id: &str,
    project_path: &str,
) -> Result<Session, String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Failed to stat {}: {e}", path.display()))?;
    let mtime = metadata.modified().ok();
    scan_session_metadata_with_meta(path, level, project_id, project_path, &metadata, mtime)
}

/// Inner implementation that accepts pre-read metadata to avoid redundant stat() calls.
fn scan_session_metadata_with_meta(
    path: &Path,
    level: &SessionMetadataLevel,
    project_id: &str,
    project_path: &str,
    metadata: &fs::Metadata,
    mtime: Option<SystemTime>,
) -> Result<Session, String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let session_id = file_name.trim_end_matches(".jsonl").to_string();

    let created_at = metadata
        .created()
        .or_else(|_| metadata.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);

    // Load todo data if available
    let todo_data = load_todo_data(&session_id);

    if *level == SessionMetadataLevel::Light {
        // Light mode: only file metadata
        let has_subagents = check_has_subagents(path);

        return Ok(Session {
            id: session_id,
            project_id: project_id.to_string(),
            project_path: project_path.to_string(),
            todo_data,
            created_at,
            first_message: None,
            message_timestamp: None,
            has_subagents,
            message_count: 0,
            is_ongoing: None,
            git_branch: None,
            metadata_level: Some(SessionMetadataLevel::Light),
            context_consumption: None,
            compaction_count: None,
            phase_breakdown: None,
            slug: None,
            has_plan_content: None,
        });
    }

    // Deep mode: parse JSONL content, passing mtime to avoid extra stat()
    let deep = parse_session_deep_metadata(path, mtime);
    let has_subagents = check_has_subagents(path);

    Ok(Session {
        id: session_id,
        project_id: project_id.to_string(),
        project_path: project_path.to_string(),
        todo_data,
        created_at,
        first_message: deep.first_message,
        message_timestamp: deep.message_timestamp,
        has_subagents,
        message_count: deep.message_count,
        is_ongoing: Some(deep.is_ongoing),
        git_branch: deep.git_branch,
        metadata_level: Some(SessionMetadataLevel::Deep),
        context_consumption: deep.context_consumption,
        compaction_count: deep.compaction_count,
        phase_breakdown: None,
        slug: deep.slug,
        has_plan_content: if deep.has_plan_content { Some(true) } else { None },
    })
}

/// Check if a session has subagents by looking for:
/// 1. A session subdirectory containing a `subagents/` folder
/// 2. Legacy `agent_*.jsonl` files at project root
fn check_has_subagents(session_path: &Path) -> bool {
    let session_id = session_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if let Some(parent) = session_path.parent() {
        // New structure: {session_uuid}/subagents/
        let subagents_dir = parent.join(session_id).join("subagents");
        if subagents_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&subagents_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("agent-") && name_str.ends_with(".jsonl") {
                        return true;
                    }
                }
            }
        }

        // Legacy structure: agent_*.jsonl at project root
        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if (name_str.starts_with("agent_") || name_str.starts_with("agent-"))
                    && name_str.ends_with(".jsonl")
                    && entry.path().is_file()
                {
                    // For legacy files, we would need to check sessionId inside,
                    // but for a quick check we just return true if any exist.
                    return true;
                }
            }
        }
    }

    false
}

/// Deep metadata extracted from parsing a session JSONL file.
struct DeepMetadata {
    first_message: Option<String>,
    message_timestamp: Option<String>,
    message_count: u32,
    is_ongoing: bool,
    git_branch: Option<String>,
    context_consumption: Option<u64>,
    compaction_count: Option<u32>,
    slug: Option<String>,
    has_plan_content: bool,
}

/// Parse a session JSONL file to extract deep metadata.
///
/// This reads the file line by line, extracting:
/// - First user message text (for display)
/// - First message timestamp
/// - Total message count
/// - Whether the session appears ongoing
/// - Git branch from earliest entry
/// - Context consumption from usage metadata
/// - Compaction count from summary entries
///
/// If `mtime` is provided, it is reused for the "is ongoing" check instead of
/// issuing an extra `metadata()` syscall.
fn parse_session_deep_metadata(path: &Path, mtime: Option<SystemTime>) -> DeepMetadata {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return DeepMetadata {
                first_message: None,
                message_timestamp: None,
                message_count: 0,
                is_ongoing: false,
                git_branch: None,
                context_consumption: None,
                compaction_count: None,
                slug: None,
                has_plan_content: false,
            };
        }
    };

    let reader = BufReader::new(file);
    let mut first_message: Option<String> = None;
    let mut message_timestamp: Option<String> = None;
    let mut git_branch: Option<String> = None;
    let mut message_count: u32 = 0;
    // Track last entry type as a simple enum to avoid String allocations per line
    let mut last_entry_type: u8 = 0; // 0=none, 1=user, 2=assistant, 3=summary, 4=other
    let mut last_user_is_genuine: bool = false;
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut compaction_count: u32 = 0;
    let mut slug: Option<String> = None;
    let mut has_plan_content: bool = false;

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Quick parse: use serde_json::Value for flexible access
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");
        message_count += 1;

        // Extract slug from first entry that has one (present on all ConversationalFields entries)
        if slug.is_none() {
            slug = value
                .get("slug")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
        }

        match entry_type {
            "user" => {
                last_entry_type = 1; // user

                // Determine if this is a genuine user prompt (not command output or meta).
                // isMeta:true → tool result; content with only <local-command-stdout> → command output.
                let is_meta = value
                    .get("isMeta")
                    .and_then(|m| m.as_bool())
                    .unwrap_or(false);
                let content_str = value
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                let is_command_output = content_str.contains("<local-command-stdout>")
                    || content_str.contains("<local-command-stderr>")
                    || content_str.contains("<local-command-caveat>")
                    || content_str.contains("<command-name>")
                    || content_str.contains("<system-reminder>");
                last_user_is_genuine = !is_meta && !is_command_output;

                // Check if first non-meta user entry has planContent (plan chain)
                if !has_plan_content && !is_meta {
                    has_plan_content = value.get("planContent").is_some();
                }

                // Extract git branch from the first entry that has one
                if git_branch.is_none() {
                    git_branch = value
                        .get("gitBranch")
                        .and_then(|b| b.as_str())
                        .filter(|b| !b.is_empty())
                        .map(|b| b.to_string());
                }

                // Extract first user message text
                if first_message.is_none() && !is_meta {
                    if let Some(msg) = value.get("message") {
                        first_message = extract_message_text(msg);
                        message_timestamp = value
                            .get("timestamp")
                            .and_then(|t| t.as_str())
                            .map(|t| t.to_string());
                    }
                }
            }
            "assistant" => {
                last_entry_type = 2; // assistant
                last_user_is_genuine = false;

                // Accumulate token usage
                if let Some(msg) = value.get("message") {
                    if let Some(usage) = msg.get("usage") {
                        total_input_tokens += usage
                            .get("input_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0);
                        total_output_tokens += usage
                            .get("output_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0);
                    }
                }

                // Extract git branch if not yet found
                if git_branch.is_none() {
                    git_branch = value
                        .get("gitBranch")
                        .and_then(|b| b.as_str())
                        .filter(|b| !b.is_empty())
                        .map(|b| b.to_string());
                }
            }
            "summary" => {
                last_entry_type = 3; // summary
                last_user_is_genuine = false;
                compaction_count += 1;
            }
            _ => {
                last_entry_type = 4; // other
                last_user_is_genuine = false;
            }
        }
    }

    // A session is "ongoing" if its file was modified within the last 5 minutes.
    // This means Claude is (or very recently was) actively writing to it.
    //
    // We no longer rely solely on last_entry_type == "user" because:
    // - The last entry flips to "assistant" the moment Claude finishes a turn,
    //   making the green dot vanish even while the user is still interacting.
    // - Array-format user messages (e.g. "[Request interrupted by user]") are
    //   indistinguishable from genuine prompts via content inspection alone.
    //
    // The last_user_is_genuine check is still used to suppress false positives
    // for dead sessions whose last user entry is command/system output AND whose
    // file somehow appears recent (e.g. due to filesystem timestamps).
    // Reuse the mtime passed in from the caller to avoid an extra stat() syscall.
    // Fall back to reading metadata if no mtime was provided.
    let resolved_mtime = mtime.or_else(|| {
        fs::metadata(path).and_then(|m| m.modified()).ok()
    });
    let recently_modified = resolved_mtime
        .and_then(|mt| SystemTime::now().duration_since(mt).ok())
        .map(|age| age.as_secs() < 300) // 5 minutes
        .unwrap_or(false);

    let is_ongoing = recently_modified
        && (last_user_is_genuine || last_entry_type != 1);

    let context_consumption = if total_input_tokens > 0 || total_output_tokens > 0 {
        Some(total_input_tokens + total_output_tokens)
    } else {
        None
    };

    DeepMetadata {
        first_message,
        message_timestamp,
        message_count,
        is_ongoing,
        git_branch,
        context_consumption,
        slug,
        has_plan_content,
        compaction_count: if compaction_count > 0 {
            Some(compaction_count)
        } else {
            None
        },
    }
}

/// Strip XML-like tags (e.g. `<command-name>`, `<command-arguments>`) from a string,
/// collapsing resulting whitespace. Used to clean up slash command messages.
fn strip_xml_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    // Collapse multiple whitespace into a single space and trim
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract text content from a message JSON value.
fn extract_message_text(msg: &serde_json::Value) -> Option<String> {
    let content = msg.get("content")?;

    match content {
        serde_json::Value::String(s) => {
            let cleaned = strip_xml_tags(s);
            let trimmed = cleaned.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(truncate_string(&trimmed, 200))
            }
        }
        serde_json::Value::Array(blocks) => {
            for block in blocks {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if block_type == "text" {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        let cleaned = strip_xml_tags(text);
                        let trimmed = cleaned.trim().to_string();
                        if !trimmed.is_empty() {
                            return Some(truncate_string(&trimmed, 200));
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Truncate a string to a maximum number of characters.
fn truncate_string(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        let mut end = max_chars;
        // Don't cut in the middle of a multi-byte character
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Load todo data from `~/.claude/todos/{session_id}.json` if it exists.
fn load_todo_data(session_id: &str) -> Option<serde_json::Value> {
    let home = dirs::home_dir()?;
    let todos_dir = home.join(".claude").join("todos");

    // Try exact match first: {session_id}.json
    let todo_path = todos_dir.join(format!("{session_id}.json"));
    if todo_path.is_file() {
        if let Ok(content) = fs::read_to_string(&todo_path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                return Some(value);
            }
        }
    }

    // Also look for agent-prefixed todo files: {session_id}-agent-*.json
    if let Ok(entries) = fs::read_dir(&todos_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(session_id) && name_str.ends_with(".json") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                        return Some(value);
                    }
                }
            }
        }
    }

    None
}

/// Paginated session listing with cursor-based pagination.
///
/// Sessions are sorted by `createdAt` descending. The cursor is a composite
/// of `(timestamp, sessionId)` encoded as a JSON string.
pub fn scan_sessions_paginated(
    project_dir: &Path,
    project_id: &str,
    cursor: Option<&str>,
    limit: usize,
    options: Option<&SessionsPaginationOptions>,
) -> Result<PaginatedSessionsResult, String> {
    scan_sessions_paginated_with_cache(project_dir, project_id, cursor, limit, options, None)
}

pub fn scan_sessions_paginated_with_cache(
    project_dir: &Path,
    project_id: &str,
    cursor: Option<&str>,
    limit: usize,
    options: Option<&SessionsPaginationOptions>,
    cache: Option<&SessionCache>,
) -> Result<PaginatedSessionsResult, String> {
    let project_path = crate::scanner::projects::decode_project_id(project_id);
    let metadata_level = options
        .and_then(|o| o.metadata_level.as_ref())
        .unwrap_or(&SessionMetadataLevel::Deep);
    let include_total_count = options
        .and_then(|o| o.include_total_count)
        .unwrap_or(true);

    // Collect all session file metadata for sorting
    let mut session_entries = collect_session_entries(project_dir)?;

    // Sort by createdAt descending (most recent first)
    session_entries.sort_by(|a, b| {
        b.created_at_ms
            .partial_cmp(&a.created_at_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_count = if include_total_count {
        session_entries.len() as u32
    } else {
        0
    };

    // Apply cursor filter
    let start_index = if let Some(cursor_str) = cursor {
        if let Ok(cursor_val) = serde_json::from_str::<SessionCursor>(cursor_str) {
            session_entries
                .iter()
                .position(|e| {
                    e.created_at_ms < cursor_val.timestamp
                        || (e.created_at_ms == cursor_val.timestamp
                            && e.session_id <= cursor_val.session_id)
                })
                .unwrap_or(session_entries.len())
        } else {
            0
        }
    } else {
        0
    };

    let page_entries = &session_entries[start_index..];
    let has_more = page_entries.len() > limit;
    let page_entries = &page_entries[..page_entries.len().min(limit)];

    // Build Session objects for this page (in parallel)
    let pool = crate::scanner::scan_pool();
    let sessions: Vec<Session> = pool.install(|| {
        page_entries
            .par_iter()
            .filter_map(|entry| {
                let result = if let Some(c) = cache {
                    scan_session_metadata_cached(
                        &entry.path,
                        metadata_level,
                        project_id,
                        &project_path,
                        c,
                    )
                    .map(Arc::unwrap_or_clone)
                } else {
                    scan_session_metadata(&entry.path, metadata_level, project_id, &project_path)
                };
                match result {
                    Ok(s) => Some(s),
                    Err(e) => {
                        eprintln!("[scanner] Warning: skipping session {}: {e}", entry.session_id);
                        None
                    }
                }
            })
            .collect()
    });

    // Build next cursor
    let next_cursor = if has_more {
        if let Some(last) = sessions.last() {
            let cursor = SessionCursor {
                timestamp: last.created_at,
                session_id: last.id.clone(),
            };
            Some(serde_json::to_string(&cursor).unwrap_or_default())
        } else {
            None
        }
    } else {
        None
    };

    Ok(PaginatedSessionsResult {
        sessions,
        next_cursor,
        has_more,
        total_count,
    })
}

/// Lightweight entry used for sorting/filtering before full metadata parse.
struct SessionFileEntry {
    session_id: String,
    path: std::path::PathBuf,
    created_at_ms: f64,
}

/// Collect all session file entries from a project directory.
fn collect_session_entries(project_dir: &Path) -> Result<Vec<SessionFileEntry>, String> {
    let entries = fs::read_dir(project_dir)
        .map_err(|e| format!("Failed to read project dir: {e}"))?;

    let mut result = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if !file_name.ends_with(".jsonl") {
            continue;
        }

        if file_name.starts_with("agent_") || file_name.starts_with("agent-") {
            continue;
        }

        let session_id = file_name.trim_end_matches(".jsonl").to_string();

        // Reuse the DirEntry's metadata to avoid an extra stat() syscall
        let created_at_ms = entry
            .metadata()
            .ok()
            .and_then(|m| m.created().or_else(|_| m.modified()).ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        result.push(SessionFileEntry {
            session_id,
            path,
            created_at_ms,
        });
    }

    Ok(result)
}

/// Fetch specific sessions by their IDs.
pub fn get_sessions_by_ids(
    project_dir: &Path,
    project_id: &str,
    session_ids: &[String],
    options: Option<&SessionsByIdsOptions>,
    cache: Option<&SessionCache>,
) -> Result<Vec<Session>, String> {
    let project_path = crate::scanner::projects::decode_project_id(project_id);
    let metadata_level = options
        .and_then(|o| o.metadata_level.as_ref())
        .unwrap_or(&SessionMetadataLevel::Deep);

    // Build (path, exists) list first so par_iter can process them
    let paths: Vec<_> = session_ids
        .iter()
        .filter_map(|session_id| {
            let path = project_dir.join(format!("{session_id}.jsonl"));
            if path.is_file() { Some(path) } else { None }
        })
        .collect();

    let pool = crate::scanner::scan_pool();
    let sessions: Vec<Session> = pool.install(|| {
        paths
            .par_iter()
            .filter_map(|path| {
                if let Some(c) = cache {
                    scan_session_metadata_cached(
                        path,
                        metadata_level,
                        project_id,
                        &project_path,
                        c,
                    )
                    .ok()
                    .map(Arc::unwrap_or_clone)
                } else {
                    scan_session_metadata(path, metadata_level, project_id, &project_path).ok()
                }
            })
            .collect()
    });

    Ok(sessions)
}

/// Build a full session detail by parsing the entire JSONL file.
///
/// This produces messages, chunks, processes, and metrics.
/// Note: Chunk building and semantic extraction are Phase 4 concerns;
/// this function parses messages and processes for now.
pub fn scan_session_detail(
    project_dir: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<SessionDetail, String> {
    let project_path = crate::scanner::projects::decode_project_id(project_id);
    let session_path = project_dir.join(format!("{session_id}.jsonl"));

    if !session_path.is_file() {
        return Err(format!("Session file not found: {}", session_path.display()));
    }

    // Parse the JSONL file
    let entries = crate::jsonl::reader::read_jsonl_file(&session_path)?;
    let progress_map = crate::jsonl::parser::extract_progress_map(&entries);
    let messages = crate::jsonl::parser::parse_entries_to_messages(entries);

    // Scan subagents
    let processes = crate::scanner::subagents::scan_subagents(project_dir, session_id)?;

    // Build session metadata
    let session_meta = scan_session_metadata(
        &session_path,
        &SessionMetadataLevel::Deep,
        project_id,
        &project_path,
    )?;

    // Build and enhance chunks from parsed messages and processes
    let raw_chunks =
        crate::parser::chunk_builder::build_chunks(&messages, &processes, &progress_map);
    let chunks = crate::parser::chunk_builder::enhance_chunks(raw_chunks, &messages);

    // Derive session metrics from chunk metrics to avoid a separate pass over messages.
    let metrics = derive_metrics_from_chunks(&chunks, messages.len() as u32);

    Ok(SessionDetail {
        session: session_meta,
        messages,
        chunks,
        processes,
        metrics,
    })
}

/// Derive session metrics from enhanced chunks, avoiding a separate pass over messages.
fn derive_metrics_from_chunks(
    chunks: &[crate::models::chunks::EnhancedChunk],
    message_count: u32,
) -> SessionMetrics {
    use crate::models::chunks::EnhancedChunk;

    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_creation_tokens: u64 = 0;
    let mut first_time: Option<&str> = None;
    let mut last_time: Option<&str> = None;

    for chunk in chunks {
        let base = match chunk {
            EnhancedChunk::User(d) => &d.base,
            EnhancedChunk::Ai(d) => &d.base,
            EnhancedChunk::System(d) => &d.base,
            EnhancedChunk::Compact(d) => &d.base,
        };

        input_tokens += base.metrics.input_tokens;
        output_tokens += base.metrics.output_tokens;
        cache_read_tokens += base.metrics.cache_read_tokens;
        cache_creation_tokens += base.metrics.cache_creation_tokens;

        if first_time.is_none() {
            first_time = Some(&base.start_time);
        }
        last_time = Some(&base.end_time);
    }

    let duration_ms = match (first_time, last_time) {
        (Some(s), Some(e)) => {
            let start = chrono::DateTime::parse_from_rfc3339(s);
            let end = chrono::DateTime::parse_from_rfc3339(e);
            match (start, end) {
                (Ok(s), Ok(e)) => (e.timestamp_millis() - s.timestamp_millis()).max(0) as f64,
                _ => 0.0,
            }
        }
        _ => 0.0,
    };

    SessionMetrics {
        duration_ms,
        total_tokens: input_tokens + output_tokens,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        message_count,
        cost_usd: None,
    }
}

/// Compute session metrics from parsed messages.
pub fn compute_session_metrics(
    messages: &[crate::models::messages::ParsedMessage],
) -> SessionMetrics {
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_creation_tokens: u64 = 0;
    let mut first_timestamp: Option<f64> = None;
    let mut last_timestamp: Option<f64> = None;

    for msg in messages {
        if let Some(usage) = &msg.usage {
            input_tokens += usage.input_tokens;
            output_tokens += usage.output_tokens;
            cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
            cache_creation_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
        }

        // Parse timestamp for duration calculation
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&msg.timestamp) {
            let ms = ts.timestamp_millis() as f64;
            first_timestamp = Some(first_timestamp.map_or(ms, |f: f64| f.min(ms)));
            last_timestamp = Some(last_timestamp.map_or(ms, |l: f64| l.max(ms)));
        }
    }

    let duration_ms = match (first_timestamp, last_timestamp) {
        (Some(first), Some(last)) => (last - first).max(0.0),
        _ => 0.0,
    };

    let total_tokens = input_tokens + output_tokens;

    SessionMetrics {
        duration_ms,
        total_tokens,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        message_count: messages.len() as u32,
        cost_usd: None,
    }
}

/// Type alias for the session metrics LRU cache.
pub type MetricsCache = Mutex<lru::LruCache<PathBuf, (SystemTime, SessionMetrics)>>;

/// Get session metrics without building full session detail.
/// Uses the metrics cache when available to avoid redundant JSONL parsing.
pub fn get_session_metrics(
    project_dir: &Path,
    session_id: &str,
    metrics_cache: Option<&MetricsCache>,
) -> Result<SessionMetrics, String> {
    let session_path = project_dir.join(format!("{session_id}.jsonl"));

    if !session_path.is_file() {
        return Err(format!("Session file not found: {}", session_path.display()));
    }

    let current_mtime = std::fs::metadata(&session_path)
        .and_then(|m| m.modified())
        .ok();

    // Check metrics cache
    if let (Some(cache), Some(mtime)) = (metrics_cache, current_mtime) {
        if let Ok(mut guard) = cache.lock() {
            if let Some((cached_mtime, cached_metrics)) = guard.get(&session_path) {
                if *cached_mtime == mtime {
                    return Ok(cached_metrics.clone());
                }
            }
        }
    }

    let entries = crate::jsonl::reader::read_jsonl_file(&session_path)?;
    let messages = crate::jsonl::parser::parse_entries_to_messages(entries);
    let metrics = compute_session_metrics(&messages);

    // Store in cache
    if let (Some(cache), Some(mtime)) = (metrics_cache, current_mtime) {
        if let Ok(mut guard) = cache.lock() {
            guard.put(session_path, (mtime, metrics.clone()));
        }
    }

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live integration test: scan actual ~/.claude projects and sessions.
    ///
    /// Run with: cargo test test_live_scan_projects -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_live_scan_projects() {
        let home = dirs::home_dir().unwrap();
        let claude_root = home.join(".claude");
        if !claude_root.exists() {
            println!("No ~/.claude directory, skipping");
            return;
        }

        let projects =
            crate::scanner::projects::scan_projects(&claude_root).unwrap();
        println!("Found {} projects", projects.len());
        for p in projects.iter().take(3) {
            println!(
                "  Project: {} (path: {}, sessions: {})",
                p.id,
                p.path,
                p.sessions.len()
            );
        }
        assert!(!projects.is_empty(), "Expected at least one project");
    }

    /// Live integration test: deep-scan a real session file.
    ///
    /// Run with: cargo test test_live_scan_session_deep -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_live_scan_session_deep() {
        let home = dirs::home_dir().unwrap();
        let claude_root = home.join(".claude");
        if !claude_root.exists() {
            println!("No ~/.claude directory, skipping");
            return;
        }

        let projects =
            crate::scanner::projects::scan_projects(&claude_root).unwrap();
        println!("Found {} projects total", projects.len());

        // Find first project that has at least one session
        let project = match projects.iter().find(|p| !p.sessions.is_empty()) {
            Some(p) => p,
            None => {
                println!("No projects with sessions found, skipping");
                return;
            }
        };

        println!(
            "Using project: {} ({} sessions)",
            project.name,
            project.sessions.len()
        );

        // Deep-scan the first session
        let project_dir = claude_root.join("projects").join(&project.id);
        let session_id = &project.sessions[0];
        let session_path = project_dir.join(format!("{session_id}.jsonl"));

        let session = scan_session_metadata(
            &session_path,
            &SessionMetadataLevel::Deep,
            &project.id,
            &project.path,
        )
        .unwrap();

        println!("Session: {}", session.id);
        println!("  messageCount: {}", session.message_count);
        println!(
            "  firstMessage: {}",
            session
                .first_message
                .as_deref()
                .map(|m| if m.len() > 80 {
                    format!("{}...", &m[..80])
                } else {
                    m.to_string()
                })
                .unwrap_or_else(|| "(none)".to_string())
        );
        println!(
            "  gitBranch: {}",
            session.git_branch.as_deref().unwrap_or("(none)")
        );
        println!(
            "  isOngoing: {}",
            session
                .is_ongoing
                .map(|b| b.to_string())
                .unwrap_or_else(|| "(unknown)".to_string())
        );
        println!(
            "  contextConsumption: {}",
            session
                .context_consumption
                .map(|c| format!("{} tokens", c))
                .unwrap_or_else(|| "(none)".to_string())
        );
        println!(
            "  compactionCount: {}",
            session
                .compaction_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "0".to_string())
        );
        println!(
            "  hasSubagents: {}",
            session.has_subagents
        );

        assert!(session.message_count > 0, "Expected at least one message");
    }

    /// Benchmark test: finds the largest .jsonl session file in ~/.claude/projects/
    /// and measures JSONL parse + message conversion + chunk building time.
    ///
    /// Run with: cargo test test_benchmark_scan_large_session -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_benchmark_scan_large_session() {
        let home = dirs::home_dir().unwrap();
        let claude_root = home.join(".claude");
        if !claude_root.exists() {
            println!("No ~/.claude directory, skipping");
            return;
        }

        let projects_dir = claude_root.join("projects");
        let mut largest_file: Option<(std::path::PathBuf, u64)> = None;

        // Find the largest .jsonl file across all projects
        if let Ok(projects) = std::fs::read_dir(&projects_dir) {
            for project_entry in projects.flatten() {
                if let Ok(sessions) = std::fs::read_dir(project_entry.path()) {
                    for session_entry in sessions.flatten() {
                        let path = session_entry.path();
                        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                            // Skip subagent files
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            if name.starts_with("agent_") || name.starts_with("agent-") {
                                continue;
                            }
                            if let Ok(meta) = std::fs::metadata(&path) {
                                let size = meta.len();
                                if largest_file.as_ref().map(|(_, s)| size > *s).unwrap_or(true) {
                                    largest_file = Some((path, size));
                                }
                            }
                        }
                    }
                }
            }
        }

        let (path, size) = match largest_file {
            Some(f) => f,
            None => {
                println!("No .jsonl files found in ~/.claude/projects/");
                return;
            }
        };

        println!(
            "Largest session: {:?} ({} KB, {:.1} MB)",
            path,
            size / 1024,
            size as f64 / 1024.0 / 1024.0
        );

        // Phase 1: JSONL read + parse
        let start = std::time::Instant::now();
        let entries = crate::jsonl::reader::read_jsonl_file(&path).unwrap();
        let read_time = start.elapsed();
        println!("  Read {} JSONL entries in {:?}", entries.len(), read_time);

        // Phase 2: Convert entries to ParsedMessages
        let start = std::time::Instant::now();
        let messages = crate::jsonl::parser::parse_entries_to_messages(entries);
        let convert_time = start.elapsed();
        println!(
            "  Converted to {} ParsedMessages in {:?}",
            messages.len(),
            convert_time
        );

        // Phase 3: Build chunks
        let start = std::time::Instant::now();
        let chunks =
            crate::parser::chunk_builder::build_chunks(&messages, &[], &std::collections::HashMap::new());
        let chunk_time = start.elapsed();
        println!("  Built {} chunks in {:?}", chunks.len(), chunk_time);

        // Phase 4: Compute metrics
        let start = std::time::Instant::now();
        let metrics = compute_session_metrics(&messages);
        let metrics_time = start.elapsed();
        println!("  Computed metrics in {:?}", metrics_time);

        let total_time = read_time + convert_time + chunk_time + metrics_time;
        println!("  ---");
        println!("  Total pipeline: {:?}", total_time);
        println!(
            "  Metrics: {} total tokens, {} messages, {:.1}s duration",
            metrics.total_tokens,
            metrics.message_count,
            metrics.duration_ms / 1000.0
        );

        assert!(
            total_time.as_secs() < 10,
            "Full pipeline took too long: {:?}",
            total_time
        );
    }

    #[test]
    fn test_session_cache_hit_returns_cached_data() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_dir = tmp.path();
        let session_path = project_dir.join("test-session.jsonl");

        // Write a minimal JSONL file
        let jsonl = r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"test-session","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hello world"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#;
        std::fs::write(&session_path, jsonl).unwrap();

        let cache: SessionCache = Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(10).unwrap(),
        ));

        // First call: cache miss, should read from disk
        let result1 = scan_session_metadata_cached(
            &session_path,
            &SessionMetadataLevel::Deep,
            "test-project",
            "/tmp/project",
            &cache,
        )
        .unwrap();

        assert_eq!(result1.id, "test-session");
        assert_eq!(result1.message_count, 1);

        // Verify it's in the cache
        {
            let guard = cache.lock().unwrap();
            assert_eq!(guard.len(), 1);
        }

        // Second call: cache hit (same mtime)
        let result2 = scan_session_metadata_cached(
            &session_path,
            &SessionMetadataLevel::Deep,
            "test-project",
            "/tmp/project",
            &cache,
        )
        .unwrap();

        assert_eq!(result2.id, result1.id);
        assert_eq!(result2.message_count, result1.message_count);
    }

    #[test]
    fn test_session_cache_eviction_on_file_change() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_path = tmp.path().join("test-session.jsonl");
        std::fs::write(&session_path, r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hello"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#).unwrap();

        let cache: SessionCache = Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(10).unwrap(),
        ));

        // Populate cache
        scan_session_metadata_cached(
            &session_path,
            &SessionMetadataLevel::Deep,
            "p1",
            "/tmp",
            &cache,
        )
        .unwrap();

        assert_eq!(cache.lock().unwrap().len(), 1);

        // Evict
        evict_session_cache(&cache, &session_path);

        assert_eq!(cache.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_session_cache_invalidates_on_mtime_change() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_path = tmp.path().join("test-session.jsonl");
        std::fs::write(&session_path, r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hello"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#).unwrap();

        let cache: SessionCache = Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(10).unwrap(),
        ));

        // Populate cache
        let result1 = scan_session_metadata_cached(
            &session_path,
            &SessionMetadataLevel::Deep,
            "p1",
            "/tmp",
            &cache,
        )
        .unwrap();
        assert_eq!(result1.message_count, 1);

        // Wait a bit and modify file (adding a second line)
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut content = std::fs::read_to_string(&session_path).unwrap();
        content.push_str("\n");
        content.push_str(r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"world"},"timestamp":"2025-01-01T00:00:01Z","uuid":"u2"}"#);
        std::fs::write(&session_path, content).unwrap();

        // Re-read: should get fresh data since mtime changed
        let result2 = scan_session_metadata_cached(
            &session_path,
            &SessionMetadataLevel::Deep,
            "p1",
            "/tmp",
            &cache,
        )
        .unwrap();
        assert_eq!(result2.message_count, 2);
    }
}
