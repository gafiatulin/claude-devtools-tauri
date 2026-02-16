/**
 * ContextBadge - Displays a compact badge showing unified context injections.
 * Shows count of NEW injections (CLAUDE.md, mentioned files, tool outputs) with hover popover.
 * Replaces the standalone ClaudeMdBadge with a unified view of all context sources.
 */

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import {
  COLOR_BORDER,
  COLOR_SURFACE_RAISED,
  COLOR_TEXT_SECONDARY,
} from '@renderer/constants/cssVariables';

import { ContextBadgePopover } from './ContextBadgePopover';

import type {
  ClaudeMdContextInjection,
  ContextStats,
  MentionedFileInjection,
  TaskCoordinationInjection,
  ThinkingTextInjection,
  ToolOutputInjection,
  UserMessageInjection,
} from '@renderer/types/contextInjection';

interface ContextBadgeProps {
  stats: ContextStats;
  projectRoot?: string;
}

export const ContextBadge = ({
  stats,
  projectRoot,
}: Readonly<ContextBadgeProps>): React.ReactElement | null => {
  const [showPopover, setShowPopover] = useState(false);
  const [popoverStyle, setPopoverStyle] = useState<React.CSSProperties>({});
  const [arrowStyle, setArrowStyle] = useState<React.CSSProperties>({});
  const containerRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Single-pass categorization and token computation from newInjections
  const injectionStats = useMemo(() => {
    const claudeMd: ClaudeMdContextInjection[] = [];
    const mentionedFiles: MentionedFileInjection[] = [];
    const toolOutputs: ToolOutputInjection[] = [];
    const thinkingText: ThinkingTextInjection[] = [];
    const taskCoordination: TaskCoordinationInjection[] = [];
    const userMessages: UserMessageInjection[] = [];
    let totalTokens = 0;
    let claudeMdTokens = 0;
    let mentionedFileTokens = 0;
    let toolOutputTokens = 0;
    let thinkingTextTokens = 0;
    let taskCoordinationTokens = 0;
    let userMessageTokens = 0;

    for (const inj of stats.newInjections) {
      totalTokens += inj.estimatedTokens;
      switch (inj.category) {
        case 'claude-md':
          claudeMd.push(inj);
          claudeMdTokens += inj.estimatedTokens;
          break;
        case 'mentioned-file':
          mentionedFiles.push(inj);
          mentionedFileTokens += inj.estimatedTokens;
          break;
        case 'tool-output':
          toolOutputs.push(inj);
          toolOutputTokens += inj.estimatedTokens;
          break;
        case 'thinking-text':
          thinkingText.push(inj);
          thinkingTextTokens += inj.estimatedTokens;
          break;
        case 'task-coordination':
          taskCoordination.push(inj);
          taskCoordinationTokens += inj.estimatedTokens;
          break;
        case 'user-message':
          userMessages.push(inj);
          userMessageTokens += inj.estimatedTokens;
          break;
      }
    }

    return {
      totalTokens,
      claudeMd, claudeMdTokens,
      mentionedFiles, mentionedFileTokens,
      toolOutputs, toolOutputTokens,
      thinkingText, thinkingTextTokens,
      taskCoordination, taskCoordinationTokens,
      userMessages, userMessageTokens,
    };
  }, [stats.newInjections]);

  const totalNew =
    stats.newCounts.claudeMd +
    stats.newCounts.mentionedFiles +
    stats.newCounts.toolOutputs +
    stats.newCounts.thinkingText +
    stats.newCounts.taskCoordination +
    stats.newCounts.userMessages;

  const badgeStyle: React.CSSProperties = {
    backgroundColor: COLOR_SURFACE_RAISED,
    border: `1px solid ${COLOR_BORDER}`,
    color: COLOR_TEXT_SECONDARY,
  };

  useEffect(() => {
    if (showPopover && containerRef.current) {
      const rect = containerRef.current.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;
      const popoverWidth = 300;
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
          minWidth: 260,
          maxWidth: 340,
          maxHeight,
          overflowY: 'auto',
          zIndex: 99999,
        });
        setArrowStyle({
          position: 'absolute',
          ...(openAbove
            ? {
                bottom: -4,
                borderRight: `1px solid ${COLOR_BORDER}`,
                borderBottom: `1px solid ${COLOR_BORDER}`,
                borderLeft: 'none',
                borderTop: 'none',
              }
            : {
                top: -4,
                borderLeft: `1px solid ${COLOR_BORDER}`,
                borderTop: `1px solid ${COLOR_BORDER}`,
                borderRight: 'none',
                borderBottom: 'none',
              }),
          [openLeft ? 'right' : 'left']: 12,
          width: 8,
          height: 8,
          transform: 'rotate(45deg)',
          backgroundColor: COLOR_SURFACE_RAISED,
        });
      });
    }
  }, [showPopover]);

  useEffect(() => {
    if (!showPopover) return;

    const isInsideRect = (el: HTMLElement | null, x: number, y: number): boolean => {
      if (!el) return false;
      const rect = el.getBoundingClientRect();
      return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    };

    const handleClickOutside = (e: MouseEvent): void => {
      if (
        isInsideRect(popoverRef.current, e.clientX, e.clientY) ||
        isInsideRect(containerRef.current, e.clientX, e.clientY)
      ) {
        return;
      }
      setShowPopover(false);
    };

    const handleScroll = (e: Event): void => {
      if (popoverRef.current && e.target instanceof Node && popoverRef.current.contains(e.target)) {
        return;
      }
      setShowPopover(false);
    };

    document.addEventListener('mousedown', handleClickOutside);
    window.addEventListener('scroll', handleScroll, true);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      window.removeEventListener('scroll', handleScroll, true);
    };
  }, [showPopover]);

  if (totalNew === 0) {
    return null;
  }

  return (
    <div
      role="button"
      tabIndex={0}
      ref={containerRef}
      className="relative inline-flex"
      onClick={(e) => {
        e.stopPropagation();
        setShowPopover(!showPopover);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          e.stopPropagation();
          setShowPopover(!showPopover);
        }
      }}
    >
      <span
        className="inline-flex cursor-pointer items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
        style={badgeStyle}
      >
        <span>Context</span>
        <span className="font-semibold">+{totalNew}</span>
      </span>

      {showPopover &&
        createPortal(
          <ContextBadgePopover
            popoverRef={popoverRef}
            popoverStyle={popoverStyle}
            arrowStyle={arrowStyle}
            projectRoot={projectRoot}
            totalNewTokens={injectionStats.totalTokens}
            newUserMessageInjections={injectionStats.userMessages}
            newClaudeMdInjections={injectionStats.claudeMd}
            newMentionedFileInjections={injectionStats.mentionedFiles}
            newToolOutputInjections={injectionStats.toolOutputs}
            newTaskCoordinationInjections={injectionStats.taskCoordination}
            newThinkingTextInjections={injectionStats.thinkingText}
            userMessageTokens={injectionStats.userMessageTokens}
            claudeMdTokens={injectionStats.claudeMdTokens}
            mentionedFileTokens={injectionStats.mentionedFileTokens}
            toolOutputTokens={injectionStats.toolOutputTokens}
            taskCoordinationTokens={injectionStats.taskCoordinationTokens}
            thinkingTextTokens={injectionStats.thinkingTextTokens}
          />,
          document.body
        )}
    </div>
  );
};
