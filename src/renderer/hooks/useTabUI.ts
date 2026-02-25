/**
 * useTabUI - Hooks for accessing per-tab UI state.
 *
 * PERFORMANCE FIX: The original useTabUI() subscribed to the entire `tabUIStates` Map
 * via `useStore((s) => s.tabUIStates)`. Every mutation (expand/collapse any group)
 * creates a new Map, causing ALL components using useTabUI() to re-render — O(N)
 * re-renders for N AIChatGroups per toggle.
 *
 * Fix: Individual hooks (useIsAIGroupExpanded, useExpandedDisplayItemIds,
 * useIsSubagentTraceExpanded) subscribe to specific primitive/reference-stable values.
 * Only the component whose specific value changed re-renders.
 *
 * The monolithic useTabUI() is kept for ChatHistory which needs multiple values
 * (context panel, scroll position, phase selection), but now uses individual selectors
 * returning primitives instead of subscribing to the entire Map.
 */

import { useCallback } from 'react';

import { useTabIdOptional } from '@renderer/contexts/useTabUIContext';
import { useStore } from '@renderer/store';
import { useShallow } from 'zustand/react/shallow';

import type { AppState } from '@renderer/store/types';

// =============================================================================
// Constants
// =============================================================================

/** Shared empty set — avoids new Set() allocations that break reference equality */
const EMPTY_SET: ReadonlySet<string> = new Set<string>();

// =============================================================================
// Targeted Hooks (hot-path components: AIChatGroup, SubagentItem)
// =============================================================================

/**
 * Check if a specific AI group is expanded in the current tab.
 * Returns a primitive boolean — only re-renders when THIS group's expansion toggles.
 */
export function useIsAIGroupExpanded(aiGroupId: string): boolean {
  const tabId = useTabIdOptional();
  return useStore(
    useCallback(
      (s: AppState) => {
        if (!tabId) return false;
        return s.tabUIStates.get(tabId)?.expandedAIGroupIds.has(aiGroupId) ?? false;
      },
      [tabId, aiGroupId]
    )
  );
}

/**
 * Get expanded display item IDs for a specific AI group in the current tab.
 * Returns a reference-stable Set — only re-renders when THIS group's display items change.
 */
export function useExpandedDisplayItemIds(aiGroupId: string): ReadonlySet<string> {
  const tabId = useTabIdOptional();
  return useStore(
    useCallback(
      (s: AppState): ReadonlySet<string> => {
        if (!tabId) return EMPTY_SET;
        return (
          (s.tabUIStates.get(tabId)?.expandedDisplayItemIds.get(aiGroupId) as
            | ReadonlySet<string>
            | undefined) ?? EMPTY_SET
        );
      },
      [tabId, aiGroupId]
    )
  );
}

/**
 * Check if a specific subagent trace is expanded in the current tab.
 * Returns a primitive boolean — only re-renders when THIS subagent's expansion toggles.
 */
export function useIsSubagentTraceExpanded(subagentId: string): boolean {
  const tabId = useTabIdOptional();
  return useStore(
    useCallback(
      (s: AppState) => {
        if (!tabId) return false;
        return s.tabUIStates.get(tabId)?.expandedSubagentTraceIds.has(subagentId) ?? false;
      },
      [tabId, subagentId]
    )
  );
}

// =============================================================================
// Types
// =============================================================================

interface UseTabUIReturn {
  tabId: string | null;
  // AI Group expansion (actions only — use useIsAIGroupExpanded for state)
  toggleAIGroupExpansion: (aiGroupId: string) => void;
  expandAIGroup: (aiGroupId: string) => void;
  // Display item expansion (actions only — use useExpandedDisplayItemIds for state)
  toggleDisplayItemExpansion: (aiGroupId: string, itemId: string) => void;
  expandDisplayItem: (aiGroupId: string, itemId: string) => void;
  // Subagent trace expansion (actions only — use useIsSubagentTraceExpanded for state)
  toggleSubagentTraceExpansion: (subagentId: string) => void;
  expandSubagentTrace: (subagentId: string) => void;
  // Context panel
  isContextPanelVisible: boolean;
  setContextPanelVisible: (visible: boolean) => void;
  // Context phase selection
  selectedContextPhase: number | null;
  setSelectedContextPhase: (phase: number | null) => void;
  // Scroll position
  savedScrollTop: number | undefined;
  saveScrollPosition: (scrollTop: number) => void;
  // Initialization
  initializeTabUI: () => void;
}

// =============================================================================
// Main Hook (for ChatHistory and other multi-value consumers)
// =============================================================================

/**
 * Hook for accessing per-tab UI state and actions.
 *
 * For hot-path components (AIChatGroup, SubagentItem), prefer the individual
 * hooks (useIsAIGroupExpanded, useExpandedDisplayItemIds, useIsSubagentTraceExpanded)
 * which subscribe to specific values and avoid unnecessary re-renders.
 */
