/**
 * ContextBadgePopover - The popover content for the ContextBadge.
 * Shows sections for each injection category with token counts.
 */

import React, { useState } from 'react';

import {
  COLOR_BORDER_SUBTLE,
  COLOR_TEXT,
  COLOR_TEXT_MUTED,
  COLOR_TEXT_SECONDARY,
} from '@renderer/constants/cssVariables';
import { resolveAbsolutePath, shortenDisplayPath } from '@renderer/utils/pathDisplay';
import { formatTokensCompact as formatTokens } from '@shared/utils/tokenFormatting';
import { ChevronRight } from 'lucide-react';

import { CopyablePath } from '../common/CopyablePath';

import type {
  ClaudeMdContextInjection,
  MentionedFileInjection,
  TaskCoordinationInjection,
  ThinkingTextInjection,
  ToolOutputInjection,
  UserMessageInjection,
} from '@renderer/types/contextInjection';

// =============================================================================
// PopoverSection
// =============================================================================

const PopoverSection = ({
  title,
  count,
  tokenCount,
  children,
  defaultExpanded = false,
}: Readonly<{
  title: string;
  count: number;
  tokenCount: number;
  children: React.ReactNode;
  defaultExpanded?: boolean;
}>): React.ReactElement => {
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        className="mb-1 flex cursor-pointer items-center gap-1 text-xs font-medium hover:opacity-80"
        style={{ color: COLOR_TEXT_MUTED }}
        onClick={(e) => {
          e.stopPropagation();
          setExpanded(!expanded);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            e.stopPropagation();
            setExpanded(!expanded);
          }
        }}
      >
        <ChevronRight
          className={`size-3 shrink-0 transition-transform ${expanded ? 'rotate-90' : ''}`}
        />
        <span>
          {title} ({count}) ~{formatTokens(tokenCount)} tokens
        </span>
      </div>
      {expanded && <div className="space-y-1.5 pl-4">{children}</div>}
    </div>
  );
};

// =============================================================================
// ContextBadgePopover
// =============================================================================

interface ContextBadgePopoverProps {
  popoverRef: React.RefObject<HTMLDivElement | null>;
  popoverStyle: React.CSSProperties;
  arrowStyle: React.CSSProperties;
  projectRoot?: string;
  totalNewTokens: number;
  newUserMessageInjections: UserMessageInjection[];
  newClaudeMdInjections: ClaudeMdContextInjection[];
  newMentionedFileInjections: MentionedFileInjection[];
  newToolOutputInjections: ToolOutputInjection[];
  newTaskCoordinationInjections: TaskCoordinationInjection[];
  newThinkingTextInjections: ThinkingTextInjection[];
  userMessageTokens: number;
  claudeMdTokens: number;
  mentionedFileTokens: number;
  toolOutputTokens: number;
  taskCoordinationTokens: number;
  thinkingTextTokens: number;
}

