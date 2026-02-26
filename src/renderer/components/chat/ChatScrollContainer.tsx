/**
 * ChatScrollContainer - The scroll container and virtualized/non-virtualized
 * chat item rendering for ChatHistory.
 * Handles the sticky context button, virtual rows, and non-virtual item mapping.
 */

import React from 'react';

import { ChatHistoryItem } from './ChatHistoryItem';

import type { TriggerColor } from '@shared/constants/triggerColors';
import type { ContextInjection } from '@renderer/types/contextInjection';
import type { SessionConversation } from '@renderer/types/groups';

interface ChatScrollContainerProps {
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  conversation: SessionConversation;
  shouldVirtualize: boolean;
  topBanner?: React.ReactNode;
  bottomBanner?: React.ReactNode;
  rowVirtualizer: {
    getTotalSize: () => number;
    getVirtualItems: () => Array<{
      key: React.Key;
      index: number;
      start: number;
    }>;
    measureElement: (el: Element | null) => void;
  };
  allContextInjections: ContextInjection[];
  isContextPanelVisible: boolean;
  setContextPanelVisible: (visible: boolean) => void;
  isContextButtonHovered: boolean;
  setIsContextButtonHovered: (hovered: boolean) => void;
  highlightedGroupId: string | null;
  effectiveHighlightToolUseId: string | undefined;
  isSearchHighlight: boolean;
  isNavigationHighlight: boolean;
  effectiveHighlightColor: TriggerColor | 'blue';
  registerChatItemRef: (groupId: string) => (el: HTMLElement | null) => void;
  registerAIGroupRefCombined: (groupId: string) => (el: HTMLElement | null) => void;
  registerToolRef: (toolId: string, el: HTMLElement | null) => void;
}

export const ChatScrollContainer = function ChatScrollContainer({
  scrollContainerRef,
  conversation,
  shouldVirtualize,
  topBanner,
  bottomBanner,
  rowVirtualizer,
  allContextInjections,
  isContextPanelVisible,
  setContextPanelVisible,
  isContextButtonHovered,
  setIsContextButtonHovered,
  highlightedGroupId,
  effectiveHighlightToolUseId,
  isSearchHighlight,
  isNavigationHighlight,
  effectiveHighlightColor,
  registerChatItemRef,
  registerAIGroupRefCombined,
  registerToolRef,
}: ChatScrollContainerProps): React.JSX.Element {
  return (
    <div
      ref={scrollContainerRef as React.Ref<HTMLDivElement>}
      className="flex-1 overflow-y-auto"
      style={{ backgroundColor: 'var(--color-surface)' }}
    >
      {/* Sticky Context button */}
      {allContextInjections.length > 0 && (
        <div className="pointer-events-none sticky top-0 z-10 flex justify-end px-4 pb-0 pt-3">
          <button
            onClick={() => setContextPanelVisible(!isContextPanelVisible)}
            onMouseEnter={() => setIsContextButtonHovered(true)}
            onMouseLeave={() => setIsContextButtonHovered(false)}
            className="pointer-events-auto flex items-center gap-1 rounded-md px-2.5 py-1.5 text-xs shadow-lg backdrop-blur-md transition-colors"
            style={{
              backgroundColor: isContextPanelVisible
                ? 'var(--context-btn-active-bg)'
                : isContextButtonHovered
                  ? 'var(--context-btn-bg-hover)'
                  : 'var(--context-btn-bg)',
              color: isContextPanelVisible
                ? 'var(--context-btn-active-text)'
                : 'var(--color-text-secondary)',
            }}
          >
            Context ({allContextInjections.length})
          </button>
        </div>
      )}
      <div
        className="mx-auto max-w-5xl px-6 py-8"
        style={{ marginTop: allContextInjections.length > 0 ? '-2rem' : 0 }}
      >
        <div className="space-y-8">
          {topBanner}
          {shouldVirtualize ? (
            <div
              style={{
                height: `${rowVirtualizer.getTotalSize()}px`,
                width: '100%',
                position: 'relative',
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const item = conversation.items[virtualRow.index];
                if (!item) return null;
                return (
                  <div
                    key={virtualRow.key}
                    ref={rowVirtualizer.measureElement}
                    data-index={virtualRow.index}
                    className="pb-8"
                    style={{
                      position: 'absolute',
                      top: 0,
                      left: 0,
                      width: '100%',
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    <ChatHistoryItem
                      item={item}
                      highlightedGroupId={highlightedGroupId}
                      highlightToolUseId={effectiveHighlightToolUseId}
                      isSearchHighlight={isSearchHighlight}
                      isNavigationHighlight={isNavigationHighlight}
                      highlightColor={effectiveHighlightColor}
                      registerChatItemRef={registerChatItemRef}
                      registerAIGroupRef={registerAIGroupRefCombined}
                      registerToolRef={registerToolRef}
                    />
                  </div>
                );
              })}
            </div>
          ) : (
            conversation.items.map((item) => (
              <ChatHistoryItem
                key={item.group.id}
                item={item}
                highlightedGroupId={highlightedGroupId}
                highlightToolUseId={effectiveHighlightToolUseId}
                isSearchHighlight={isSearchHighlight}
                isNavigationHighlight={isNavigationHighlight}
                highlightColor={effectiveHighlightColor}
                registerChatItemRef={registerChatItemRef}
                registerAIGroupRef={registerAIGroupRefCombined}
                registerToolRef={registerToolRef}
              />
            ))
          )}
          {bottomBanner}
        </div>
      </div>
    </div>
  );
}
