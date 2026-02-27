/**
 * DefaultToolViewer
 *
 * Default rendering for tools that don't have specialized viewers.
 * Includes background task output streaming for Bash run_in_background commands.
 */

import React from 'react';

import { type ItemStatus, StatusDot } from '../BaseItem';

import { BackgroundTaskOutput, extractBackgroundTaskId } from './BackgroundTaskOutput';
import { renderInput, renderOutput } from './renderHelpers';
import { ToolProgressDisplay } from './ToolProgressDisplay';

import type { LinkedToolItem } from '@renderer/types/groups';

interface DefaultToolViewerProps {
  linkedTool: LinkedToolItem;
  status: ItemStatus;
}

export const DefaultToolViewer: React.FC<DefaultToolViewerProps> = ({ linkedTool, status }) => {
  const backgroundTaskId = extractBackgroundTaskId(linkedTool);

  return (
    <>
      {/* Input Section */}
      <div>
        <div className="mb-1 text-xs" style={{ color: 'var(--tool-item-muted)' }}>
          Input
        </div>
        <div
          className="max-h-96 overflow-auto rounded p-3 font-mono text-xs"
          style={{
            backgroundColor: 'var(--code-bg)',
            border: '1px solid var(--code-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          {renderInput(linkedTool.name, linkedTool.input)}
        </div>
      </div>

      {/* Output Section */}
      {!linkedTool.isOrphaned && linkedTool.result && (
        <div>
          <div
            className="mb-1 flex items-center gap-2 text-xs"
            style={{ color: 'var(--tool-item-muted)' }}
          >
            Output
            <StatusDot status={status} />
          </div>
          <div
            className="max-h-96 overflow-auto rounded p-3 font-mono text-xs"
            style={{
              backgroundColor: 'var(--code-bg)',
              border: '1px solid var(--code-border)',
              color:
                status === 'error'
                  ? 'var(--tool-result-error-text)'
                  : 'var(--color-text-secondary)',
            }}
          >
            {renderOutput(linkedTool.result.content)}
          </div>
        </div>
      )}

      {/* Background task streaming output */}
      {backgroundTaskId && <BackgroundTaskOutput taskId={backgroundTaskId} active={true} />}

      {/* In-progress output */}
      {linkedTool.isOrphaned && linkedTool.progress && (
        <div>
          <div
            className="mb-1 flex items-center gap-2 text-xs"
            style={{ color: 'var(--tool-item-muted)' }}
          >
            Output (in progress)
            <StatusDot status="pending" />
          </div>
          <ToolProgressDisplay progress={linkedTool.progress} />
        </div>
      )}
    </>
  );
};
