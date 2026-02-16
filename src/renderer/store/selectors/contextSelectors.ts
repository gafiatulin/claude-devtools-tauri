/**
 * Context selectors - Cross-slice selectors combining session + tab state.
 * These selectors derive context-related state that spans multiple store slices.
 */

import type { AppState } from '../types';
import type { ContextInjection } from '@renderer/types/contextInjection';

/**
 * Select accumulated context injections for the last AI group in the active tab's session.
 * Optionally filtered by selected context phase.
 */
export function selectAccumulatedInjections(state: AppState): ContextInjection[] {
  const tabId = state.activeTabId;
  const tabData = tabId ? state.tabSessionData[tabId] : null;
  const conversation = tabData?.conversation ?? state.conversation;
  const contextStats = tabData?.sessionContextStats ?? state.sessionContextStats;
  const phaseInfo = tabData?.sessionPhaseInfo ?? state.sessionPhaseInfo;

  if (!contextStats || !conversation?.items.length) {
    return [];
  }

  // Get phase selection from tab UI state
  const tabUI = tabId ? state.tabUIStates.get(tabId) : null;
  const selectedPhase = tabUI?.selectedContextPhase ?? null;

  // Determine target AI group
  let targetAiGroupId: string | undefined;
  if (selectedPhase !== null && phaseInfo) {
    const phase = phaseInfo.phases.find((p) => p.phaseNumber === selectedPhase);
    if (phase) {
      targetAiGroupId = phase.lastAIGroupId;
    }
  }

  if (!targetAiGroupId) {
    const lastAiItem = [...conversation.items].reverse().find((item) => item.type === 'ai');
    if (lastAiItem?.type !== 'ai') return [];
    targetAiGroupId = lastAiItem.group.id;
  }

  const stats = contextStats.get(targetAiGroupId);
  return stats?.accumulatedInjections ?? [];
}

/**
 * Select total context injection count for the active session.
 */
export function selectContextInjectionCount(state: AppState): number {
  return selectAccumulatedInjections(state).length;
}

/**
 * Select whether the session has any context injections.
 */
export function selectHasContextInjections(state: AppState): boolean {
  return selectContextInjectionCount(state) > 0;
}

/**
 * Select the total estimated tokens across all accumulated context injections.
 */
export function selectTotalContextTokens(state: AppState): number {
  const injections = selectAccumulatedInjections(state);
  return injections.reduce((sum, inj) => sum + inj.estimatedTokens, 0);
}

/**
 * Select context injection counts by category.
 */
export function selectContextCountsByCategory(state: AppState): {
  claudeMd: number;
  mentionedFiles: number;
  toolOutputs: number;
  thinkingText: number;
  taskCoordination: number;
  userMessages: number;
} {
  const injections = selectAccumulatedInjections(state);
  const counts = {
    claudeMd: 0,
    mentionedFiles: 0,
    toolOutputs: 0,
    thinkingText: 0,
    taskCoordination: 0,
    userMessages: 0,
  };

  for (const inj of injections) {
    switch (inj.category) {
      case 'claude-md':
        counts.claudeMd++;
        break;
      case 'mentioned-file':
        counts.mentionedFiles++;
        break;
      case 'tool-output':
        counts.toolOutputs++;
        break;
      case 'thinking-text':
        counts.thinkingText++;
        break;
      case 'task-coordination':
        counts.taskCoordination++;
        break;
      case 'user-message':
        counts.userMessages++;
        break;
    }
  }

  return counts;
}
