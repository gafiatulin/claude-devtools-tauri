use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::models::domain::{
    Project, RepositoryGroup, RepositoryIdentity, Worktree, WorktreeSource,
};

/// Run a git command with `-C <path>` and return trimmed stdout on success.
fn git_output(repo_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

/// Detect if a path is inside a git repository and return the repository root.
pub fn find_git_root(path: &Path) -> Option<PathBuf> {
    let root = git_output(path, &["rev-parse", "--show-toplevel"])?;
    Some(PathBuf::from(root))
}

/// Get the current branch name for a repository.
/// Returns `None` for detached HEAD or non-git directories.
pub fn get_current_branch(repo_path: &Path) -> Option<String> {
    git_output(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .filter(|b| b != "HEAD") // detached HEAD
}

/// Get the path to the main `.git` directory (resolves worktree `.git` files).
fn get_git_common_dir(repo_path: &Path) -> Option<PathBuf> {
    let common = git_output(repo_path, &["rev-parse", "--git-common-dir"])?;
    let common_path = if Path::new(&common).is_absolute() {
        PathBuf::from(&common)
    } else {
        repo_path.join(&common)
    };
    // Canonicalize to resolve any `..` segments
    common_path.canonicalize().ok()
}

/// Get all worktrees for a repository by running `git worktree list --porcelain`.
pub fn get_worktrees(repo_path: &Path) -> Vec<WorktreeInfo> {
    let output = match git_output(repo_path, &["worktree", "list", "--porcelain"]) {
        Some(o) => o,
        None => return Vec::new(),
    };

    parse_worktree_list(&output)
}

/// Parsed worktree entry from `git worktree list --porcelain`.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub is_bare: bool,
}

/// Parse the porcelain output of `git worktree list --porcelain`.
fn parse_worktree_list(output: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in output.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            // If we have a pending entry, push it
            if let Some(path) = current_path.take() {
                worktrees.push(WorktreeInfo {
                    path,
                    branch: current_branch.take(),
                    is_bare,
                });
            }
            current_path = Some(PathBuf::from(path_str));
            current_branch = None;
            is_bare = false;
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            // branch refs/heads/main -> main
            current_branch = branch_ref.strip_prefix("refs/heads/").map(String::from);
        } else if line == "bare" {
            is_bare = true;
        }
        // "HEAD <sha>" and "detached" lines are ignored
    }

    // Push the last entry
    if let Some(path) = current_path {
        worktrees.push(WorktreeInfo {
            path,
            branch: current_branch,
            is_bare,
        });
    }

    worktrees
}

