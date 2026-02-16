/**
 * WriteToolViewer
 *
 * Renders the Write tool result.
 */

import React from 'react';

import { CodeBlockViewer } from '@renderer/components/chat/viewers';

import type { LinkedToolItem } from '@renderer/types/groups';

interface WriteToolViewerProps {
  linkedTool: LinkedToolItem;
}

export const WriteToolViewer: React.FC<WriteToolViewerProps> = ({ linkedTool }) => {
  const toolUseResult = linkedTool.result?.toolUseResult as Record<string, unknown> | undefined;

  const filePath = (toolUseResult?.filePath as string) || (linkedTool.input.file_path as string);
  const content = (toolUseResult?.content as string) || (linkedTool.input.content as string) || '';
  const isCreate = toolUseResult?.type === 'create';

  return (
    <div className="space-y-2">
      <div className="mb-1 text-xs text-zinc-500">
        {isCreate ? 'Created file' : 'Wrote to file'}
      </div>
      <CodeBlockViewer fileName={filePath} content={content} startLine={1} />
    </div>
  );
};
