/**
 * TokenBreakdownTable - The token breakdown popover content for TokenUsageDisplay.
 * Shows input, cache, output tokens breakdown with optional context stats section.
 */

import React, { useState } from 'react';

import { COLOR_TEXT_MUTED, COLOR_TEXT_SECONDARY } from '@renderer/constants/cssVariables';
import { formatTokensCompact as formatTokens, formatTokensDetailed } from '@shared/utils/tokenFormatting';
import { ChevronRight } from 'lucide-react';

import type { ClaudeMdStats } from '@renderer/types/claudeMd';
import type { ContextStats } from '@renderer/types/contextInjection';

// =============================================================================
// SessionContextSection (internal)
// =============================================================================

const SessionContextSection = ({
  contextStats,
  totalTokens,
  thinkingTokens = 0,
  textOutputTokens = 0,
}: Readonly<{
  contextStats: ContextStats;
  totalTokens: number;
  thinkingTokens?: number;
  textOutputTokens?: number;
}>): React.JSX.Element => {
  const [expanded, setExpanded] = useState(false);

  const { tokensByCategory } = contextStats;
  const thinkingTextTokens = thinkingTokens + textOutputTokens;
  const adjustedContextTotal = contextStats.totalEstimatedTokens + thinkingTextTokens;
  const contextPercent =
    totalTokens > 0 ? Math.min((adjustedContextTotal / totalTokens) * 100, 100).toFixed(1) : '0.0';

  const claudeMdCount = contextStats.accumulatedInjections.filter(
    (inj) => inj.category === 'claude-md'
  ).length;
  const mentionedFilesCount = contextStats.accumulatedInjections.filter(
    (inj) => inj.category === 'mentioned-file'
  ).length;
  const toolOutputsCount = contextStats.accumulatedInjections.filter(
    (inj) => inj.category === 'tool-output'
  ).length;
  const taskCoordinationCount = contextStats.accumulatedInjections.filter(
    (inj) => inj.category === 'task-coordination'
  ).length;
  const userMessagesCount = contextStats.accumulatedInjections.filter(
    (inj) => inj.category === 'user-message'
  ).length;

  const pct = (v: number): string =>
    totalTokens > 0 ? Math.min((v / totalTokens) * 100, 100).toFixed(1) : '0.0';

  return (
    <div className="mt-1">
      <div className="my-1" style={{ borderTop: '1px solid var(--color-border-subtle)' }} />
      <div
        role="button"
        tabIndex={0}
        className="-mx-1 flex cursor-pointer items-center justify-between gap-3 rounded px-1 py-0.5 transition-colors hover:bg-white/5"
        onClick={() => setExpanded(!expanded)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setExpanded(!expanded);
          }
        }}
      >
        <div className="flex items-center gap-1" style={{ color: COLOR_TEXT_MUTED }}>
          <ChevronRight
            className={`size-3 shrink-0 transition-transform duration-150 ${expanded ? 'rotate-90' : ''}`}
          />
          <span className="whitespace-nowrap text-[10px]">Visible Context</span>
        </div>
        <span
          className="whitespace-nowrap text-[10px] tabular-nums"
          style={{ color: COLOR_TEXT_MUTED }}
        >
          {formatTokens(adjustedContextTotal)} ({contextPercent}%)
        </span>
      </div>

      {expanded && (
        <div className="mt-1 space-y-1.5 pl-4">
          {tokensByCategory.claudeMd > 0 && (
            <div className="flex items-center justify-between text-[10px]">
              <span style={{ color: COLOR_TEXT_MUTED }}>
                CLAUDE.md <span className="opacity-60">&times;{claudeMdCount}</span>
              </span>
              <span className="tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
                {formatTokens(tokensByCategory.claudeMd)}{' '}
                <span className="opacity-60">({pct(tokensByCategory.claudeMd)}%)</span>
              </span>
            </div>
          )}
          {tokensByCategory.mentionedFiles > 0 && (
            <div className="flex items-center justify-between text-[10px]">
              <span style={{ color: COLOR_TEXT_MUTED }}>
                @files <span className="opacity-60">&times;{mentionedFilesCount}</span>
              </span>
              <span className="tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
                {formatTokens(tokensByCategory.mentionedFiles)}{' '}
                <span className="opacity-60">({pct(tokensByCategory.mentionedFiles)}%)</span>
              </span>
            </div>
          )}
          {tokensByCategory.toolOutputs > 0 && (
            <div className="flex items-center justify-between text-[10px]">
              <span style={{ color: COLOR_TEXT_MUTED }}>
                Tool Outputs <span className="opacity-60">&times;{toolOutputsCount}</span>
              </span>
              <span className="tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
                {formatTokens(tokensByCategory.toolOutputs)}{' '}
                <span className="opacity-60">({pct(tokensByCategory.toolOutputs)}%)</span>
              </span>
            </div>
          )}
          {tokensByCategory.taskCoordination > 0 && (
            <div className="flex items-center justify-between text-[10px]">
              <span style={{ color: COLOR_TEXT_MUTED }}>
                Task Coordination <span className="opacity-60">&times;{taskCoordinationCount}</span>
              </span>
              <span className="tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
                {formatTokens(tokensByCategory.taskCoordination)}{' '}
                <span className="opacity-60">({pct(tokensByCategory.taskCoordination)}%)</span>
              </span>
            </div>
          )}
          {tokensByCategory.userMessages > 0 && (
            <div className="flex items-center justify-between text-[10px]">
              <span style={{ color: COLOR_TEXT_MUTED }}>
                User Messages <span className="opacity-60">&times;{userMessagesCount}</span>
              </span>
              <span className="tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
                {formatTokens(tokensByCategory.userMessages)}{' '}
                <span className="opacity-60">({pct(tokensByCategory.userMessages)}%)</span>
              </span>
            </div>
          )}
          {thinkingTextTokens > 0 && (
            <div className="flex items-center justify-between text-[10px]">
              <span style={{ color: COLOR_TEXT_MUTED }}>Thinking + Text</span>
              <span className="tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
                {formatTokens(thinkingTextTokens)}{' '}
                <span className="opacity-60">({pct(thinkingTextTokens)}%)</span>
              </span>
            </div>
          )}
          <div
            className="pt-0.5 text-[9px] italic"
            style={{ color: COLOR_TEXT_MUTED, opacity: 0.7 }}
          >
            Accumulated across entire session without duplication
          </div>
        </div>
      )}
    </div>
  );
};

