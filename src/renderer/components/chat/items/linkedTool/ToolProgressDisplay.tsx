/**
 * ToolProgressDisplay
 *
 * Renders in-progress tool output from Claude Code progress entries.
 * Shows streaming stdout for Bash, hook status, MCP tool status, or task waiting info.
 */

import React from 'react';

import { Loader2 } from 'lucide-react';

import type { ToolProgress } from '@renderer/types/data';

interface ToolProgressDisplayProps {
  progress: ToolProgress;
}

export const ToolProgressDisplay: React.FC<ToolProgressDisplayProps> = ({ progress }) => {
  switch (progress.type) {
    case 'bash':
      return (
        <div className="space-y-1">
          <div
            className="flex items-center gap-2 text-xs"
            style={{ color: 'var(--tool-item-muted)' }}
          >
            <Loader2 className="size-3 animate-spin" />
            <span>
              Running ({progress.totalLines} lines, {formatElapsed(progress.elapsedTimeSeconds)})
            </span>
          </div>
          {progress.fullOutput && (
            <div
              className="max-h-48 overflow-auto rounded p-2 font-mono text-xs"
              style={{
                backgroundColor: 'var(--code-bg)',
                border: '1px solid var(--code-border)',
                color: 'var(--color-text-secondary)',
              }}
            >
              <pre className="whitespace-pre-wrap break-all">{progress.fullOutput}</pre>
            </div>
          )}
        </div>
      );

    case 'hook':
      return (
        <div
          className="flex items-center gap-2 text-xs"
          style={{ color: 'var(--tool-item-muted)' }}
        >
          <Loader2 className="size-3 animate-spin" />
          <span>
            Hook: {progress.hookName} ({progress.hookEvent})
          </span>
        </div>
      );

    case 'mcp':
      return (
        <div
          className="flex items-center gap-2 text-xs"
          style={{ color: 'var(--tool-item-muted)' }}
        >
          <Loader2 className="size-3 animate-spin" />
          <span>
            MCP: {progress.serverName} / {progress.toolName}
            {progress.elapsedTimeMs != null && (
              <> ({formatElapsed(progress.elapsedTimeMs / 1000)})</>
            )}
          </span>
        </div>
      );

    case 'waiting':
      return (
        <div
          className="flex items-center gap-2 text-xs"
          style={{ color: 'var(--tool-item-muted)' }}
        >
          <Loader2 className="size-3 animate-spin" />
          <span>Waiting: {progress.taskDescription}</span>
        </div>
      );

    default:
      return null;
  }
};

function formatElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs.toFixed(0)}s`;
}
