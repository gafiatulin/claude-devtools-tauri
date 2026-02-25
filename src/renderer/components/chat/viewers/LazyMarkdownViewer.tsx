import React, { useCallback, useEffect, useRef, useState } from 'react';

import { api } from '@renderer/api';
import { CopyButton } from '@renderer/components/common/CopyButton';
import {
  CODE_BG,
  CODE_BORDER,
  CODE_HEADER_BG,
  COLOR_TEXT_MUTED,
  COLOR_TEXT_SECONDARY,
} from '@renderer/constants/cssVariables';
import { parseMarkdownInWorker } from '@renderer/workers/markdownWorkerClient';
import { FileText } from 'lucide-react';

import type { MarkdownViewerProps } from './types';

/**
 * Renders markdown content with off-thread parsing via `marked` in a Web Worker.
 *
 * Shows plain preformatted text instantly, then swaps in formatted HTML once the
 * worker returns. Links are intercepted via event delegation and opened externally.
 */
export function LazyMarkdownViewer({
  content,
  maxHeight = 'max-h-96',
  className = '',
  label,
  copyable = false,
}: MarkdownViewerProps): React.JSX.Element {
  const [html, setHtml] = useState<string | null>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // Parse markdown in worker whenever content changes
  useEffect(() => {
    let cancelled = false;
    setHtml(null);

    parseMarkdownInWorker(content).then(
      (result) => {
        if (!cancelled) setHtml(result);
      },
      (err) => {
        console.error('[LazyMarkdownViewer] Worker parse failed:', err);
        // Keep showing plain <pre> text on error
      }
    );

    return () => {
      cancelled = true;
    };
  }, [content]);

  // Intercept <a> clicks in rendered HTML → open externally
  const handleClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    const anchor = target.closest('a');
    if (anchor) {
      e.preventDefault();
      const href = anchor.getAttribute('href');
      if (href) {
        void api.openExternal(href);
      }
    }
  }, []);

  return (
    <div
      className={`overflow-hidden rounded-lg shadow-sm ${copyable && !label ? 'group relative' : ''} ${className}`}
      style={{
        backgroundColor: CODE_BG,
        border: `1px solid ${CODE_BORDER}`,
      }}
    >
      {/* Copy button overlay (when no label header) */}
      {copyable && !label && <CopyButton text={content} />}

      {/* Optional header — matches CodeBlockViewer style */}
      {label && (
        <div
          className="flex items-center gap-2 px-3 py-2"
          style={{
            backgroundColor: CODE_HEADER_BG,
            borderBottom: `1px solid ${CODE_BORDER}`,
          }}
        >
          <FileText className="size-4 shrink-0" style={{ color: COLOR_TEXT_MUTED }} />
          <span className="text-sm font-medium" style={{ color: COLOR_TEXT_SECONDARY }}>
            {label}
          </span>
          {copyable && (
            <>
              <span className="flex-1" />
              <CopyButton text={content} inline />
            </>
          )}
        </div>
      )}

      {/* Content area */}
      <div className={`overflow-auto ${maxHeight}`}>
        {html !== null ? (
          <div
            ref={wrapperRef}
            className="md-rendered p-4"
            onClick={handleClick}
            dangerouslySetInnerHTML={{ __html: html }}
          />
        ) : (
          <pre
            className="whitespace-pre-wrap break-words p-4 text-sm leading-relaxed"
            style={{ color: 'var(--prose-body)' }}
          >
            {content}
          </pre>
        )}
      </div>
    </div>
  );
}
