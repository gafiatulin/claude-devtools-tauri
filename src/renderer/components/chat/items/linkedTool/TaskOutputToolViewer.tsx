/**
 * TaskOutputToolViewer
 *
 * Specialized viewer for TaskOutput tool calls.
 * Shows task_id prominently, metadata (block/timeout), and terminal-like output.
 * For completed results, extracts clean output from toolUseResult.task.output.
 * For in-progress results, streams output from the background task's output file.
 */

import React, { useEffect, useRef } from 'react';

import { Terminal } from 'lucide-react';

import { type ItemStatus, StatusDot } from '../BaseItem';

import { BackgroundTaskOutput } from './BackgroundTaskOutput';
import { ToolProgressDisplay } from './ToolProgressDisplay';

import type { LinkedToolItem } from '@renderer/types/groups';

interface TaskOutputToolViewerProps {
  linkedTool: LinkedToolItem;
  status: ItemStatus;
}

function formatTimeout(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
}

/** Extract clean output from toolUseResult structured data */
function extractTaskOutput(linkedTool: LinkedToolItem): {
  output: string;
  taskStatus?: string;
  retrievalStatus?: string;
  exitCode?: number | null;
  description?: string;
} | null {
  const tur = linkedTool.result?.toolUseResult as Record<string, unknown> | undefined;
  if (!tur) return null;

  const task = tur.task as Record<string, unknown> | undefined;
  if (!task) return null;

  const output = task.output as string | undefined;
  if (output === undefined) return null;

  return {
    output,
    taskStatus: task.status as string | undefined,
    retrievalStatus: tur.retrieval_status as string | undefined,
    exitCode: task.exitCode as number | null | undefined,
    description: task.description as string | undefined,
  };
}

/** Fall back to extracting output from raw XML content */
function extractOutputFromXml(content: string): string {
  const match = content.match(/<output>([\s\S]*?)<\/output>/);
  return match ? match[1].trim() : content;
}

export const TaskOutputToolViewer: React.FC<TaskOutputToolViewerProps> = ({
  linkedTool,
  status,
}) => {
  const outputRef = useRef<HTMLPreElement>(null);
  const taskId = linkedTool.input.task_id as string | undefined;
  const block = linkedTool.input.block as boolean | undefined;
  const timeout = linkedTool.input.timeout as number | undefined;

  // Extract structured output (prefer toolUseResult, fall back to raw content)
  const taskData = extractTaskOutput(linkedTool);

  let completedOutput = '';
  if (taskData) {
    completedOutput = taskData.output;
  } else if (linkedTool.result?.content) {
    const resultContent = linkedTool.result.content;
    completedOutput =
      typeof resultContent === 'string'
        ? extractOutputFromXml(resultContent)
        : Array.isArray(resultContent)
          ? resultContent
              .map((item: unknown) => (typeof item === 'string' ? item : JSON.stringify(item)))
              .join('\n')
          : JSON.stringify(resultContent, null, 2);
  }

  const lineCount = completedOutput ? completedOutput.split('\n').length : 0;

  // Auto-scroll output to bottom
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [completedOutput]);

  // Status label for task result
  const isTimeout = taskData?.retrievalStatus === 'timeout';
  const isError =
    status === 'error' || (taskData?.exitCode != null && taskData.exitCode !== 0);

  return (
    <>
      {/* Input Section */}
      <div>
        <div className="mb-1 text-xs" style={{ color: 'var(--tool-item-muted)' }}>
          Input
        </div>
        <div
          className="rounded p-3 text-xs"
          style={{
            backgroundColor: 'var(--code-bg)',
            border: '1px solid var(--code-border)',
          }}
        >
          <div className="flex items-center gap-2">
            <Terminal className="size-3.5" style={{ color: 'var(--tool-item-muted)' }} />
            <code className="font-mono" style={{ color: 'var(--color-text-secondary)' }}>
              {taskId ?? 'unknown'}
            </code>
          </div>
          {(block !== undefined || timeout !== undefined) && (
            <div className="mt-1.5 flex gap-3" style={{ color: 'var(--tool-item-muted)' }}>
              {block !== undefined && <span>block: {String(block)}</span>}
              {timeout !== undefined && <span>timeout: {formatTimeout(timeout)}</span>}
            </div>
          )}
        </div>
      </div>

      {/* Output Section (completed) */}
      {!linkedTool.isOrphaned && linkedTool.result && (
        <div>
          <div
            className="mb-1 flex items-center gap-2 text-xs"
            style={{ color: 'var(--tool-item-muted)' }}
          >
            Output
            <StatusDot status={status} />
            {isTimeout && <span style={{ color: 'var(--tool-result-error-text)' }}>timeout</span>}
            {taskData?.exitCode != null && taskData.exitCode !== 0 && (
              <span style={{ color: 'var(--tool-result-error-text)' }}>
                exit {taskData.exitCode}
              </span>
            )}
            {lineCount > 0 && <span>{lineCount} lines</span>}
          </div>
          <pre
            ref={outputRef}
            className="max-h-80 overflow-auto whitespace-pre-wrap break-all rounded p-3 font-mono text-xs"
            style={{
              backgroundColor: 'var(--code-bg)',
              border: '1px solid var(--code-border)',
              color: isError
                ? 'var(--tool-result-error-text)'
                : 'var(--color-text-secondary)',
            }}
          >
            {completedOutput}
          </pre>
        </div>
      )}

      {/* In-progress: show waiting spinner + stream log output below */}
      {linkedTool.isOrphaned && (
        <div>
          {linkedTool.progress && (
            <ToolProgressDisplay progress={linkedTool.progress} />
          )}
          {taskId && (
            <BackgroundTaskOutput taskId={taskId} active={true} hideHeader={true} />
          )}
        </div>
      )}
    </>
  );
};
