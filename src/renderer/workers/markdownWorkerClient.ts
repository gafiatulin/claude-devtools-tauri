/**
 * markdownWorkerClient — Promise-based interface to the markdown parser Web Worker.
 *
 * The worker is created lazily on first use and reused across components.
 * Falls back to main-thread `marked.parse()` if the worker fails to initialize.
 */

import type {
  MarkdownWorkerRequest,
  MarkdownWorkerResponse,
} from './markdownWorker';

type PendingResolver = {
  resolve: (html: string) => void;
  reject: (err: Error) => void;
};

let worker: Worker | null = null;
let workerFailed = false;
let nextRequestId = 0;
const pending = new Map<number, PendingResolver>();

function getWorker(): Worker | null {
  if (workerFailed) return null;
  if (worker) return worker;

  try {
    worker = new Worker(
      new URL('./markdownWorker.ts', import.meta.url),
      { type: 'module' }
    );

    worker.onmessage = (event: MessageEvent<MarkdownWorkerResponse>) => {
      const { id, html, error } = event.data;
      const entry = pending.get(id);
      if (!entry) return;
      pending.delete(id);

      if (error !== undefined) {
        console.error('[markdownWorker] Parse error:', error);
        entry.reject(new Error(error));
      } else {
        entry.resolve(html!);
      }
    };

    worker.onerror = (event) => {
      console.error('[markdownWorker] Worker error:', event.message, event);
      for (const entry of pending.values()) {
        entry.reject(new Error('Markdown worker crashed'));
      }
      pending.clear();
      worker = null;
      workerFailed = true;
    };

    worker.onmessageerror = (event) => {
      console.error('[markdownWorker] Message deserialization error:', event);
    };

    return worker;
  } catch (err) {
    console.error('[markdownWorker] Failed to create worker:', err);
    workerFailed = true;
    return null;
  }
}

/**
 * Parses markdown to HTML in a Web Worker.
 * Falls back to the main-thread `marked.parse()` if the worker is unavailable.
 */
export async function parseMarkdownInWorker(markdown: string): Promise<string> {
  const w = getWorker();

  if (!w) {
    // Fallback: run synchronously on the main thread
    const { marked } = await import('marked');
    return marked.parse(markdown, { async: false }) as string;
  }

  return new Promise<string>((resolve, reject) => {
    const id = nextRequestId++;
    pending.set(id, { resolve, reject });

    const message: MarkdownWorkerRequest = { id, markdown };
    w.postMessage(message);
  });
}
