use crate::models::domain::MessageCategory;
use crate::models::messages::ParsedMessage;

/// Classify a parsed message into a category for chunk building.
///
/// The classification order matters:
/// 1. Hard noise is filtered out first (system entries, synthetic, empty stdout/stderr)
/// 2. Compact summary messages get their own chunks
/// 3. System output messages (stdout/stderr) become system chunks
/// 4. User messages become user chunks
/// 5. Assistant messages become AI chunks
pub fn classify_message(msg: &ParsedMessage) -> MessageCategory {
    // Hard noise is always filtered first
    if msg.is_parsed_hard_noise_message() {
        return MessageCategory::HardNoise;
    }

    // Compact summary messages
    if msg.is_parsed_compact_message() {
        return MessageCategory::Compact;
    }

    // System output messages (stdout/stderr from tool execution)
    if msg.is_parsed_system_chunk_message() {
        return MessageCategory::System;
    }

    // User messages that should form user chunks
    if msg.is_parsed_user_chunk_message() {
        return MessageCategory::User;
    }

    // Assistant messages
    if msg.message_type == crate::models::domain::MessageType::Assistant {
        return MessageCategory::Ai;
    }

    // Internal user messages (tool results) that don't qualify as user chunks
    // are treated as AI-context messages since they appear during AI response flows
    if msg.is_parsed_internal_user_message() {
        return MessageCategory::Ai;
    }

    // Everything else is noise
    MessageCategory::HardNoise
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domain::MessageType;
    use crate::models::jsonl::StringOrBlocks;

    fn make_msg(msg_type: MessageType, content: &str) -> ParsedMessage {
        ParsedMessage {
            uuid: "test".to_string(),
            parent_uuid: None,
            message_type: msg_type,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            role: None,
            content: StringOrBlocks::String(content.to_string()),
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
        }
    }

    #[test]
    fn test_user_message_classified_as_user() {
        let msg = make_msg(MessageType::User, "Hello world");
        assert_eq!(classify_message(&msg), MessageCategory::User);
    }

    #[test]
    fn test_assistant_message_classified_as_ai() {
        let msg = make_msg(MessageType::Assistant, "Response");
        assert_eq!(classify_message(&msg), MessageCategory::Ai);
    }

    #[test]
    fn test_system_entry_classified_as_hard_noise() {
        let msg = make_msg(MessageType::System, "system info");
        assert_eq!(classify_message(&msg), MessageCategory::HardNoise);
    }

    #[test]
    fn test_compact_summary_classified() {
        let mut msg = make_msg(MessageType::Summary, "summary text");
        msg.is_compact_summary = Some(true);
        // Summary type is hard noise, but a compact message with user type might differ
        // Summary type entries are always hard noise per is_parsed_hard_noise_message
        assert_eq!(classify_message(&msg), MessageCategory::HardNoise);
    }
}
