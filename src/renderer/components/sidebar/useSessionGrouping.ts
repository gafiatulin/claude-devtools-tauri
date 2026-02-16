/**
 * useSessionGrouping - Encapsulates session grouping, filtering, and sorting logic
 * for the sidebar session list. Handles pinned sessions, hidden sessions,
 * date grouping, and context-based sorting.
 */

import { useMemo } from 'react';

import {
  getNonEmptyCategories,
  groupSessionsByDate,
  separatePinnedSessions,
} from '@renderer/utils/dateGrouping';

import type { Session } from '@renderer/types/data';
import type { DateCategory } from '@renderer/types/tabs';
import type { SessionSortMode } from '@renderer/types/data';

// Virtual list item types
export type VirtualItem =
  | { type: 'header'; category: DateCategory; id: string }
  | { type: 'pinned-header'; id: string }
  | { type: 'session'; session: Session; isPinned: boolean; isHidden: boolean; id: string }
  | { type: 'loader'; id: string };

interface UseSessionGroupingParams {
  sessions: Session[];
  pinnedSessionIds: string[];
  hiddenSessionIds: string[];
  showHiddenSessions: boolean;
  sessionSortMode: SessionSortMode;
  sessionsHasMore: boolean;
}

interface UseSessionGroupingResult {
  hiddenSet: Set<string>;
  virtualItems: VirtualItem[];
}

export function useSessionGrouping({
  sessions,
  pinnedSessionIds,
  hiddenSessionIds,
  showHiddenSessions,
  sessionSortMode,
  sessionsHasMore,
}: UseSessionGroupingParams): UseSessionGroupingResult {
  const hiddenSet = useMemo(() => new Set(hiddenSessionIds), [hiddenSessionIds]);

  // Filter out hidden sessions unless showHiddenSessions is on
  const visibleSessions = useMemo(() => {
    if (showHiddenSessions) return sessions;
    return sessions.filter((s) => !hiddenSet.has(s.id));
  }, [sessions, hiddenSet, showHiddenSessions]);

  // Separate pinned sessions from unpinned
  const { pinned: pinnedSessions, unpinned: unpinnedSessions } = useMemo(
    () => separatePinnedSessions(visibleSessions, pinnedSessionIds),
    [visibleSessions, pinnedSessionIds]
  );

  // Group only unpinned sessions by date
  const groupedSessions = useMemo(() => groupSessionsByDate(unpinnedSessions), [unpinnedSessions]);

  // Get non-empty categories in display order
  const nonEmptyCategories = useMemo(
    () => getNonEmptyCategories(groupedSessions),
    [groupedSessions]
  );

  // Sessions sorted by context consumption (for most-context sort mode)
  const contextSortedSessions = useMemo(() => {
    if (sessionSortMode !== 'most-context') return [];
    return [...visibleSessions].sort(
      (a, b) => (b.contextConsumption ?? 0) - (a.contextConsumption ?? 0)
    );
  }, [visibleSessions, sessionSortMode]);

  // Flatten sessions with date headers into virtual list items
  const virtualItems = useMemo((): VirtualItem[] => {
    const items: VirtualItem[] = [];

    if (sessionSortMode === 'most-context') {
      // Flat list sorted by consumption - no date headers, no pinned section
      for (const session of contextSortedSessions) {
        items.push({
          type: 'session',
          session,
          isPinned: pinnedSessionIds.includes(session.id),
          isHidden: hiddenSet.has(session.id),
          id: `session-${session.id}`,
        });
      }
    } else {
      // Default: date-grouped view with pinned section
      if (pinnedSessions.length > 0) {
        items.push({
          type: 'pinned-header',
          id: 'header-pinned',
        });

        for (const session of pinnedSessions) {
          items.push({
            type: 'session',
            session,
            isPinned: true,
            isHidden: hiddenSet.has(session.id),
            id: `session-${session.id}`,
          });
        }
      }

      for (const category of nonEmptyCategories) {
        items.push({
          type: 'header',
          category,
          id: `header-${category}`,
        });

        for (const session of groupedSessions[category]) {
          items.push({
            type: 'session',
            session,
            isPinned: false,
            isHidden: hiddenSet.has(session.id),
            id: `session-${session.id}`,
          });
        }
      }
    }

    // Add loader item if there are more sessions to load
    if (sessionsHasMore) {
      items.push({
        type: 'loader',
        id: 'loader',
      });
    }

    return items;
  }, [
    sessionSortMode,
    contextSortedSessions,
    pinnedSessionIds,
    hiddenSet,
    pinnedSessions,
    nonEmptyCategories,
    groupedSessions,
    sessionsHasMore,
  ]);

  return { hiddenSet, virtualItems };
}
