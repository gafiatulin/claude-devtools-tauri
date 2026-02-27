/**
 * BackgroundTaskOutput
 *
 * Streams live output from a background task's output file on disk.
 * Polls the Rust backend every 2 seconds to read the file content.
 * Shows a blinking yellow indicator while the task is still running.
 * Used by both Bash (run_in_background) and TaskOutput viewers.
 */

import React, { useEffect, useRef, useState } from 'react';

import { api } from '@renderer/api';

interface BackgroundTaskOutputProps {
  taskId: string;
  /** Whether to actively poll (set false to stop polling) */
  active: boolean;
  /** Called with running state whenever it changes */
  onRunningChange?: (running: boolean) => void;
  /** Hide the status header (when parent provides its own status) */
  hideHeader?: boolean;
}

export const BackgroundTaskOutput: React.FC<BackgroundTaskOutputProps> = ({ taskId, active, onRunningChange, hideHeader }) => {
  const [output, setOutput] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(true);
  const outputRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    if (!active || !taskId) return;

    let cancelled = false;

    const poll = async (): Promise<void> => {
      try {
        const result = await api.readBackgroundTaskOutput(taskId);
        if (!cancelled && result) {
          setOutput(result.content);
          setIsRunning(result.isRunning);
        }
      } catch {
        // File may not exist yet
      }
    };

    poll();
    const interval = setInterval(poll, 2000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [active, taskId]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  // Notify parent when running state changes
  useEffect(() => {
    onRunningChange?.(isRunning);
  }, [isRunning, onRunningChange]);

  if (!output) return null;

  const lineCount = output.split('\n').length;

  return (
    <div>
      {!hideHeader && (
        <div
          className="mb-1 flex items-center gap-2 text-xs"
          style={{ color: 'var(--tool-item-muted)' }}
        >
          <span
            className={`inline-block size-2 rounded-full ${isRunning ? 'animate-pulse' : ''}`}
            style={{ backgroundColor: isRunning ? '#eab308' : '#22c55e' }}
          />
          {isRunning ? 'Background task running' : 'Background task completed'}
          <span>{lineCount} lines</span>
        </div>
      )}
      <pre
        ref={outputRef}
        className="max-h-80 overflow-auto whitespace-pre-wrap break-all rounded p-3 font-mono text-xs"
        style={{
          backgroundColor: 'var(--code-bg)',
          border: `1px solid ${isRunning ? '#eab308' : 'var(--code-border)'}`,
          color: 'var(--color-text-secondary)',
        }}
      >
        {output}
      </pre>
    </div>
  );
};

/** Extract background task ID from Bash run_in_background result text */
export function extractBackgroundTaskId(
  linkedTool: { name: string; input: Record<string, unknown>; result?: { content: string | unknown[] } }
): string | null {
  if (linkedTool.name !== 'Bash') return null;
  if (!linkedTool.input.run_in_background) return null;

  const content = linkedTool.result?.content;
  if (typeof content !== 'string') return null;

  const match = content.match(/Command running in background with ID:\s*(\S+)/);
  return match ? match[1].replace(/\.$/, '') : null;
}
