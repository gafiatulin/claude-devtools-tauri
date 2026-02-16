/**
 * BulkActionBar - Renders the multi-select action bar for sidebar sessions.
 * Shows pin, hide, unhide, and cancel buttons when sessions are selected.
 */

import React from 'react';

import { Eye, EyeOff, Pin, X } from 'lucide-react';

interface BulkActionBarProps {
  selectedCount: number;
  showHiddenSessions: boolean;
  someSelectedAreHidden: boolean;
  onBulkPin: () => void;
  onBulkHide: () => void;
  onBulkUnhide: () => void;
  onClearSelection: () => void;
}

export const BulkActionBar = React.memo(function BulkActionBar({
  selectedCount,
  showHiddenSessions,
  someSelectedAreHidden,
  onBulkPin,
  onBulkHide,
  onBulkUnhide,
  onClearSelection,
}: BulkActionBarProps): React.JSX.Element {
  return (
    <div
      className="flex items-center gap-1.5 border-b px-3 py-1.5"
      style={{
        borderColor: 'var(--color-border)',
        backgroundColor: 'var(--color-surface-raised)',
      }}
    >
      <span
        className="text-[11px] font-medium"
        style={{ color: 'var(--color-text-secondary)' }}
      >
        {selectedCount} selected
      </span>
      <div className="ml-auto flex items-center gap-1">
        <button
          onClick={onBulkPin}
          className="rounded px-1.5 py-0.5 text-[10px] font-medium transition-colors hover:bg-white/5"
          style={{ color: 'var(--color-text-secondary)' }}
          title="Pin selected sessions"
        >
          <Pin className="inline-block size-3" /> Pin
        </button>
        <button
          onClick={onBulkHide}
          className="rounded px-1.5 py-0.5 text-[10px] font-medium transition-colors hover:bg-white/5"
          style={{ color: 'var(--color-text-secondary)' }}
          title="Hide selected sessions"
        >
          <EyeOff className="inline-block size-3" /> Hide
        </button>
        {showHiddenSessions && someSelectedAreHidden && (
          <button
            onClick={onBulkUnhide}
            className="rounded px-1.5 py-0.5 text-[10px] font-medium transition-colors hover:bg-white/5"
            style={{ color: 'var(--color-text-secondary)' }}
            title="Unhide selected sessions"
          >
            <Eye className="inline-block size-3" /> Unhide
          </button>
        )}
        <button
          onClick={onClearSelection}
          className="rounded p-0.5 transition-colors hover:bg-white/5"
          style={{ color: 'var(--color-text-muted)' }}
          title="Cancel selection"
        >
          <X className="size-3.5" />
        </button>
      </div>
    </div>
  );
});
