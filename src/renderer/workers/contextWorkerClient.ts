/**
 * contextWorkerClient — Promise-based interface to the context computation Web Worker.
 *
 * The worker is created lazily on first use and reused across sessions.
 * Falls back to synchronous main-thread computation if the worker fails to initialize.
 */

import type { ClaudeMdFileInfo } from '@renderer/types/data';
import type { MentionedFileInfo, ContextStats, ContextPhaseInfo } from '@renderer/types/contextInjection';
import type { ChatItem } from '@renderer/types/groups';
import type { ContextWorkerRequest, ContextWorkerResponse } from './contextWorker';

type PendingResolver = {
  resolve: (result: { statsMap: Map<string, ContextStats>; phaseInfo: ContextPhaseInfo }) => void;
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
    worker = new Worker(new URL('./contextWorker.ts', import.meta.url), { type: 'module' });

    worker.onmessage = (event: MessageEvent<ContextWorkerResponse>) => {
      const { id, statsMap, phaseInfo, error } = event.data;
      const entry = pending.get(id);
      if (!entry) return;
      pending.delete(id);

      if (error !== undefined) {
        console.error('[contextWorker] Computation error:', error);
        entry.reject(new Error(error));
      } else {
        // postMessage structured-clones Maps, so these arrive as proper Map objects.
        entry.resolve({
          statsMap: statsMap!,
          phaseInfo: phaseInfo!,
        });
      }
    };

    worker.onerror = (event) => {
      console.error('[contextWorker] Worker error:', event.message, event);
      for (const entry of pending.values()) {
        entry.reject(new Error('Context worker crashed'));
      }
      pending.clear();
      worker = null;
      workerFailed = true;
    };

    worker.onmessageerror = (event) => {
      console.error('[contextWorker] Message deserialization error:', event);
    };

    return worker;
  } catch (err) {
    console.error('[contextWorker] Failed to create worker:', err);
    workerFailed = true;
    return null;
  }
}

/**
 * Runs processSessionContextWithPhases in a Web Worker.
 * Falls back to the main-thread implementation if the worker is unavailable.
 */
export async function computeContextInWorker(
  items: ChatItem[],
  projectRoot: string,
  claudeMdTokenData: Record<string, ClaudeMdFileInfo> | undefined,
  mentionedFileTokenData: Map<string, MentionedFileInfo> | undefined,
  directoryTokenData: Record<string, ClaudeMdFileInfo> | undefined
): Promise<{ statsMap: Map<string, ContextStats>; phaseInfo: ContextPhaseInfo }> {
  const w = getWorker();

  if (!w) {
    // Fallback: run synchronously on the main thread
    const { processSessionContextWithPhases } = await import('@renderer/utils/contextTracker');
    return processSessionContextWithPhases(
      items,
      projectRoot,
      claudeMdTokenData,
      mentionedFileTokenData,
      directoryTokenData
    );
  }

  return new Promise<{ statsMap: Map<string, ContextStats>; phaseInfo: ContextPhaseInfo }>(
    (resolve, reject) => {
      const id = nextRequestId++;
      pending.set(id, { resolve, reject });

      const message: ContextWorkerRequest = {
        id,
        items,
        projectRoot,
        claudeMdTokenData,
        mentionedFileTokenData,
        directoryTokenData,
      };
      w.postMessage(message);
    }
  );
}
