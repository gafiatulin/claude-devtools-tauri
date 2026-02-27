/**
 * API type definitions for the Tauri command surface.
 *
 * Defines the interface exposed to the renderer process via Tauri's invoke().
 * The implementation lives in src/renderer/api/tauriClient.ts.
 */

import type {
  AppConfig,
  DetectedError,
  NotificationTrigger,
  TriggerTestResult,
} from './notifications';
import type { WaterfallData } from './visualization';
import type {
  ConversationGroup,
  FileChangeEvent,
  PaginatedSessionsResult,
  Project,
  RepositoryGroup,
  SearchSessionsResult,
  Session,
  SessionDetail,
  SessionMetrics,
  SessionsByIdsOptions,
  SessionsPaginationOptions,
  SubagentDetail,
} from '@main/types';

// =============================================================================
// Notifications API
// =============================================================================

/**
 * Result of notifications:get with pagination.
 */
interface NotificationsResult {
  notifications: DetectedError[];
  total: number;
  totalCount: number;
  unreadCount: number;
  hasMore: boolean;
}

/**
 * Notifications API.
 * Note: Event callbacks use `unknown` types because Tauri event data is untyped at the listen() layer.
 * Consumers should cast to DetectedError or NotificationClickData as appropriate.
 */
export interface NotificationsAPI {
  get: (options?: { limit?: number; offset?: number }) => Promise<NotificationsResult>;
  markRead: (id: string) => Promise<boolean>;
  markAllRead: () => Promise<boolean>;
  delete: (id: string) => Promise<boolean>;
  clear: () => Promise<boolean>;
  getUnreadCount: () => Promise<number>;
  onNew: (callback: (event: unknown, error: unknown) => void) => () => void;
  onUpdated: (
    callback: (event: unknown, payload: { total: number; unreadCount: number }) => void
  ) => () => void;
  onClicked: (callback: (event: unknown, data: unknown) => void) => () => void;
}

// =============================================================================
// Config API
// =============================================================================

/**
 * Config API exposed via preload.
 */
export interface ConfigAPI {
  get: () => Promise<AppConfig>;
  update: (section: string, data: object) => Promise<AppConfig>;
  addIgnoreRegex: (pattern: string) => Promise<AppConfig>;
  removeIgnoreRegex: (pattern: string) => Promise<AppConfig>;
  addIgnoreRepository: (repositoryId: string) => Promise<AppConfig>;
  removeIgnoreRepository: (repositoryId: string) => Promise<AppConfig>;
  snooze: (minutes: number) => Promise<AppConfig>;
  clearSnooze: () => Promise<AppConfig>;
  // Trigger management methods
  addTrigger: (trigger: Omit<NotificationTrigger, 'isBuiltin'>) => Promise<AppConfig>;
  updateTrigger: (triggerId: string, updates: Partial<NotificationTrigger>) => Promise<AppConfig>;
  removeTrigger: (triggerId: string) => Promise<AppConfig>;
  getTriggers: () => Promise<NotificationTrigger[]>;
  testTrigger: (trigger: NotificationTrigger) => Promise<TriggerTestResult>;
  /** Opens native folder selection dialog and returns selected paths */
  selectFolders: () => Promise<string[]>;
  /** Open native dialog to select local Claude root folder */
  selectClaudeRootFolder: () => Promise<ClaudeRootFolderSelection | null>;
  /** Get resolved Claude root path info for local mode */
  getClaudeRootInfo: () => Promise<ClaudeRootInfo>;
  /** Find Windows WSL Claude root candidates (UNC paths) */
  findWslClaudeRoots: () => Promise<WslClaudeRootCandidate[]>;
  /** Opens the config JSON file in an external editor */
  openInEditor: () => Promise<void>;
  /** Pin a session for a project */
  pinSession: (projectId: string, sessionId: string) => Promise<void>;
  /** Unpin a session for a project */
  unpinSession: (projectId: string, sessionId: string) => Promise<void>;
  /** Hide a session for a project */
  hideSession: (projectId: string, sessionId: string) => Promise<void>;
  /** Unhide a session for a project */
  unhideSession: (projectId: string, sessionId: string) => Promise<void>;
  /** Bulk hide sessions for a project */
  hideSessions: (projectId: string, sessionIds: string[]) => Promise<void>;
  /** Bulk unhide sessions for a project */
  unhideSessions: (projectId: string, sessionIds: string[]) => Promise<void>;
}

