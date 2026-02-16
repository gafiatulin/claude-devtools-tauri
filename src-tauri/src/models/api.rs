use serde::{Deserialize, Serialize};

// =============================================================================
// CLAUDE.md File Info
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMdFileInfo {
    pub path: String,
    pub exists: bool,
    pub char_count: u32,
    pub estimated_tokens: u32,
}

// =============================================================================
// Claude Root Info (simplified — macOS only, no WSL)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeRootInfo {
    pub default_path: String,
    pub resolved_path: String,
    pub custom_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeRootFolderSelection {
    pub path: String,
    pub is_claude_dir_name: bool,
    pub has_projects_dir: bool,
}

// Dropped: ContextInfo — no context switching (local-only)
// Dropped: HttpServerStatus — no HTTP sidecar server (local-only)
// Dropped: UpdaterStatus — no auto-updater
// Dropped: WslClaudeRootCandidate — Windows-only

// =============================================================================
// Validation types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathValidationResult {
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_directory: Option<bool>,
}
