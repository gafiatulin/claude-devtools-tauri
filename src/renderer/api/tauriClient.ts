/**
 * Tauri-native implementation of ElectronAPI.
 *
 * Replaces the HTTP+SSE HttpAPIClient with direct Tauri `invoke()` calls
 * and `listen()` event subscriptions. No HTTP server, no Node.js process.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type {
  AppConfig,
  ClaudeMdFileInfo,
  ClaudeRootInfo,
  ClaudeRootFolderSelection,
  ConfigAPI,
  ConversationGroup,
  ElectronAPI,
  FileChangeEvent,
  NotificationsAPI,
  NotificationTrigger,
  PaginatedSessionsResult,
  Project,
  RepositoryGroup,
  SearchSessionsResult,
  Session,
  SessionAPI,
  SessionDetail,
  SessionMetrics,
  SessionsByIdsOptions,
  SessionsPaginationOptions,
  SubagentDetail,
  TriggerTestResult,
  WaterfallData,
  WslClaudeRootCandidate,
} from '@shared/types';

// =============================================================================
// Date revival helpers
// =============================================================================

/**
 * Tauri `invoke()` returns JSON-deserialized objects but does NOT auto-revive
 * ISO 8601 date strings into Date objects. This walks the object tree in-place
 * to convert date strings, avoiding a full JSON.stringify/parse roundtrip.
 */
const ISO_DATE_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/;

function reviveDates<T>(data: unknown): T {
  if (typeof data === 'string') {
    if (data.length >= 19 && data.length <= 30 && ISO_DATE_RE.test(data)) {
      const d = new Date(data);
      if (!isNaN(d.getTime())) return d as unknown as T;
    }
    return data as unknown as T;
  }
  if (Array.isArray(data)) {
    for (let i = 0; i < data.length; i++) data[i] = reviveDates(data[i]);
    return data as unknown as T;
  }
  if (data && typeof data === 'object') {
    const obj = data as Record<string, unknown>;
    for (const key of Object.keys(obj)) {
      obj[key] = reviveDates(obj[key]);
    }
  }
  return data as T;
}

async function invokeWithDates<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const result = await invoke<unknown>(cmd, args);
  return reviveDates<T>(result);
}

// =============================================================================
// Event listener helper
// =============================================================================

/**
 * Creates a synchronous cleanup function wrapping Tauri's async `listen()`.
 * The returned function can be called immediately; the underlying unlisten
 * resolves when available.
 */
