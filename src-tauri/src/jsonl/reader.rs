use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::models::jsonl::ChatHistoryEntry;

/// A lazy iterator over a JSONL file that parses one entry per call to `next()`.
///
/// Malformed lines are skipped with a warning rather than failing iteration.
/// Empty lines, trailing newlines, and BOM markers are handled gracefully.
///
/// Use `read_jsonl_iter()` to construct. Use `read_jsonl_file()` if you need
/// all entries collected into a `Vec` (required when downstream code needs
/// multiple passes, e.g. compact-boundary detection).
pub struct JsonlIterator {
    reader: BufReader<File>,
    path_display: String,
    line_num: usize,
    is_first_line: bool,
}

impl Iterator for JsonlIterator {
    type Item = ChatHistoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => return None, // EOF
                Ok(_) => {
                    self.line_num += 1;

                    // Strip BOM from first line if present (in-place via drain to avoid reallocation)
                    if self.is_first_line {
                        self.is_first_line = false;
                        if line.starts_with('\u{feff}') {
                            line.drain(..'\u{feff}'.len_utf8());
                        }
                    }

                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match super::parser::parse_entry(trimmed) {
                        Ok(entry) => return Some(entry),
                        Err(e) => {
                            eprintln!(
                                "[jsonl] Warning: skipping malformed line {} of {}: {e}",
                                self.line_num, self.path_display
                            );
                            continue;
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[jsonl] Warning: failed to read line {} of {}: {e}",
                        self.line_num + 1,
                        self.path_display
                    );
                    return None;
                }
            }
        }
    }
}

/// Open a JSONL file and return a lazy iterator that parses one entry per call.
///
/// The file is opened eagerly; parsing is deferred until `next()` is called.
/// Prefer this over `read_jsonl_file()` when you can process entries one at a
/// time, to avoid allocating the full `Vec<ChatHistoryEntry>` upfront.
///
/// Note: `parse_entries_to_messages()` still requires a collected `Vec` because
/// it needs two passes (compact-boundary UUID detection then message building).
/// In that case use `read_jsonl_file()` which calls `collect()` internally.
pub fn read_jsonl_iter(path: &Path) -> Result<JsonlIterator, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    Ok(JsonlIterator {
        reader: BufReader::new(file),
        path_display: path.display().to_string(),
        line_num: 0,
        is_first_line: true,
    })
}

