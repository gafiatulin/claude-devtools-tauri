use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;

pub mod commands;
pub mod config;
pub mod events;
pub mod git;
pub mod jsonl;
pub mod models;
pub mod notifications;
pub mod parser;
pub mod scanner;
pub mod search;
pub mod watcher;

// =============================================================================
// Application State
// =============================================================================

/// Shared application state managed by Tauri.
///
/// Provides access to the config manager, notification store, and
/// resolved Claude root path.
pub struct AppState {
    pub config: config::manager::ConfigManager,
    pub notifications: notifications::store::NotificationStore,
    claude_root: PathBuf,
    pub session_cache:
        Mutex<lru::LruCache<PathBuf, (std::time::SystemTime, Arc<models::domain::Session>)>>,
    pub metrics_cache:
        Mutex<lru::LruCache<PathBuf, (std::time::SystemTime, models::domain::SessionMetrics)>>,
    /// Cached project list with timestamp. Avoids redundant scan_projects() calls
    /// when multiple commands need the project list within a short window.
    pub projects_cache: Mutex<Option<(std::time::Instant, Vec<models::domain::Project>)>>,
}

impl AppState {
    /// Returns the resolved Claude root path (e.g., ~/.claude).
    pub fn claude_root(&self) -> PathBuf {
        self.claude_root.clone()
    }
}

/// Holds the file watcher handle to keep it alive for the app's lifetime.
/// Dropping this would stop the watcher. The inner value is never read
/// directly -- its purpose is preventing the handle from being dropped.
#[allow(dead_code)]
struct WatcherState(std::sync::Mutex<Option<watcher::file_watcher::WatcherHandle>>);

// =============================================================================
// Application entry point
// =============================================================================

fn resolve_claude_root(config: &config::manager::ConfigManager) -> PathBuf {
    // Check if user has configured a custom Claude root
    let app_config = config.get();
    if let Some(custom) = &app_config.general.claude_root_path {
        let path = PathBuf::from(custom);
        if path.exists() {
            return path;
        }
    }

    // Default: ~/.claude
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber for performance spans
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("claude_devtools_tauri_lib=info".parse().unwrap()),
        )
        .with_target(true)
        .init();

    // Initialize config manager
    let config_path = config::manager::ConfigManager::default_path();
    let config_manager = config::manager::ConfigManager::load(&config_path).unwrap_or_else(|e| {
        eprintln!("[config] Failed to load config, using defaults: {e}");
        config::manager::ConfigManager::new(config_path)
    });

    let claude_root = resolve_claude_root(&config_manager);

    // Initialize notification store (load from disk if exists)
    let notifications_path = notifications::store::NotificationStore::default_path();
    let notification_store = notifications::store::NotificationStore::load(&notifications_path)
        .unwrap_or_else(|e| {
            eprintln!("[notifications] Failed to load notifications, starting fresh: {e}");
            notifications::store::NotificationStore::new(notifications_path)
        });

    let app_state = AppState {
        config: config_manager,
        notifications: notification_store,
        claude_root,
        session_cache: Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(500).unwrap(),
        )),
        metrics_cache: Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(100).unwrap(),
        )),
        projects_cache: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Projects
            commands::projects::get_projects,
            commands::projects::get_repository_groups,
            commands::projects::get_worktree_sessions,
            // Sessions
            commands::sessions::get_sessions,
            commands::sessions::get_sessions_paginated,
            commands::sessions::get_sessions_by_ids,
            commands::sessions::get_session_detail,
            commands::sessions::get_session_metrics,
            commands::sessions::get_session_groups,
            commands::sessions::get_waterfall_data,
            commands::sessions::get_subagent_detail,
            // Search
            commands::search::search_sessions,
            // Config
            commands::config::get_config,
            commands::config::update_config,
            commands::config::add_ignore_regex,
            commands::config::remove_ignore_regex,
            commands::config::add_ignore_repository,
            commands::config::remove_ignore_repository,
            commands::config::snooze_notifications,
            commands::config::clear_snooze,
            commands::config::add_trigger,
            commands::config::update_trigger,
            commands::config::remove_trigger,
            commands::config::get_triggers,
            commands::config::test_trigger,
            commands::config::pin_session,
            commands::config::unpin_session,
            commands::config::hide_session,
            commands::config::unhide_session,
            commands::config::hide_sessions,
            commands::config::unhide_sessions,
            // Notifications
            commands::notifications::get_notifications,
            commands::notifications::mark_notification_read,
            commands::notifications::mark_all_notifications_read,
            commands::notifications::delete_notification,
            commands::notifications::clear_notifications,
            commands::notifications::get_unread_count,
            // Validation
            commands::validation::validate_path,
            commands::validation::validate_mentions,
            // CLAUDE.md
            commands::claude_md::read_claude_md_files,
            commands::claude_md::read_directory_claude_md,
            commands::claude_md::read_mentioned_file,
            // Shell
            commands::shell::open_path,
            commands::shell::open_external,
            commands::shell::get_app_version,
            commands::shell::scroll_to_line,
            commands::shell::read_background_task_output,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let claude_root = app.state::<AppState>().claude_root();

            // Start the file watcher synchronously and store the handle
            // in managed state so it lives for the app's lifetime.
            match watcher::file_watcher::start_watcher(handle, &claude_root) {
                Ok(watcher_handle) => {
                    app.manage(WatcherState(std::sync::Mutex::new(Some(watcher_handle))));
                }
                Err(e) => {
                    eprintln!("[tauri] Failed to start file watcher: {e}");
                    app.manage(WatcherState(std::sync::Mutex::new(None)));
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
