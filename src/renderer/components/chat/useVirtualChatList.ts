/**
 * useVirtualChatList - Encapsulates virtual list setup for ChatHistory.
 * Manages the @tanstack/react-virtual virtualizer instance, group index mapping,
 * and the ensureGroupVisible helper for scroll-to-index navigation.
 */

import { useCallback, useEffect, useMemo } from 'react';

import { useVirtualizer } from '@tanstack/react-virtual';

import type { SessionConversation } from '@renderer/types/groups';

/**
 * Waits for two requestAnimationFrame cycles, allowing the virtualizer to render.
 */
function waitForDoubleRaf(): Promise<void> {
  return new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  );
}

interface UseVirtualChatListParams {
  conversation: SessionConversation | null;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  virtualizationThreshold?: number;
  estimatedItemHeight?: number;
  overscan?: number;
}

interface UseVirtualChatListResult {
  shouldVirtualize: boolean;
  rowVirtualizer: ReturnType<typeof useVirtualizer<HTMLDivElement, Element>>;
  groupIndexMap: Map<string, number>;
  ensureGroupVisible: (groupId: string) => Promise<void>;
}

export function useVirtualChatList({
  conversation,
  scrollContainerRef,
  virtualizationThreshold = 120,
  estimatedItemHeight = 260,
  overscan = 15,
}: UseVirtualChatListParams): UseVirtualChatListResult {
  const shouldVirtualize = (conversation?.items.length ?? 0) >= virtualizationThreshold;

  const groupIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    if (!conversation?.items) {
      return map;
    }
    conversation.items.forEach((item, index) => {
      map.set(item.group.id, index);
    });
    return map;
  }, [conversation]);

  const rowVirtualizer = useVirtualizer({
    count: shouldVirtualize ? (conversation?.items.length ?? 0) : 0,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => estimatedItemHeight,
    overscan,
  });

  // Force recalculation after the scroll container mounts.
  // The virtualizer's internal onChange may not trigger a parent re-render
  // (e.g., when ChatScrollContainer props haven't changed by reference),
  // so we explicitly force a recalculation once the scroll element is available.
  useEffect(() => {
    if (shouldVirtualize && scrollContainerRef.current && rowVirtualizer.getVirtualItems().length === 0) {
      rowVirtualizer.measure();
    }
  }, [shouldVirtualize, rowVirtualizer, scrollContainerRef]);

  const ensureGroupVisible = useCallback(
    async (groupId: string) => {
      if (!shouldVirtualize) {
        return;
      }
      const index = groupIndexMap.get(groupId);
      if (index === undefined) {
        return;
      }
      rowVirtualizer.scrollToIndex(index, { align: 'center' });
      // Wait 2 RAF frames so the virtualizer has time to render the target row
      await waitForDoubleRaf();
    },
    [groupIndexMap, rowVirtualizer, shouldVirtualize]
  );

  return {
    shouldVirtualize,
    rowVirtualizer,
    groupIndexMap,
    ensureGroupVisible,
  };
}