/// Compute a stable repository identity based on remote URL or root commit hash.
pub fn get_repository_identity(repo_path: &Path) -> Option<RepositoryIdentity> {
    let git_common_dir = get_git_common_dir(repo_path)?;

    // Try remote origin URL first
    let remote_url = git_output(repo_path, &["remote", "get-url", "origin"]);

    // Determine the identity string to hash
    let identity_source = if let Some(ref url) = remote_url {
        url.clone()
    } else {
        // Fall back to root commit hash
        git_output(repo_path, &["rev-list", "--max-parents=0", "HEAD"])?
    };

    // Hash the identity source to create a stable ID
    let mut hasher = Sha256::new();
    hasher.update(identity_source.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let id = hash[..16].to_string(); // Use first 16 hex chars

    // Derive a display name from the git dir or remote URL
    let name = derive_repo_name(&remote_url, &git_common_dir);

    Some(RepositoryIdentity {
        id,
        remote_url: remote_url,
        main_git_dir: git_common_dir.to_string_lossy().to_string(),
        name,
    })
}

/// Derive a human-readable repository name from the remote URL or git directory.
fn derive_repo_name(remote_url: &Option<String>, git_dir: &Path) -> String {
    if let Some(url) = remote_url {
        // Extract repo name from URL: "https://github.com/org/repo.git" -> "repo"
        let name = url
            .rsplit('/')
            .next()
            .unwrap_or(url)
            .trim_end_matches(".git");
        if !name.is_empty() {
            return name.to_string();
        }
    }

    // Fall back to parent directory name of the .git dir
    git_dir
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Detect the worktree source from the project path.
fn detect_worktree_source(path: &str) -> WorktreeSource {
    if path.contains("/vibe-kanban/worktrees/") || path.contains("/vibe-kanban/") {
        WorktreeSource::VibeKanban
    } else if path.contains("/conductor/workspaces/") || path.contains("/conductor/") {
        WorktreeSource::Conductor
    } else if path.contains("/.auto-claude/worktrees/") || path.contains("/.auto-claude/") {
        WorktreeSource::AutoClaude
    } else if path.contains("/.21st/worktrees/") || path.contains("/.21st/") {
        WorktreeSource::TwentyFirst
    } else if path.contains("/.claude-worktrees/") {
        WorktreeSource::ClaudeDesktop
    } else if path.contains("/.ccswitch/worktrees/") || path.contains("/.ccswitch/") {
        WorktreeSource::Ccswitch
    } else if find_git_root(Path::new(path)).is_some() {
        WorktreeSource::Git
    } else {
        WorktreeSource::Unknown
    }
}

/// Build repository groups from a list of projects.
///
/// Groups projects that share the same git repository identity together.
/// Non-git projects become standalone groups with a single worktree.
pub fn build_repository_groups(projects: &[Project]) -> Vec<RepositoryGroup> {
    // Map: identity_id -> (Option<RepositoryIdentity>, Vec<(Project, is_main_worktree, branch)>)
    let mut groups: HashMap<String, (Option<RepositoryIdentity>, Vec<ProjectWorktreeInfo>)> =
        HashMap::new();

    // Order of insertion for stable ordering
    let mut group_order: Vec<String> = Vec::new();

    for project in projects {
        let project_path = Path::new(&project.path);
        let identity = get_repository_identity(project_path);

        let group_id = identity
            .as_ref()
            .map(|i| i.id.clone())
            .unwrap_or_else(|| project.id.clone());

        let git_root = find_git_root(project_path);
        let is_main_worktree = git_root
            .as_ref()
            .map(|root| root.as_path() == project_path)
            .unwrap_or(false);

        let branch = get_current_branch(project_path);
        let source = detect_worktree_source(&project.path);

        let entry = groups.entry(group_id.clone()).or_insert_with(|| {
            group_order.push(group_id.clone());
            (identity.clone(), Vec::new())
        });

        entry.1.push(ProjectWorktreeInfo {
            project: project.clone(),
            is_main_worktree,
            branch,
            source,
        });
    }

    // Build the final RepositoryGroup list
    let mut result: Vec<RepositoryGroup> = Vec::new();

    for group_id in &group_order {
        let (identity, project_infos) = match groups.remove(group_id) {
            Some(v) => v,
            None => continue,
        };

        let name = identity
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_else(|| {
                project_infos
                    .first()
                    .map(|p| p.project.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            });

        let mut worktrees: Vec<Worktree> = project_infos
            .iter()
            .map(|info| {
                let worktree_name = info
                    .branch
                    .clone()
                    .unwrap_or_else(|| info.project.name.clone());

                Worktree {
                    id: info.project.id.clone(),
                    path: info.project.path.clone(),
                    name: worktree_name,
                    git_branch: info.branch.clone(),
                    is_main_worktree: info.is_main_worktree,
                    source: info.source.clone(),
                    sessions: info.project.sessions.clone(),
                    created_at: info.project.created_at,
                    most_recent_session: info.project.most_recent_session,
                }
            })
            .collect();

        // Sort worktrees: main worktree first, then by most recent session descending
        worktrees.sort_by(|a, b| {
            b.is_main_worktree
                .cmp(&a.is_main_worktree)
                .then_with(|| {
                    let a_recent = a.most_recent_session.unwrap_or(0.0);
                    let b_recent = b.most_recent_session.unwrap_or(0.0);
                    b_recent
                        .partial_cmp(&a_recent)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        let most_recent_session = worktrees
            .iter()
            .filter_map(|w| w.most_recent_session)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let total_sessions: u32 = worktrees
            .iter()
            .map(|w| w.sessions.len() as u32)
            .sum();

        result.push(RepositoryGroup {
            id: group_id.clone(),
            identity,
            worktrees,
            name,
            most_recent_session,
            total_sessions,
        });
    }

    // Sort groups by most recent session descending
    result.sort_by(|a, b| {
        let a_recent = a.most_recent_session.unwrap_or(0.0);
        let b_recent = b.most_recent_session.unwrap_or(0.0);
        b_recent
            .partial_cmp(&a_recent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    result
}

/// Internal helper struct for building worktree info from projects.
struct ProjectWorktreeInfo {
    project: Project,
    is_main_worktree: bool,
    branch: Option<String>,
    source: WorktreeSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list_single() {
        let output = "worktree /Users/user/project\nHEAD abc123\nbranch refs/heads/main\n";
        let result = parse_worktree_list(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("/Users/user/project"));
        assert_eq!(result[0].branch, Some("main".to_string()));
        assert!(!result[0].is_bare);
    }

    #[test]
    fn test_parse_worktree_list_multiple() {
        let output = "\
worktree /Users/user/project
HEAD abc123
branch refs/heads/main

worktree /Users/user/project-feature
HEAD def456
branch refs/heads/feature/cool

worktree /Users/user/project-detached
HEAD 789abc
detached
";
        let result = parse_worktree_list(output);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].branch, Some("main".to_string()));
        assert_eq!(result[1].branch, Some("feature/cool".to_string()));
        assert_eq!(result[2].branch, None); // detached
    }

    #[test]
    fn test_parse_worktree_list_bare() {
        let output = "worktree /Users/user/project.git\nHEAD abc123\nbare\n";
        let result = parse_worktree_list(output);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_bare);
    }

    #[test]
    fn test_parse_worktree_list_empty() {
        let result = parse_worktree_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_derive_repo_name_from_url() {
        let url = Some("https://github.com/org/my-repo.git".to_string());
        let name = derive_repo_name(&url, Path::new("/tmp/.git"));
        assert_eq!(name, "my-repo");
    }

    #[test]
    fn test_derive_repo_name_from_url_no_git_suffix() {
        let url = Some("https://github.com/org/my-repo".to_string());
        let name = derive_repo_name(&url, Path::new("/tmp/.git"));
        assert_eq!(name, "my-repo");
    }

    #[test]
    fn test_derive_repo_name_from_ssh_url() {
        let url = Some("git@github.com:org/my-repo.git".to_string());
        let name = derive_repo_name(&url, Path::new("/tmp/.git"));
        assert_eq!(name, "my-repo");
    }

    #[test]
    fn test_derive_repo_name_fallback_to_dir() {
        let name = derive_repo_name(&None, Path::new("/Users/user/myproject/.git"));
        assert_eq!(name, "myproject");
    }

    #[test]
    fn test_detect_worktree_source_vibe_kanban() {
        assert_eq!(
            detect_worktree_source("/tmp/vibe-kanban/worktrees/issue-123/repo"),
            WorktreeSource::VibeKanban
        );
    }

    #[test]
    fn test_detect_worktree_source_conductor() {
        assert_eq!(
            detect_worktree_source("/Users/user/conductor/workspaces/repo/ws1"),
            WorktreeSource::Conductor
        );
    }

    #[test]
    fn test_detect_worktree_source_auto_claude() {
        assert_eq!(
            detect_worktree_source("/Users/user/.auto-claude/worktrees/tasks/123"),
            WorktreeSource::AutoClaude
        );
    }

    #[test]
    fn test_detect_worktree_source_21st() {
        assert_eq!(
            detect_worktree_source("/Users/user/.21st/worktrees/abc/name"),
            WorktreeSource::TwentyFirst
        );
    }

    #[test]
    fn test_detect_worktree_source_claude_desktop() {
        assert_eq!(
            detect_worktree_source("/Users/user/.claude-worktrees/repo/branch"),
            WorktreeSource::ClaudeDesktop
        );
    }

    #[test]
    fn test_detect_worktree_source_ccswitch() {
        assert_eq!(
            detect_worktree_source("/Users/user/.ccswitch/worktrees/repo/branch"),
            WorktreeSource::Ccswitch
        );
    }

    #[test]
    fn test_detect_worktree_source_unknown() {
        // Non-existent path, not a git repo
        assert_eq!(
            detect_worktree_source("/tmp/definitely-not-a-real-path-xyz123"),
            WorktreeSource::Unknown
        );
    }
}