export function useTabUI(): UseTabUIReturn {
  const tabId = useTabIdOptional();

  // Subscribe to individual reactive values (primitives) — NOT the entire tabUIStates Map.
  // Each selector returns a primitive/simple value, so re-renders only trigger when
  // the specific value changes (not when unrelated expansion state changes).
  const isContextPanelVisible = useStore(
    useCallback(
      (s: AppState) => {
        if (!tabId) return false;
        return s.tabUIStates.get(tabId)?.showContextPanel ?? false;
      },
      [tabId]
    )
  );

  const selectedContextPhase = useStore(
    useCallback(
      (s: AppState): number | null => {
        if (!tabId) return null;
        return s.tabUIStates.get(tabId)?.selectedContextPhase ?? null;
      },
      [tabId]
    )
  );

  const savedScrollTop = useStore(
    useCallback(
      (s: AppState): number | undefined => {
        if (!tabId) return undefined;
        return s.tabUIStates.get(tabId)?.savedScrollTop;
      },
      [tabId]
    )
  );

  // Get all tab UI actions from store (these are stable function references)
  const {
    toggleAIGroupExpansionForTab,
    expandAIGroupForTab,
    toggleDisplayItemExpansionForTab,
    expandDisplayItemForTab,
    toggleSubagentTraceExpansionForTab,
    expandSubagentTraceForTab,
    setContextPanelVisibleForTab,
    setSelectedContextPhaseForTab,
    saveScrollPositionForTab,
    initTabUIState,
  } = useStore(
    useShallow((s) => ({
      toggleAIGroupExpansionForTab: s.toggleAIGroupExpansionForTab,
      expandAIGroupForTab: s.expandAIGroupForTab,
      toggleDisplayItemExpansionForTab: s.toggleDisplayItemExpansionForTab,
      expandDisplayItemForTab: s.expandDisplayItemForTab,
      toggleSubagentTraceExpansionForTab: s.toggleSubagentTraceExpansionForTab,
      expandSubagentTraceForTab: s.expandSubagentTraceForTab,
      setContextPanelVisibleForTab: s.setContextPanelVisibleForTab,
      setSelectedContextPhaseForTab: s.setSelectedContextPhaseForTab,
      saveScrollPositionForTab: s.saveScrollPositionForTab,
      initTabUIState: s.initTabUIState,
    }))
  );

  // ==========================================================================
  // Tab-bound action wrappers
  // ==========================================================================

  const toggleAIGroupExpansion = useCallback(
    (aiGroupId: string): void => {
      if (!tabId) return;
      toggleAIGroupExpansionForTab(tabId, aiGroupId);
    },
    [tabId, toggleAIGroupExpansionForTab]
  );

  const expandAIGroup = useCallback(
    (aiGroupId: string): void => {
      if (!tabId) return;
      expandAIGroupForTab(tabId, aiGroupId);
    },
    [tabId, expandAIGroupForTab]
  );

  const toggleDisplayItemExpansion = useCallback(
    (aiGroupId: string, itemId: string): void => {
      if (!tabId) return;
      toggleDisplayItemExpansionForTab(tabId, aiGroupId, itemId);
    },
    [tabId, toggleDisplayItemExpansionForTab]
  );

  const expandDisplayItem = useCallback(
    (aiGroupId: string, itemId: string): void => {
      if (!tabId) return;
      expandDisplayItemForTab(tabId, aiGroupId, itemId);
    },
    [tabId, expandDisplayItemForTab]
  );

  const toggleSubagentTraceExpansion = useCallback(
    (subagentId: string): void => {
      if (!tabId) return;
      toggleSubagentTraceExpansionForTab(tabId, subagentId);
    },
    [tabId, toggleSubagentTraceExpansionForTab]
  );

  const expandSubagentTrace = useCallback(
    (subagentId: string): void => {
      if (!tabId) return;
      expandSubagentTraceForTab(tabId, subagentId);
    },
    [tabId, expandSubagentTraceForTab]
  );

  const setContextPanelVisible = useCallback(
    (visible: boolean): void => {
      if (!tabId) return;
      setContextPanelVisibleForTab(tabId, visible);
    },
    [tabId, setContextPanelVisibleForTab]
  );

  const setSelectedContextPhase = useCallback(
    (phase: number | null): void => {
      if (!tabId) return;
      setSelectedContextPhaseForTab(tabId, phase);
    },
    [tabId, setSelectedContextPhaseForTab]
  );

  const saveScrollPosition = useCallback(
    (scrollTop: number): void => {
      if (!tabId) return;
      saveScrollPositionForTab(tabId, scrollTop);
    },
    [tabId, saveScrollPositionForTab]
  );

  const initializeTabUI = useCallback((): void => {
    if (!tabId) return;
    initTabUIState(tabId);
  }, [tabId, initTabUIState]);

  return {
    tabId,
    toggleAIGroupExpansion,
    expandAIGroup,
    toggleDisplayItemExpansion,
    expandDisplayItem,
    toggleSubagentTraceExpansion,
    expandSubagentTrace,
    isContextPanelVisible,
    setContextPanelVisible,
    selectedContextPhase,
    setSelectedContextPhase,
    savedScrollTop,
    saveScrollPosition,
    initializeTabUI,
  };
}