/// Read a JSONL file and collect all entries into a `Vec<ChatHistoryEntry>`.
///
/// Malformed lines are skipped with a warning rather than failing the entire file.
/// This is a convenience wrapper around `read_jsonl_iter().collect()`.
pub fn read_jsonl_file(path: &Path) -> Result<Vec<ChatHistoryEntry>, String> {
    Ok(read_jsonl_iter(path)?.collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Parse individual JSONL lines covering all known entry types.
    #[test]
    fn test_parse_known_entry_types() {
        let cases = vec![
            // user entry
            r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hello"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#,
            // assistant entry
            r#"{"type":"assistant","parentUuid":"u1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1","gitBranch":"main","message":{"role":"assistant","model":"claude","id":"m1","type":"message","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5}},"requestId":"r1","timestamp":"2025-01-01T00:00:01Z","uuid":"a1"}"#,
            // system entry (local_command subtype - no durationMs)
            r#"{"type":"system","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1","gitBranch":"main","subtype":"local_command","isMeta":false,"content":"<local-command-stdout>ok</local-command-stdout>","level":"info","timestamp":"2025-01-01T00:00:02Z","uuid":"s1"}"#,
            // system entry (turn_duration subtype - has durationMs)
            r#"{"type":"system","parentUuid":"u1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1","gitBranch":"main","subtype":"turn_duration","durationMs":1234.5,"isMeta":true,"timestamp":"2025-01-01T00:00:03Z","uuid":"s2"}"#,
            // file-history-snapshot
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{"messageId":"m1","trackedFileBackups":{},"timestamp":"2025-01-01T00:00:00Z"},"isSnapshotUpdate":false}"#,
            // queue-operation
            r#"{"type":"queue-operation","timestamp":"2025-01-01T00:00:04Z","uuid":"q1","operation":"enqueue"}"#,
            // progress (unknown type - should parse as Unknown)
            r#"{"type":"progress","parentUuid":"u1","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"abc","version":"2.1","gitBranch":"main","data":{"type":"bash_progress"}}"#,
        ];

        for (i, case) in cases.iter().enumerate() {
            let result = super::super::parser::parse_entry(case);
            assert!(result.is_ok(), "Failed to parse case {}: {:?}", i, result.err());
        }

        // Verify the progress entry is parsed as Unknown
        let progress = super::super::parser::parse_entry(cases[6]).unwrap();
        assert!(matches!(progress, crate::models::jsonl::ChatHistoryEntry::Unknown));
    }

    /// Real-world: file-history-snapshot with object values in trackedFileBackups.
    #[test]
    fn test_parse_file_history_snapshot_with_object_backups() {
        let json = r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{"messageId":"m1","trackedFileBackups":{"/some/file.md":{"backupFileName":null,"version":1,"backupTime":"2026-02-17T01:29:34.463Z"}},"timestamp":"2026-02-17T01:21:09.624Z"},"isSnapshotUpdate":true}"#;
        let result = super::super::parser::parse_entry(json);
        assert!(result.is_ok(), "file-history-snapshot with object backup values failed: {:?}", result.err());
        let msgs = super::super::parser::parse_entries_to_messages(vec![result.unwrap()]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, crate::models::domain::MessageType::FileHistorySnapshot);
    }

    /// Real-world: user message content containing a "document" block (e.g., PDF).
    #[test]
    fn test_parse_user_message_with_document_content_block() {
        let json = r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":[{"type":"document","source":{"type":"base64","media_type":"application/pdf","data":"AAAA"}}]},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#;
        let result = super::super::parser::parse_entry(json);
        assert!(result.is_ok(), "user message with document block failed: {:?}", result.err());
        let msgs = super::super::parser::parse_entries_to_messages(vec![result.unwrap()]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, crate::models::domain::MessageType::User);
    }

    /// Test that parse_entries_to_messages filters out Unknown entries.
    #[test]
    fn test_parse_entries_filters_unknown() {
        let entries = vec![
            crate::models::jsonl::ChatHistoryEntry::Unknown,
            crate::models::jsonl::ChatHistoryEntry::Unknown,
        ];
        let messages = super::super::parser::parse_entries_to_messages(entries);
        assert!(messages.is_empty());
    }

    /// Test that read_jsonl_iter lazily parses entries one at a time.
    #[test]
    fn test_jsonl_iterator_collects_same_as_read_file() {
        // Write a temp JSONL file and verify iterator == read_jsonl_file
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsonl_iter.jsonl");

        let lines = [
            r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"hello"},"timestamp":"2025-01-01T00:00:00Z","uuid":"u1"}"#,
            r#"{"type":"progress","data":{}}"#, // unknown — skipped
            r#"{"type":"user","parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"2.1","gitBranch":"main","message":{"role":"user","content":"world"},"timestamp":"2025-01-01T00:00:01Z","uuid":"u2"}"#,
        ];
        {
            let mut f = std::fs::File::create(&path).unwrap();
            for line in &lines {
                writeln!(f, "{line}").unwrap();
            }
        }

        let via_iter: Vec<_> = read_jsonl_iter(&path).unwrap().collect();
        let via_file = read_jsonl_file(&path).unwrap();

        assert_eq!(via_iter.len(), via_file.len());
        // Both should include Unknown for the progress line
        assert_eq!(via_iter.len(), 3);

        std::fs::remove_file(&path).ok();
    }

    /// Integration test: parse a real JSONL file from ~/.claude if available.
    #[test]
    fn test_parse_real_jsonl_file() {
        let home = std::env::var("HOME").unwrap_or_default();
        let project_dir = PathBuf::from(&home)
            .join(".claude/projects/-Users-victor-Workspace-personal-claude-devtools-tauri");

        if !project_dir.exists() {
            eprintln!("Skipping real file test: project dir not found");
            return;
        }

        // Find the first .jsonl file
        let jsonl_file = std::fs::read_dir(&project_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .map(|e| e.path());

        let Some(path) = jsonl_file else {
            eprintln!("Skipping real file test: no .jsonl files found");
            return;
        };

        let entries = read_jsonl_file(&path).expect("Failed to parse real JSONL file");
        assert!(!entries.is_empty(), "No entries parsed from real file");

        // Verify iterator produces same count
        let iter_count = read_jsonl_iter(&path).unwrap().count();
        assert_eq!(iter_count, entries.len(), "Iterator and read_jsonl_file disagree on entry count");

        // Convert to messages
        let messages = super::super::parser::parse_entries_to_messages(entries);
        assert!(!messages.is_empty(), "No messages converted from real entries");
    }
}
