/**
 * SubagentMetrics - Renders the meta info row and context usage section
 * for an expanded SubagentItem.
 */

import React from 'react';

import {
  CARD_ICON_MUTED,
  CARD_SEPARATOR,
  CARD_TEXT_LIGHT,
  CARD_TEXT_LIGHTER,
  COLOR_TEXT_MUTED,
  COLOR_TEXT_SECONDARY,
} from '@renderer/constants/cssVariables';
import { formatDuration, formatTokensCompact } from '@renderer/utils/formatters';
import { getModelColorClass } from '@shared/utils/modelParser';
import { ArrowUpRight, CircleDot, Sigma } from 'lucide-react';

import type { computeSubagentPhaseBreakdown } from '@renderer/utils/aiGroupHelpers';
import type { ModelInfo } from '@shared/utils/modelParser';

interface SubagentMetricsProps {
  subagentId: string;
  subagentType: string;
  durationMs?: number;
  modelInfo: { name: string; family: ModelInfo['family'] } | null;
  isTeamMember: boolean;
  mainSessionImpact?: { callTokens: number; resultTokens: number; totalTokens: number };
  lastUsage: {
    input_tokens: number;
    output_tokens: number;
    cache_read_input_tokens?: number;
    cache_creation_input_tokens?: number;
  } | null;
  phaseData: ReturnType<typeof computeSubagentPhaseBreakdown> | null;
  cumulativeMetrics?: { outputTokens: number; turnCount: number };
}

export const SubagentMetrics = React.memo(function SubagentMetrics({
  subagentId,
  subagentType,
  durationMs,
  modelInfo,
  isTeamMember,
  mainSessionImpact,
  lastUsage,
  phaseData,
  cumulativeMetrics,
}: SubagentMetricsProps): React.JSX.Element {
  const hasMainImpact = mainSessionImpact && mainSessionImpact.totalTokens > 0;
  const hasIsolated = lastUsage && lastUsage.input_tokens + lastUsage.output_tokens > 0;
  const isMultiPhase = phaseData != null && phaseData.compactionCount > 0;
  const isolatedTotal = isMultiPhase
    ? phaseData.totalConsumption
    : lastUsage
      ? lastUsage.input_tokens +
        lastUsage.output_tokens +
        (lastUsage.cache_read_input_tokens ?? 0) +
        (lastUsage.cache_creation_input_tokens ?? 0)
      : 0;

  return (
    <>
      {/* Row 1: Meta Info (Horizontal Flow) */}
      <div
        className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px]"
        style={{ color: COLOR_TEXT_MUTED }}
      >
        <span>
          <span style={{ color: CARD_ICON_MUTED }}>Type</span>{' '}
          <span className="font-mono" style={{ color: CARD_TEXT_LIGHT }}>
            {subagentType}
          </span>
        </span>
        <span style={{ color: CARD_SEPARATOR }}>&#8226;</span>
        <span>
          <span style={{ color: CARD_ICON_MUTED }}>Duration</span>{' '}
          <span className="font-mono tabular-nums" style={{ color: CARD_TEXT_LIGHT }}>
            {formatDuration(durationMs ?? 0)}
          </span>
        </span>
        {modelInfo && (
          <>
            <span style={{ color: CARD_SEPARATOR }}>&#8226;</span>
            <span>
              <span style={{ color: CARD_ICON_MUTED }}>Model</span>{' '}
              <span className={`font-mono ${getModelColorClass(modelInfo.family)}`}>
                {modelInfo.name}
              </span>
            </span>
          </>
        )}
        <span style={{ color: CARD_SEPARATOR }}>&#8226;</span>
        <span>
          <span style={{ color: CARD_ICON_MUTED }}>ID</span>{' '}
          <span
            className="inline-block max-w-[120px] truncate align-bottom font-mono"
            style={{ color: CARD_ICON_MUTED }}
            title={subagentId}
          >
            {subagentId.slice(0, 8)}
          </span>
        </span>
      </div>

      {/* Row 2: Context Usage (Clean List) */}
      {(hasMainImpact ?? hasIsolated) && (
        <div className="pt-2">
          <div
            className="mb-2 text-[10px] font-semibold uppercase tracking-wider"
            style={{ color: CARD_ICON_MUTED }}
          >
            Context Usage
          </div>

          <div className="space-y-1.5">
            {hasMainImpact && !isTeamMember && (
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <ArrowUpRight
                    className="size-3"
                    style={{ color: 'rgba(251, 191, 36, 0.7)' }}
                  />
                  <span className="text-xs" style={{ color: COLOR_TEXT_SECONDARY }}>
                    Main Context
                  </span>
                </div>
                <span
                  className="font-mono text-xs font-medium tabular-nums"
                  style={{ color: CARD_TEXT_LIGHTER }}
                >
                  {mainSessionImpact!.totalTokens.toLocaleString()}
                </span>
              </div>
            )}

            {cumulativeMetrics && (
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Sigma className="size-3" style={{ color: 'rgba(168, 85, 247, 0.7)' }} />
                  <span className="text-xs" style={{ color: COLOR_TEXT_SECONDARY }}>
                    Total Output
                  </span>
                </div>
                <span
                  className="font-mono text-xs font-medium tabular-nums"
                  style={{ color: CARD_TEXT_LIGHTER }}
                >
                  {cumulativeMetrics.outputTokens.toLocaleString()}
                  <span style={{ color: CARD_ICON_MUTED }}>
                    {' '}
                    ({cumulativeMetrics.turnCount} turns)
                  </span>
                </span>
              </div>
            )}

            {hasIsolated && (
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <CircleDot className="size-3" style={{ color: 'rgba(56, 189, 248, 0.7)' }} />
                  <span className="text-xs" style={{ color: COLOR_TEXT_SECONDARY }}>
                    {isTeamMember ? 'Context Window' : 'Subagent Context'}
                  </span>
                </div>
                <span
                  className="font-mono text-xs font-medium tabular-nums"
                  style={{ color: CARD_TEXT_LIGHTER }}
                >
                  {isolatedTotal.toLocaleString()}
                </span>
              </div>
            )}

            {isMultiPhase &&
              phaseData.phases.map((phase) => (
                <div key={phase.phaseNumber} className="flex items-center justify-between pl-5">
                  <span className="text-[11px]" style={{ color: CARD_ICON_MUTED }}>
                    Phase {phase.phaseNumber}
                  </span>
                  <span
                    className="font-mono text-[11px] tabular-nums"
                    style={{ color: CARD_ICON_MUTED }}
                  >
                    {formatTokensCompact(phase.peakTokens)}
                    {phase.postCompaction != null && (
                      <span style={{ color: '#4ade80' }}>
                        {' '}
                        &rarr; {formatTokensCompact(phase.postCompaction)}
                      </span>
                    )}
                  </span>
                </div>
              ))}
          </div>
        </div>
      )}
    </>
  );
});
