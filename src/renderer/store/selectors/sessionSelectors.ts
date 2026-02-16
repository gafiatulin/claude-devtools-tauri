/**
 * Session selectors - Derived state selectors for session-related data.
 * These selectors extract and compute session-specific state from the store.
 */

import type { AppState } from '../types';
import type { Session } from '@renderer/types/data';

/**
 * Select sessions filtered by hidden state.
 * Returns all sessions if showHiddenSessions is true, otherwise filters hidden ones.
 */
export function selectVisibleSessions(state: AppState): Session[] {
  if (state.showHiddenSessions) return state.sessions;
  const hiddenSet = new Set(state.hiddenSessionIds);
  return state.sessions.filter((s) => !hiddenSet.has(s.id));
}

/**
 * Select the currently active session detail, preferring per-tab data.
 */
export function selectActiveSessionDetail(state: AppState) {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].sessionDetail;
  }
  return state.sessionDetail;
}

/**
 * Select the current conversation, preferring per-tab data.
 */
export function selectActiveConversation(state: AppState) {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].conversation;
  }
  return state.conversation;
}

/**
 * Select whether conversation is loading, preferring per-tab data.
 */
export function selectConversationLoading(state: AppState): boolean {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].conversationLoading;
  }
  return state.conversationLoading;
}

/**
 * Select context stats for the active tab.
 */
export function selectActiveContextStats(state: AppState) {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].sessionContextStats;
  }
  return state.sessionContextStats;
}

/**
 * Select phase info for the active tab.
 */
export function selectActivePhaseInfo(state: AppState) {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].sessionPhaseInfo;
  }
  return state.sessionPhaseInfo;
}

/**
 * Select the visible AI group for the active tab.
 */
export function selectActiveVisibleAIGroupId(state: AppState): string | null {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].visibleAIGroupId;
  }
  return state.visibleAIGroupId;
}

/**
 * Select the selected AI group for the active tab.
 */
export function selectActiveSelectedAIGroup(state: AppState) {
  const tabId = state.activeTabId;
  if (tabId && state.tabSessionData[tabId]) {
    return state.tabSessionData[tabId].selectedAIGroup;
  }
  return state.selectedAIGroup;
}

/**
 * Compute the total number of chat items in the active conversation.
 */
export function selectChatItemCount(state: AppState): number {
  const conversation = selectActiveConversation(state);
  return conversation?.items.length ?? 0;
}

/**
 * Select whether the session has multiple context phases (compaction occurred).
 */
export function selectIsMultiPhase(state: AppState): boolean {
  const phaseInfo = selectActivePhaseInfo(state);
  return (phaseInfo?.compactionCount ?? 0) > 0;
}