// =============================================================================
// TokenBreakdownTable
// =============================================================================

interface TokenBreakdownTableProps {
  popoverRef: React.RefObject<HTMLDivElement | null>;
  popoverStyle: React.CSSProperties;
  arrowStyle: React.CSSProperties;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalTokens: number;
  thinkingTokens?: number;
  textOutputTokens?: number;
  modelName?: string;
  modelColorClass: string;
  claudeMdStats?: ClaudeMdStats;
  contextStats?: ContextStats;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
}

export const TokenBreakdownTable = React.memo(function TokenBreakdownTable({
  popoverRef,
  popoverStyle,
  arrowStyle,
  inputTokens,
  outputTokens,
  cacheReadTokens,
  cacheCreationTokens,
  totalTokens,
  thinkingTokens = 0,
  textOutputTokens = 0,
  modelName,
  modelColorClass,
  claudeMdStats,
  contextStats,
  onMouseEnter,
  onMouseLeave,
}: TokenBreakdownTableProps): React.JSX.Element {
  return (
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/click-events-have-key-events -- tooltip uses mouse handlers for hover/drag behavior, not interactive
    <div
      ref={popoverRef as React.Ref<HTMLDivElement>}
      role="tooltip"
      className="rounded-lg p-3 shadow-xl"
      style={{
        ...popoverStyle,
        backgroundColor: 'var(--color-surface-raised)',
        border: '1px solid var(--color-border)',
        boxShadow: '0 10px 25px -5px rgba(0, 0, 0, 0.3)',
      }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onMouseDown={(e) => {
        e.stopPropagation();
      }}
      onClick={(e) => e.stopPropagation()}
    >
      <div style={arrowStyle} />

      <div className="space-y-2 text-xs">
        <div className="flex items-center justify-between">
          <span style={{ color: COLOR_TEXT_MUTED }}>Input Tokens</span>
          <span className="font-medium tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
            {formatTokensDetailed(inputTokens)}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: COLOR_TEXT_MUTED }}>Cache Read</span>
          <span className="font-medium tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
            {formatTokensDetailed(cacheReadTokens)}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: COLOR_TEXT_MUTED }}>Cache Write</span>
          <span className="font-medium tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
            {formatTokensDetailed(cacheCreationTokens)}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: COLOR_TEXT_MUTED }}>Output Tokens</span>
          <span className="font-medium tabular-nums" style={{ color: COLOR_TEXT_SECONDARY }}>
            {formatTokensDetailed(outputTokens)}
          </span>
        </div>
        <div className="my-1" style={{ borderTop: '1px solid var(--color-border-subtle)' }} />
        <div className="flex items-center justify-between">
          <span className="font-medium" style={{ color: COLOR_TEXT_SECONDARY }}>
            Total
          </span>
          <span
            className="font-medium tabular-nums"
            style={{ color: 'var(--color-text-primary, var(--color-text))' }}
          >
            {formatTokensDetailed(totalTokens)}
          </span>
        </div>

        {contextStats &&
          (contextStats.totalEstimatedTokens > 0 ||
            thinkingTokens > 0 ||
            textOutputTokens > 0) && (
            <SessionContextSection
              contextStats={contextStats}
              totalTokens={totalTokens}
              thinkingTokens={thinkingTokens}
              textOutputTokens={textOutputTokens}
            />
          )}

        {!contextStats && claudeMdStats && (
          <div
            className="mt-1 flex items-center justify-between text-[10px]"
            style={{ color: COLOR_TEXT_MUTED }}
          >
            <span className="whitespace-nowrap italic">
              incl. CLAUDE.md &times;{claudeMdStats.accumulatedCount}
            </span>
            <span className="tabular-nums">
              {totalTokens > 0
                ? ((claudeMdStats.totalEstimatedTokens / totalTokens) * 100).toFixed(1)
                : '0.0'}
              %
            </span>
          </div>
        )}

        {modelName && (
          <>
            <div className="my-1" style={{ borderTop: '1px solid var(--color-border-subtle)' }} />
            <div className="flex items-center justify-between">
              <span style={{ color: COLOR_TEXT_MUTED }}>Model</span>
              <span
                className={`font-medium ${modelColorClass}`}
                style={!modelColorClass ? { color: COLOR_TEXT_SECONDARY } : {}}
              >
                {modelName}
              </span>
            </div>
          </>
        )}
      </div>
    </div>
  );
});
