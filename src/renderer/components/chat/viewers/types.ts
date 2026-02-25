export interface MarkdownViewerProps {
  content: string;
  maxHeight?: string; // e.g., "max-h-64" or "max-h-96"
  className?: string;
  label?: string; // Optional label like "Thinking", "Output", etc.
  /** When true, shows a copy button (overlay when no label, inline in header when label exists) */
  copyable?: boolean;
}
