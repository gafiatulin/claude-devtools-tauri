/**
 * markdownWorker — Web Worker that parses markdown to HTML off the main thread.
 *
 * Uses `marked` (with GFM enabled by default) to convert markdown strings into
 * HTML. Vite bundles this as a separate ES module chunk via:
 *   new Worker(new URL('./markdownWorker.ts', import.meta.url), { type: 'module' })
 */

import { marked } from 'marked';

export interface MarkdownWorkerRequest {
  id: number;
  markdown: string;
}

export interface MarkdownWorkerResponse {
  id: number;
  html?: string;
  error?: string;
}

interface WorkerScope {
  onmessage: ((event: MessageEvent<MarkdownWorkerRequest>) => void) | null;
  postMessage: (message: MarkdownWorkerResponse) => void;
}
const workerSelf = self as unknown as WorkerScope;

workerSelf.onmessage = (event: MessageEvent<MarkdownWorkerRequest>) => {
  const { id, markdown } = event.data;
  try {
    const html = marked.parse(markdown, { async: false }) as string;
    workerSelf.postMessage({ id, html } satisfies MarkdownWorkerResponse);
  } catch (err) {
    workerSelf.postMessage({
      id,
      error: err instanceof Error ? err.message : String(err),
    } satisfies MarkdownWorkerResponse);
  }
};
