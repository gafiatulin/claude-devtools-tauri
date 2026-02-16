import React, { lazy, Suspense } from 'react';

import type { MarkdownViewerProps } from './MarkdownViewer';

const MarkdownViewerLazy = lazy(() => import('./MarkdownViewer'));

function MarkdownSkeleton() {
  return (
    <div className="space-y-2 py-1">
      <div className="h-3 bg-[var(--color-surface-muted)] rounded animate-pulse w-full" />
      <div className="h-3 bg-[var(--color-surface-muted)] rounded animate-pulse w-4/5" />
      <div className="h-3 bg-[var(--color-surface-muted)] rounded animate-pulse w-3/5" />
    </div>
  );
}

export function LazyMarkdownViewer(props: MarkdownViewerProps): React.JSX.Element {
  return (
    <Suspense fallback={<MarkdownSkeleton />}>
      <MarkdownViewerLazy {...props} />
    </Suspense>
  );
}
