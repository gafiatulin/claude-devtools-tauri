/**
 * TokenUsageDisplay - Compact token usage display with detailed breakdown on hover.
 * Shows total tokens with an info icon that reveals a popover with breakdown details.
 */

import React, { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import { COLOR_TEXT_MUTED } from '@renderer/constants/cssVariables';
import { getModelColorClass } from '@shared/utils/modelParser';
import { Info } from 'lucide-react';

import { TokenBreakdownTable } from './TokenBreakdownTable';
import { useTokenCalculations } from './useTokenCalculations';

import type { ClaudeMdStats } from '@renderer/types/claudeMd';
import type { ContextStats } from '@renderer/types/contextInjection';
import type { ModelInfo } from '@shared/utils/modelParser';

interface TokenUsageDisplayProps {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  thinkingTokens?: number;
  textOutputTokens?: number;
  modelName?: string;
  modelFamily?: ModelInfo['family'];
  size?: 'sm' | 'md';
  claudeMdStats?: ClaudeMdStats;
  contextStats?: ContextStats;
  phaseNumber?: number;
  totalPhases?: number;
}

export const TokenUsageDisplay = ({
  inputTokens,
  outputTokens,
  cacheReadTokens,
  cacheCreationTokens,
  thinkingTokens = 0,
  textOutputTokens = 0,
  modelName,
  modelFamily,
  size = 'sm',
  claudeMdStats,
  contextStats,
  phaseNumber,
  totalPhases,
}: Readonly<TokenUsageDisplayProps>): React.JSX.Element => {
  const { totalTokens, formattedTotal } = useTokenCalculations(
    inputTokens,
    outputTokens,
    cacheReadTokens,
    cacheCreationTokens
  );

  const textSize = size === 'sm' ? 'text-xs' : 'text-sm';
  const iconSize = size === 'sm' ? 'w-3 h-3' : 'w-3.5 h-3.5';
  const modelColorClass = modelFamily ? getModelColorClass(modelFamily) : '';

  const [showPopover, setShowPopover] = useState(false);
  const [popoverStyle, setPopoverStyle] = useState<React.CSSProperties>({});
  const [arrowStyle, setArrowStyle] = useState<React.CSSProperties>({});
  const containerRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isDraggingRef = useRef(false);

  const clearHideTimeout = (): void => {
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
  };

  const handleMouseEnter = (): void => {
    clearHideTimeout();
    setShowPopover(true);
  };

  const handleMouseLeave = (): void => {
    if (isDraggingRef.current) return;
    clearHideTimeout();
    hideTimeoutRef.current = setTimeout(() => {
      setShowPopover(false);
    }, 150);
  };

  useEffect(() => {
    return () => clearHideTimeout();
  }, []);

  useEffect(() => {
    if (!showPopover) return;

    const handleScroll = (e: Event): void => {
      if (popoverRef.current && e.target instanceof Node && popoverRef.current.contains(e.target)) {
        return;
      }
      setShowPopover(false);
    };

    window.addEventListener('scroll', handleScroll, true);
    return () => window.removeEventListener('scroll', handleScroll, true);
  }, [showPopover]);

  useEffect(() => {
    if (showPopover && containerRef.current) {
      const rect = containerRef.current.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;
      const popoverWidth = 220;
      const margin = 12;
      const openLeft = rect.left + popoverWidth > viewportWidth - 20;
      const spaceBelow = viewportHeight - rect.bottom - margin;
      const spaceAbove = rect.top - margin;
      const openAbove = spaceBelow < 200 && spaceAbove > spaceBelow;
      const maxHeight = Math.max(openAbove ? spaceAbove : spaceBelow, 120) - 8;

      queueMicrotask(() => {
        setPopoverStyle({
          position: 'fixed',
          ...(openAbove ? { bottom: viewportHeight - rect.top + 4 } : { top: rect.bottom + 4 }),
          left: openLeft ? rect.right - popoverWidth : rect.left,
          minWidth: 200,
          maxWidth: 280,
          maxHeight,
          overflowY: 'auto',
          zIndex: 99999,
        });
        setArrowStyle({
          position: 'absolute',
          ...(openAbove
            ? {
                bottom: -4,
                borderRight: '1px solid var(--color-border)',
                borderBottom: '1px solid var(--color-border)',
                borderLeft: 'none',
                borderTop: 'none',
              }
            : {
                top: -4,
                borderLeft: '1px solid var(--color-border)',
                borderTop: '1px solid var(--color-border)',
                borderRight: 'none',
                borderBottom: 'none',
              }),
          [openLeft ? 'right' : 'left']: 8,
          width: 8,
          height: 8,
          transform: 'rotate(45deg)',
          backgroundColor: 'var(--color-surface-raised)',
        });
      });
    }
  }, [showPopover]);

  return (
    <div
      className={`inline-flex items-center gap-1 ${textSize}`}
      style={{ color: COLOR_TEXT_MUTED }}
    >
      <span className="font-medium">{formattedTotal}</span>
      {totalPhases && totalPhases > 1 && phaseNumber && (
        <span
          className="rounded px-1 py-0.5 text-[10px]"
          style={{ backgroundColor: 'rgba(99, 102, 241, 0.15)', color: '#818cf8' }}
        >
          Phase {phaseNumber}/{totalPhases}
        </span>
      )}
      <div
        ref={containerRef}
        role="button"
        tabIndex={0}
        className="relative"
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        onFocus={handleMouseEnter}
        onBlur={(e) => {
          if (popoverRef.current?.contains(e.relatedTarget as Node)) return;
          handleMouseLeave();
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setShowPopover(!showPopover);
          }
        }}
        aria-expanded={showPopover}
        aria-haspopup="true"
      >
        <Info
          className={`${iconSize} cursor-help transition-colors`}
          style={{ color: COLOR_TEXT_MUTED }}
        />
        {showPopover &&
          createPortal(
            <TokenBreakdownTable
              popoverRef={popoverRef}
              popoverStyle={popoverStyle}
              arrowStyle={arrowStyle}
              inputTokens={inputTokens}
              outputTokens={outputTokens}
              cacheReadTokens={cacheReadTokens}
              cacheCreationTokens={cacheCreationTokens}
              totalTokens={totalTokens}
              thinkingTokens={thinkingTokens}
              textOutputTokens={textOutputTokens}
              modelName={modelName}
              modelColorClass={modelColorClass}
              claudeMdStats={claudeMdStats}
              contextStats={contextStats}
              onMouseEnter={handleMouseEnter}
              onMouseLeave={handleMouseLeave}
            />,
            document.body
          )}
      </div>
    </div>
  );
};