export const ContextBadgePopover = React.memo(function ContextBadgePopover({
  popoverRef,
  popoverStyle,
  arrowStyle,
  projectRoot,
  totalNewTokens,
  newUserMessageInjections,
  newClaudeMdInjections,
  newMentionedFileInjections,
  newToolOutputInjections,
  newTaskCoordinationInjections,
  newThinkingTextInjections,
  userMessageTokens,
  claudeMdTokens,
  mentionedFileTokens,
  toolOutputTokens,
  taskCoordinationTokens,
  thinkingTextTokens,
}: ContextBadgePopoverProps): React.ReactElement {
  return (
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/click-events-have-key-events -- dialog uses stopPropagation only, not interactive
    <div
      ref={popoverRef as React.Ref<HTMLDivElement>}
      role="dialog"
      aria-modal="false"
      aria-label="Context injection details"
      className="rounded-lg p-3 shadow-xl"
      style={{
        ...popoverStyle,
        backgroundColor: 'var(--color-surface-raised)',
        border: '1px solid var(--color-border)',
        boxShadow: '0 10px 25px -5px rgba(0, 0, 0, 0.3)',
      }}
      onClick={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <div style={arrowStyle} />

      <div
        className="mb-2 pb-2 text-xs font-semibold"
        style={{
          color: COLOR_TEXT,
          borderBottom: `1px solid ${COLOR_BORDER_SUBTLE}`,
        }}
      >
        New Context Injected In This Turn
      </div>

      <div className="space-y-3">
        {newUserMessageInjections.length > 0 && (
          <PopoverSection
            title="User Messages"
            count={newUserMessageInjections.length}
            tokenCount={userMessageTokens}
          >
            {newUserMessageInjections.map((injection) => (
              <div key={injection.id} className="min-w-0">
                <div className="flex items-center justify-between text-xs">
                  <span style={{ color: COLOR_TEXT_SECONDARY }}>
                    Turn {injection.turnIndex + 1}
                  </span>
                  <span style={{ color: COLOR_TEXT_MUTED }}>
                    ~{formatTokens(injection.estimatedTokens)} tokens
                  </span>
                </div>
                {injection.textPreview && (
                  <div
                    className="mt-0.5 truncate text-xs italic"
                    style={{ color: COLOR_TEXT_MUTED, opacity: 0.8 }}
                  >
                    {injection.textPreview}
                  </div>
                )}
              </div>
            ))}
          </PopoverSection>
        )}

        {newClaudeMdInjections.length > 0 && (
          <PopoverSection
            title="CLAUDE.md Files"
            count={newClaudeMdInjections.length}
            tokenCount={claudeMdTokens}
          >
            {newClaudeMdInjections.map((injection) => {
              const displayPath =
                shortenDisplayPath(injection.path, projectRoot) || injection.displayName;
              const absolutePath = resolveAbsolutePath(injection.path, projectRoot);
              return (
                <div key={injection.id} className="min-w-0">
                  <CopyablePath
                    displayText={displayPath}
                    copyText={absolutePath}
                    className="text-xs"
                    style={{ color: COLOR_TEXT_SECONDARY }}
                  />
                  <div className="text-xs" style={{ color: COLOR_TEXT_MUTED }}>
                    ~{formatTokens(injection.estimatedTokens)} tokens
                  </div>
                </div>
              );
            })}
          </PopoverSection>
        )}

        {newMentionedFileInjections.length > 0 && (
          <PopoverSection
            title="Mentioned Files"
            count={newMentionedFileInjections.length}
            tokenCount={mentionedFileTokens}
          >
            {newMentionedFileInjections.map((injection) => {
              const displayPath = shortenDisplayPath(injection.path, projectRoot);
              const absolutePath = resolveAbsolutePath(injection.path, projectRoot);
              return (
                <div key={injection.id} className="min-w-0">
                  <CopyablePath
                    displayText={displayPath}
                    copyText={absolutePath}
                    className="text-xs"
                    style={{ color: COLOR_TEXT_SECONDARY }}
                  />
                  <div className="text-xs" style={{ color: COLOR_TEXT_MUTED }}>
                    ~{formatTokens(injection.estimatedTokens)} tokens
                  </div>
                </div>
              );
            })}
          </PopoverSection>
        )}

        {newToolOutputInjections.length > 0 && (
          <PopoverSection
            title="Tool Outputs"
            count={newToolOutputInjections.length}
            tokenCount={toolOutputTokens}
          >
            {newToolOutputInjections.map((injection) =>
              injection.toolBreakdown.map((tool, idx) => (
                <div
                  key={`${injection.id}-${tool.toolName}-${idx}`}
                  className="flex items-center justify-between text-xs"
                >
                  <span style={{ color: COLOR_TEXT_SECONDARY }}>{tool.toolName}</span>
                  <span style={{ color: COLOR_TEXT_MUTED }}>
                    ~{formatTokens(tool.tokenCount)} tokens
                  </span>
                </div>
              ))
            )}
          </PopoverSection>
        )}

        {newTaskCoordinationInjections.length > 0 && (
          <PopoverSection
            title="Task Coordination"
            count={newTaskCoordinationInjections.length}
            tokenCount={taskCoordinationTokens}
          >
            {newTaskCoordinationInjections.map((injection) =>
              injection.breakdown.map((item, idx) => (
                <div
                  key={`${injection.id}-${item.label}-${idx}`}
                  className="flex items-center justify-between text-xs"
                >
                  <span style={{ color: COLOR_TEXT_SECONDARY }}>{item.label}</span>
                  <span style={{ color: COLOR_TEXT_MUTED }}>
                    ~{formatTokens(item.tokenCount)} tokens
                  </span>
                </div>
              ))
            )}
          </PopoverSection>
        )}

        {newThinkingTextInjections.length > 0 && (
          <PopoverSection
            title="Thinking + Text"
            count={newThinkingTextInjections.length}
            tokenCount={thinkingTextTokens}
          >
            {newThinkingTextInjections.map((injection) => (
              <div key={injection.id} className="min-w-0">
                <div className="text-xs" style={{ color: COLOR_TEXT_SECONDARY }}>
                  Turn {injection.turnIndex + 1}
                </div>
                <div className="space-y-0.5 pl-2">
                  {injection.breakdown.map((item, idx) => (
                    <div
                      key={`${item.type}-${idx}`}
                      className="flex items-center justify-between text-xs"
                    >
                      <span style={{ color: COLOR_TEXT_MUTED }}>
                        {item.type === 'thinking' ? 'Thinking' : 'Text'}
                      </span>
                      <span style={{ color: COLOR_TEXT_MUTED }}>
                        ~{formatTokens(item.tokenCount)} tokens
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </PopoverSection>
        )}
      </div>

      <div
        className="mt-2 flex items-center justify-between pt-2 text-xs"
        style={{ borderTop: `1px solid ${COLOR_BORDER_SUBTLE}` }}
      >
        <span style={{ color: COLOR_TEXT_MUTED }}>Total new tokens</span>
        <span style={{ color: COLOR_TEXT_SECONDARY }}>
          ~{formatTokens(totalNewTokens)} tokens
        </span>
      </div>
    </div>
  );
});
