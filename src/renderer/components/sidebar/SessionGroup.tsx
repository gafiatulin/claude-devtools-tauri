/**
 * SessionGroup - Renders a single virtual row in the date-grouped sessions list.
 * Handles pinned headers, date headers, loader items, and session items.
 */

import React from 'react';

import { Loader2, Pin } from 'lucide-react';

import { SessionItem } from './SessionItem';

import type { VirtualItem } from './useSessionGrouping';

interface SessionGroupProps {
  item: VirtualItem;
  selectedSessionId: string | null;
  selectedSet: Set<string>;
  sidebarMultiSelectActive: boolean;
  sessionsLoadingMore: boolean;
  toggleSidebarSessionSelection: (sessionId: string) => void;
}

export const SessionGroup = React.memo(function SessionGroup({
  item,
  selectedSessionId,
  selectedSet,
  sidebarMultiSelectActive,
  sessionsLoadingMore,
  toggleSidebarSessionSelection,
}: SessionGroupProps): React.JSX.Element | null {
  if (item.type === 'pinned-header') {
    return (
      <div
        className="sticky top-0 flex h-full items-center gap-1.5 border-t px-4 py-1.5 text-[11px] font-semibold uppercase tracking-wider backdrop-blur-sm"
        style={{
          backgroundColor: 'color-mix(in srgb, var(--color-surface-sidebar) 95%, transparent)',
          color: 'var(--color-text-muted)',
          borderColor: 'var(--color-border-emphasis)',
        }}
      >
        <Pin className="size-3" />
        Pinned
      </div>
    );
  }

  if (item.type === 'header') {
    return (
      <div
        className="sticky top-0 flex h-full items-center border-t px-4 py-1.5 text-[11px] font-semibold uppercase tracking-wider backdrop-blur-sm"
        style={{
          backgroundColor: 'color-mix(in srgb, var(--color-surface-sidebar) 95%, transparent)',
          color: 'var(--color-text-muted)',
          borderColor: 'var(--color-border-emphasis)',
        }}
      >
        {item.category}
      </div>
    );
  }

  if (item.type === 'loader') {
    return (
      <div
        className="flex h-full items-center justify-center"
        style={{ color: 'var(--color-text-muted)' }}
      >
        {sessionsLoadingMore ? (
          <>
            <Loader2 className="mr-2 size-4 animate-spin" />
            <span className="text-xs">Loading more sessions...</span>
          </>
        ) : (
          <span className="text-xs opacity-50">Scroll to load more</span>
        )}
      </div>
    );
  }

  return (
    <SessionItem
      session={item.session}
      isActive={selectedSessionId === item.session.id}
      isPinned={item.isPinned}
      isHidden={item.isHidden}
      multiSelectActive={sidebarMultiSelectActive}
      isSelected={selectedSet.has(item.session.id)}
      onToggleSelect={() => toggleSidebarSessionSelection(item.session.id)}
    />
  );
});
