/**
 * SubagentTrace - Renders the expandable execution trace section
 * within an expanded SubagentItem.
 */

import React, { useState } from 'react';

import {
  CARD_BORDER_STYLE,
  CARD_HEADER_BG,
  CARD_HEADER_HOVER,
  CARD_ICON_MUTED,
  COLOR_TEXT_SECONDARY,
} from '@renderer/constants/cssVariables';
import { ChevronRight, Terminal } from 'lucide-react';

import { ExecutionTrace } from './ExecutionTrace';

import type { AIGroupDisplayItem } from '@renderer/types/groups';
import type { TriggerColor } from '@shared/constants/triggerColors';

interface SubagentTraceProps {
  displayItems: AIGroupDisplayItem[];
  aiGroupId: string;
  subagentId: string;
  isTraceExpanded: boolean;
  toggleSubagentTraceExpansion: (subagentId: string) => void;
  itemsSummary: string;
  highlightToolUseId?: string;
  highlightColor?: TriggerColor;
  notificationColorMap?: Map<string, TriggerColor>;
  shouldExpandForSearch: boolean;
  searchCurrentSubagentItemId: string | null;
  registerToolRef?: (toolId: string, el: HTMLDivElement | null) => void;
}

export const SubagentTrace = React.memo(function SubagentTrace({
  displayItems,
  aiGroupId,
  subagentId,
  isTraceExpanded,
  toggleSubagentTraceExpansion,
  itemsSummary,
  highlightToolUseId,
  highlightColor,
  notificationColorMap,
  shouldExpandForSearch,
  searchCurrentSubagentItemId,
  registerToolRef,
}: SubagentTraceProps): React.JSX.Element | null {
  const [isTraceHeaderHovered, setIsTraceHeaderHovered] = useState(false);

  if (displayItems.length === 0) {
    return null;
  }

  return (
    <div
      className="overflow-hidden rounded-md"
      style={{
        border: CARD_BORDER_STYLE,
        backgroundColor: CARD_HEADER_BG,
      }}
    >
      {/* Trace Header (clickable) */}
      <div
        role="button"
        tabIndex={0}
        onClick={(e) => {
          e.stopPropagation();
          toggleSubagentTraceExpansion(subagentId);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            e.stopPropagation();
            toggleSubagentTraceExpansion(subagentId);
          }
        }}
        className="flex cursor-pointer items-center gap-2 px-3 py-2 transition-colors"
        style={{
          borderBottom: isTraceExpanded ? CARD_BORDER_STYLE : 'none',
          backgroundColor: isTraceHeaderHovered ? CARD_HEADER_HOVER : 'transparent',
        }}
        onMouseEnter={() => setIsTraceHeaderHovered(true)}
        onMouseLeave={() => setIsTraceHeaderHovered(false)}
      >
        <ChevronRight
          className={`size-3 shrink-0 transition-transform ${isTraceExpanded ? 'rotate-90' : ''}`}
          style={{ color: CARD_ICON_MUTED }}
        />
        <Terminal className="size-3.5" style={{ color: CARD_ICON_MUTED }} />
        <span className="text-xs" style={{ color: COLOR_TEXT_SECONDARY }}>
          Execution Trace
        </span>
        <span className="text-[11px]" style={{ color: CARD_ICON_MUTED }}>
          &middot; {itemsSummary}
        </span>
      </div>

      {/* Trace Content */}
      {isTraceExpanded && (
        <div className="p-2">
          <ExecutionTrace
            items={displayItems}
            aiGroupId={aiGroupId}
            highlightToolUseId={highlightToolUseId}
            highlightColor={highlightColor}
            notificationColorMap={notificationColorMap}
            searchExpandedItemId={shouldExpandForSearch ? searchCurrentSubagentItemId : null}
            registerToolRef={registerToolRef}
          />
        </div>
      )}
    </div>
  );
});