function tauriListen<T>(
  event: string,
  callback: (payload: T) => void,
): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  listen<T>(event, (ev) => {
    callback(ev.payload);
  }).then((fn) => {
    if (cancelled) fn();
    else unlisten = fn;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

// =============================================================================
// TauriClient
// =============================================================================

export class TauriClient implements ElectronAPI {
  // ---------------------------------------------------------------------------
  // Core session/project APIs
  // ---------------------------------------------------------------------------

  getAppVersion = (): Promise<string> =>
    invoke<string>('get_app_version');

  getProjects = (): Promise<Project[]> =>
    invokeWithDates<Project[]>('get_projects');

  getSessions = (projectId: string): Promise<Session[]> =>
    invokeWithDates<Session[]>('get_sessions', { projectId });

  getSessionsPaginated = (
    projectId: string,
    cursor: string | null,
    limit?: number,
    options?: SessionsPaginationOptions,
  ): Promise<PaginatedSessionsResult> =>
    invokeWithDates<PaginatedSessionsResult>('get_sessions_paginated', {
      projectId,
      cursor,
      limit,
      options,
    });

  searchSessions = (
    projectId: string,
    query: string,
    maxResults?: number,
  ): Promise<SearchSessionsResult> =>
    invokeWithDates<SearchSessionsResult>('search_sessions', {
      projectId,
      query,
      maxResults,
    });

  getSessionDetail = (projectId: string, sessionId: string): Promise<SessionDetail | null> =>
    invokeWithDates<SessionDetail | null>('get_session_detail', { projectId, sessionId });

  getSessionMetrics = (projectId: string, sessionId: string): Promise<SessionMetrics | null> =>
    invokeWithDates<SessionMetrics | null>('get_session_metrics', { projectId, sessionId });

  getWaterfallData = (projectId: string, sessionId: string): Promise<WaterfallData | null> =>
    invokeWithDates<WaterfallData | null>('get_waterfall_data', { projectId, sessionId });

  getSubagentDetail = (
    projectId: string,
    sessionId: string,
    subagentId: string,
  ): Promise<SubagentDetail | null> =>
    invokeWithDates<SubagentDetail | null>('get_subagent_detail', {
      projectId,
      sessionId,
      subagentId,
    });

  getSessionGroups = (projectId: string, sessionId: string): Promise<ConversationGroup[]> =>
    invokeWithDates<ConversationGroup[]>('get_session_groups', { projectId, sessionId });

  getSessionsByIds = (
    projectId: string,
    sessionIds: string[],
    options?: SessionsByIdsOptions,
  ): Promise<Session[]> =>
    invokeWithDates<Session[]>('get_sessions_by_ids', { projectId, sessionIds, options });

  // ---------------------------------------------------------------------------
  // Repository grouping
  // ---------------------------------------------------------------------------

  getRepositoryGroups = (): Promise<RepositoryGroup[]> =>
    invokeWithDates<RepositoryGroup[]>('get_repository_groups');

  getWorktreeSessions = (worktreeId: string): Promise<Session[]> =>
    invokeWithDates<Session[]>('get_worktree_sessions', { worktreeId });

  // ---------------------------------------------------------------------------
  // Validation
  // ---------------------------------------------------------------------------

  validatePath = (
    relativePath: string,
    projectPath: string,
  ): Promise<{ exists: boolean; isDirectory?: boolean }> =>
    invoke<{ exists: boolean; isDirectory?: boolean }>('validate_path', {
      relativePath,
      projectPath,
    });

  validateMentions = (
    mentions: { type: 'path'; value: string }[],
    projectPath: string,
  ): Promise<Record<string, boolean>> =>
    invoke<Record<string, boolean>>('validate_mentions', { mentions, projectPath });

  // ---------------------------------------------------------------------------
  // CLAUDE.md reading
  // ---------------------------------------------------------------------------

  readClaudeMdFiles = (projectRoot: string): Promise<Record<string, ClaudeMdFileInfo>> =>
    invoke<Record<string, ClaudeMdFileInfo>>('read_claude_md_files', { projectRoot });

  readDirectoryClaudeMd = (dirPath: string): Promise<ClaudeMdFileInfo> =>
    invoke<ClaudeMdFileInfo>('read_directory_claude_md', { dirPath });

  readMentionedFile = (
    absolutePath: string,
    projectRoot: string,
    maxTokens?: number,
  ): Promise<ClaudeMdFileInfo | null> =>
    invoke<ClaudeMdFileInfo | null>('read_mentioned_file', {
      absolutePath,
      projectRoot,
      maxTokens,
    });

  // ---------------------------------------------------------------------------
  // Notifications (nested API)
  // ---------------------------------------------------------------------------

  notifications: NotificationsAPI = {
    get: (options) =>
      invokeWithDates('get_notifications', {
        limit: options?.limit,
        offset: options?.offset,
      }),
    markRead: (id) => invoke<boolean>('mark_notification_read', { id }),
    markAllRead: () => invoke<boolean>('mark_all_notifications_read'),
    delete: (id) => invoke<boolean>('delete_notification', { id }),
    clear: () => invoke<boolean>('clear_notifications'),
    getUnreadCount: () => invoke<number>('get_unread_count'),
    onNew: (callback) =>
      tauriListen('notification:new', (data: unknown) => callback(null, data)),
    onUpdated: (callback) =>
      tauriListen('notification:updated', (data: unknown) =>
        callback(null, data as { total: number; unreadCount: number }),
      ),
    onClicked: (callback) =>
      tauriListen('notification:clicked', (data: unknown) => callback(null, data)),
  };

  // ---------------------------------------------------------------------------
  // Config (nested API)
  // ---------------------------------------------------------------------------

  config: ConfigAPI = {
    get: () => invokeWithDates<AppConfig>('get_config'),
    update: (section: string, data: object) =>
      invokeWithDates<AppConfig>('update_config', { section, data }),
    addIgnoreRegex: (pattern: string) =>
      invokeWithDates<AppConfig>('add_ignore_regex', { pattern }),
    removeIgnoreRegex: (pattern: string) =>
      invokeWithDates<AppConfig>('remove_ignore_regex', { pattern }),
    addIgnoreRepository: (repositoryId: string) =>
      invokeWithDates<AppConfig>('add_ignore_repository', { repositoryId }),
    removeIgnoreRepository: (repositoryId: string) =>
      invokeWithDates<AppConfig>('remove_ignore_repository', { repositoryId }),
    snooze: (minutes: number) =>
      invokeWithDates<AppConfig>('snooze_notifications', { minutes }),
    clearSnooze: () => invokeWithDates<AppConfig>('clear_snooze'),
    addTrigger: (trigger) =>
      invokeWithDates<AppConfig>('add_trigger', { trigger }),
    updateTrigger: (triggerId: string, updates) =>
      invokeWithDates<AppConfig>('update_trigger', { triggerId, updates }),
    removeTrigger: (triggerId: string) =>
      invokeWithDates<AppConfig>('remove_trigger', { triggerId }),
    getTriggers: () => invoke<NotificationTrigger[]>('get_triggers'),
    testTrigger: (trigger: NotificationTrigger) =>
      invoke<TriggerTestResult>('test_trigger', { trigger }),
    // Dropped in Tauri (macOS native) - provide no-op stubs
    selectFolders: async (): Promise<string[]> => {
      console.warn('[TauriClient] selectFolders not implemented');
      return [];
    },
    selectClaudeRootFolder: async (): Promise<ClaudeRootFolderSelection | null> => {
      console.warn('[TauriClient] selectClaudeRootFolder not implemented');
      return null;
    },
    getClaudeRootInfo: async (): Promise<ClaudeRootInfo> => {
      const config = await this.config.get();
      const fallbackPath = config.general?.claudeRootPath ?? '~/.claude';
      return {
        defaultPath: '~/.claude',
        resolvedPath: fallbackPath,
        customPath: config.general?.claudeRootPath ?? null,
      };
    },
    findWslClaudeRoots: async (): Promise<WslClaudeRootCandidate[]> => [],
    openInEditor: async (): Promise<void> => {
      console.warn('[TauriClient] openInEditor not implemented');
    },
    pinSession: (projectId: string, sessionId: string) =>
      invoke<void>('pin_session', { projectId, sessionId }),
    unpinSession: (projectId: string, sessionId: string) =>
      invoke<void>('unpin_session', { projectId, sessionId }),
    hideSession: (projectId: string, sessionId: string) =>
      invoke<void>('hide_session', { projectId, sessionId }),
    unhideSession: (projectId: string, sessionId: string) =>
      invoke<void>('unhide_session', { projectId, sessionId }),
    hideSessions: (projectId: string, sessionIds: string[]) =>
      invoke<void>('hide_sessions', { projectId, sessionIds }),
    unhideSessions: (projectId: string, sessionIds: string[]) =>
      invoke<void>('unhide_sessions', { projectId, sessionIds }),
  };

  // ---------------------------------------------------------------------------
  // Session navigation
  // ---------------------------------------------------------------------------

  session: SessionAPI = {
    scrollToLine: (sessionId: string, lineNumber: number) =>
      invoke<void>('scroll_to_line', { sessionId, lineNumber }),
  };

  // ---------------------------------------------------------------------------
  // File change events (via Tauri listen)
  // ---------------------------------------------------------------------------

  onFileChange = (callback: (event: FileChangeEvent) => void): (() => void) =>
    tauriListen<FileChangeEvent>('file-change', callback);

  onTodoChange = (callback: (event: FileChangeEvent) => void): (() => void) =>
    tauriListen<FileChangeEvent>('todo-change', callback);

  // ---------------------------------------------------------------------------
  // Shell operations
  // ---------------------------------------------------------------------------

  openPath = async (
    targetPath: string,
    projectRoot?: string,
  ): Promise<{ success: boolean; error?: string }> => {
    try {
      return await invoke<{ success: boolean; error?: string }>('open_path', {
        targetPath,
        projectRoot,
      });
    } catch (e) {
      return { success: false, error: String(e) };
    }
  };

  openExternal = async (url: string): Promise<{ success: boolean; error?: string }> => {
    try {
      return await invoke<{ success: boolean; error?: string }>('open_external', { url });
    } catch (e) {
      return { success: false, error: String(e) };
    }
  };

}
