/**
 * Tab selectors - Derived state selectors for tab-related data.
 * These selectors extract and compute tab-specific state from the store.
 */

import type { AppState } from '../types';
import type { Tab } from '@renderer/types/tabs';

/**
 * Select the currently active tab object.
 */
export function selectActiveTab(state: AppState): Tab | null {
  if (!state.activeTabId) return null;
  return state.openTabs.find((t) => t.id === state.activeTabId) ?? null;
}

/**
 * Select the active tab's session ID (if it's a session tab).
 */
export function selectActiveTabSessionId(state: AppState): string | null {
  const tab = selectActiveTab(state);
  if (tab?.type === 'session') return tab.sessionId ?? null;
  return null;
}

/**
 * Select the active tab's project ID (if it's a session tab).
 */
export function selectActiveTabProjectId(state: AppState): string | null {
  const tab = selectActiveTab(state);
  if (tab?.type === 'session' && typeof tab.projectId === 'string') return tab.projectId;
  return null;
}

/**
 * Select whether the active tab has a pending navigation request.
 */
export function selectHasPendingNavigation(state: AppState): boolean {
  const tab = selectActiveTab(state);
  return tab?.pendingNavigation != null;
}

/**
 * Select all tabs that are viewing a specific session.
 */
export function selectTabsForSession(state: AppState, sessionId: string): Tab[] {
  return state.openTabs.filter(
    (t) => t.type === 'session' && t.sessionId === sessionId
  );
}

/**
 * Select the number of open tabs.
 */
export function selectOpenTabCount(state: AppState): number {
  return state.openTabs.length;
}

/**
 * Select per-tab UI state for the active tab.
 */
export function selectActiveTabUIState(state: AppState) {
  const tabId = state.activeTabId;
  if (!tabId) return null;
  return state.tabUIStates.get(tabId) ?? null;
}

/**
 * Select whether the context panel is visible for the active tab.
 */
export function selectIsContextPanelVisible(state: AppState): boolean {
  const tabUI = selectActiveTabUIState(state);
  return tabUI?.showContextPanel ?? false;
}

/**
 * Select the selected context phase for the active tab.
 */
export function selectSelectedContextPhase(state: AppState): number | null {
  const tabUI = selectActiveTabUIState(state);
  return tabUI?.selectedContextPhase ?? null;
}