export interface ClaudeRootInfo {
  /** Auto-detected default Claude root path for this machine */
  defaultPath: string;
  /** Effective path currently used by local context */
  resolvedPath: string;
  /** Custom override path from settings (null means auto-detect) */
  customPath: string | null;
}

export interface ClaudeRootFolderSelection {
  /** Selected directory absolute path */
  path: string;
  /** Whether the selected folder name is exactly ".claude" */
  isClaudeDirName: boolean;
  /** Whether selected folder contains a "projects" directory */
  hasProjectsDir: boolean;
}

export interface WslClaudeRootCandidate {
  /** WSL distribution name (e.g. Ubuntu) */
  distro: string;
  /** Candidate Claude root path in UNC format */
  path: string;
  /** True if this root contains "projects" directory */
  hasProjectsDir: boolean;
}

// =============================================================================
// Session API
// =============================================================================

/**
 * Session navigation API exposed via preload.
 */
export interface SessionAPI {
  scrollToLine: (sessionId: string, lineNumber: number) => Promise<void>;
}

// =============================================================================
// CLAUDE.md File Info
// =============================================================================

/**
 * CLAUDE.md file information returned from reading operations.
 */
export interface ClaudeMdFileInfo {
  path: string;
  exists: boolean;
  charCount: number;
  estimatedTokens: number;
}

// =============================================================================
// App API
// =============================================================================

/**
 * Complete API surface exposed to the renderer via Tauri invoke().
 * Implemented by TauriClient in src/renderer/api/tauriClient.ts.
 */
export interface ElectronAPI {
  getAppVersion: () => Promise<string>;
  getProjects: () => Promise<Project[]>;
  getSessions: (projectId: string) => Promise<Session[]>;
  getSessionsPaginated: (
    projectId: string,
    cursor: string | null,
    limit?: number,
    options?: SessionsPaginationOptions
  ) => Promise<PaginatedSessionsResult>;
  searchSessions: (
    projectId: string,
    query: string,
    maxResults?: number
  ) => Promise<SearchSessionsResult>;
  getSessionDetail: (projectId: string, sessionId: string) => Promise<SessionDetail | null>;
  getSessionMetrics: (projectId: string, sessionId: string) => Promise<SessionMetrics | null>;
  getWaterfallData: (projectId: string, sessionId: string) => Promise<WaterfallData | null>;
  getSubagentDetail: (
    projectId: string,
    sessionId: string,
    subagentId: string
  ) => Promise<SubagentDetail | null>;
  getSessionGroups: (projectId: string, sessionId: string) => Promise<ConversationGroup[]>;
  getSessionsByIds: (
    projectId: string,
    sessionIds: string[],
    options?: SessionsByIdsOptions
  ) => Promise<Session[]>;

  // Repository grouping (worktree support)
  getRepositoryGroups: () => Promise<RepositoryGroup[]>;
  getWorktreeSessions: (worktreeId: string) => Promise<Session[]>;

  // Validation methods
  validatePath: (
    relativePath: string,
    projectPath: string
  ) => Promise<{ exists: boolean; isDirectory?: boolean }>;
  validateMentions: (
    mentions: { type: 'path'; value: string }[],
    projectPath: string
  ) => Promise<Record<string, boolean>>;

  // CLAUDE.md reading methods
  readClaudeMdFiles: (projectRoot: string) => Promise<Record<string, ClaudeMdFileInfo>>;
  readDirectoryClaudeMd: (dirPath: string) => Promise<ClaudeMdFileInfo>;
  readMentionedFile: (
    absolutePath: string,
    projectRoot: string,
    maxTokens?: number
  ) => Promise<ClaudeMdFileInfo | null>;

  // Notifications API
  notifications: NotificationsAPI;

  // Config API
  config: ConfigAPI;

  // Deep link navigation
  session: SessionAPI;

  // File change events (real-time updates)
  onFileChange: (callback: (event: FileChangeEvent) => void) => () => void;
  onTodoChange: (callback: (event: FileChangeEvent) => void) => () => void;

  // Shell operations
  openPath: (
    targetPath: string,
    projectRoot?: string
  ) => Promise<{ success: boolean; error?: string }>;
  openExternal: (url: string) => Promise<{ success: boolean; error?: string }>;

  // Background task output
  readBackgroundTaskOutput: (taskId: string) => Promise<{ content: string; isRunning: boolean } | null>;
}
