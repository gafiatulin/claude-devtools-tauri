/**
 * Render Helpers
 *
 * Shared rendering functions for tool input and output.
 */

import React from 'react';

import {
  COLOR_TEXT,
  COLOR_TEXT_MUTED,
  DIFF_ADDED_TEXT,
  DIFF_REMOVED_TEXT,
} from '@renderer/constants/cssVariables';
import { parseMarkdownInWorker } from '@renderer/workers/markdownWorkerClient';

/**
 * Tiny component that renders markdown via the worker, falling back to <pre>.
 */
function WorkerMarkdown({ content }: { content: string }): React.JSX.Element {
  const [html, setHtml] = React.useState<string | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    parseMarkdownInWorker(content).then(
      (result) => { if (!cancelled) setHtml(result); },
      () => { /* keep plain text on error */ }
    );
    return () => { cancelled = true; };
  }, [content]);

  if (html !== null) {
    return <div className="md-rendered" dangerouslySetInnerHTML={{ __html: html }} />;
  }
  return (
    <pre className="whitespace-pre-wrap break-words" style={{ color: COLOR_TEXT }}>
      {content}
    </pre>
  );
}

/**
 * Renders the input section based on tool type with theme-aware styling.
 */
export function renderInput(toolName: string, input: Record<string, unknown>): React.ReactElement {
  // Special rendering for Edit tool - show diff-like format
  if (toolName === 'Edit') {
    const filePath = input.file_path as string | undefined;
    const oldString = input.old_string as string | undefined;
    const newString = input.new_string as string | undefined;
    const replaceAll = input.replace_all as boolean | undefined;

    return (
      <div className="space-y-2">
        {filePath && (
          <div className="mb-2 text-xs" style={{ color: COLOR_TEXT_MUTED }}>
            {filePath}
            {replaceAll && (
              <span className="ml-2" style={{ color: COLOR_TEXT_MUTED }}>
                (replace all)
              </span>
            )}
          </div>
        )}
        {oldString && (
          <div className="whitespace-pre-wrap break-all" style={{ color: DIFF_REMOVED_TEXT }}>
            {oldString.split('\n').map((line, i) => (
              <div key={i}>- {line}</div>
            ))}
          </div>
        )}
        {newString && (
          <div className="whitespace-pre-wrap break-all" style={{ color: DIFF_ADDED_TEXT }}>
            {newString.split('\n').map((line, i) => (
              <div key={i}>+ {line}</div>
            ))}
          </div>
        )}
      </div>
    );
  }

  // Special rendering for Bash tool
  if (toolName === 'Bash') {
    const command = input.command as string | undefined;
    const description = input.description as string | undefined;

    return (
      <div className="space-y-2">
        {description && (
          <div className="mb-1 text-xs" style={{ color: COLOR_TEXT_MUTED }}>
            {description}
          </div>
        )}
        {command && (
          <code className="whitespace-pre-wrap break-all" style={{ color: COLOR_TEXT }}>
            {command}
          </code>
        )}
      </div>
    );
  }

  // Special rendering for ExitPlanMode — render string fields as readable text
  if (toolName === 'ExitPlanMode') {
    const allowedPrompts = input.allowedPrompts as
      | Array<{ tool?: string; prompt?: string }>
      | undefined;

    // Collect any extra string fields (e.g. plan content the model may include)
    const extraFields = Object.entries(input).filter(
      ([key, val]) => key !== 'allowedPrompts' && typeof val === 'string'
    ) as Array<[string, string]>;

    return (
      <div className="space-y-2">
        {extraFields.map(([key, val]) => (
          <div key={key}>
            <div className="mb-1 text-xs" style={{ color: COLOR_TEXT_MUTED }}>
              {key}
            </div>
            <WorkerMarkdown content={val} />
          </div>
        ))}
        {allowedPrompts && allowedPrompts.length > 0 && (
          <div>
            <div className="mb-1 text-xs" style={{ color: COLOR_TEXT_MUTED }}>
              Allowed prompts
            </div>
            <ul className="list-disc space-y-0.5 pl-4" style={{ color: COLOR_TEXT }}>
              {allowedPrompts.map((p, i) => (
                <li key={i}>
                  {p.tool && (
                    <span style={{ color: COLOR_TEXT_MUTED }}>{p.tool}: </span>
                  )}
                  {p.prompt}
                </li>
              ))}
            </ul>
          </div>
        )}
        {extraFields.length === 0 && (!allowedPrompts || allowedPrompts.length === 0) && (
          <div className="italic" style={{ color: COLOR_TEXT_MUTED }}>
            No parameters
          </div>
        )}
      </div>
    );
  }

  // Special rendering for Read tool
  if (toolName === 'Read') {
    const filePath = input.file_path as string | undefined;
    const offset = input.offset as number | undefined;
    const limit = input.limit as number | undefined;

    return (
      <div style={{ color: COLOR_TEXT }}>
        <div>{filePath}</div>
        {(offset !== undefined || limit !== undefined) && (
          <div className="mt-1 text-xs" style={{ color: COLOR_TEXT_MUTED }}>
            {offset !== undefined && `offset: ${offset}`}
            {offset !== undefined && limit !== undefined && ', '}
            {limit !== undefined && `limit: ${limit}`}
          </div>
        )}
      </div>
    );
  }

  // Special rendering for TaskOutput tool
  if (toolName === 'TaskOutput') {
    const taskId = input.task_id as string | undefined;
    const block = input.block as boolean | undefined;
    const timeout = input.timeout as number | undefined;

    return (
      <div style={{ color: COLOR_TEXT }}>
        <div className="font-mono">{taskId ?? 'unknown'}</div>
        {(block !== undefined || timeout !== undefined) && (
          <div className="mt-1 text-xs" style={{ color: COLOR_TEXT_MUTED }}>
            {block !== undefined && `block: ${String(block)}`}
            {block !== undefined && timeout !== undefined && ', '}
            {timeout !== undefined &&
              `timeout: ${timeout < 1000 ? `${timeout}ms` : timeout < 60000 ? `${(timeout / 1000).toFixed(1)}s` : `${(timeout / 60000).toFixed(1)}m`}`}
          </div>
        )}
      </div>
    );
  }

  // Default: JSON format
  return (
    <pre className="whitespace-pre-wrap break-all" style={{ color: COLOR_TEXT }}>
      {JSON.stringify(input, null, 2)}
    </pre>
  );
}

/**
 * Renders the output section with theme-aware styling.
 */
export function renderOutput(content: string | unknown[]): React.ReactElement {
  const displayText = typeof content === 'string' ? content : JSON.stringify(content, null, 2);
  return (
    <pre className="whitespace-pre-wrap break-all" style={{ color: COLOR_TEXT }}>
      {displayText}
    </pre>
  );
}
