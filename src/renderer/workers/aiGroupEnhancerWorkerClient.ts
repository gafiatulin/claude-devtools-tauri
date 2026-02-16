/**
 * aiGroupEnhancerWorkerClient — Promise-based interface to the AI group enhancer Web Worker.
 *
 * The worker is created lazily on first use and reused across components.
 * Falls back to synchronous main-thread computation if the worker fails to initialize.
 */

import type { ClaudeMdStats } from '@renderer/types/claudeMd';
import type { AIGroup, EnhancedAIGroup } from '@renderer/types/groups';
import type { PrecedingSlashInfo } from '@renderer/utils/aiGroupEnhancer';
import type {
  AIGroupEnhancerRequest,
  AIGroupEnhancerResponse,
} from './aiGroupEnhancer.worker';

type PendingResolver = {
  resolve: (result: EnhancedAIGroup) => void;
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
      new URL('./aiGroupEnhancer.worker.ts', import.meta.url),
      { type: 'module' }
    );

    worker.onmessage = (event: MessageEvent<AIGroupEnhancerResponse>) => {
      const { id, enhanced, error } = event.data;
      const entry = pending.get(id);
      if (!entry) return;
      pending.delete(id);

      if (error !== undefined) {
        console.error('[aiGroupEnhancerWorker] Computation error:', error);
        entry.reject(new Error(error));
      } else {
        entry.resolve(enhanced!);
      }
    };

    worker.onerror = (event) => {
      console.error('[aiGroupEnhancerWorker] Worker error:', event.message, event);
      for (const entry of pending.values()) {
        entry.reject(new Error('AI group enhancer worker crashed'));
      }
      pending.clear();
      worker = null;
      workerFailed = true;
    };

    worker.onmessageerror = (event) => {
      console.error('[aiGroupEnhancerWorker] Message deserialization error:', event);
    };

    return worker;
  } catch (err) {
    console.error('[aiGroupEnhancerWorker] Failed to create worker:', err);
    workerFailed = true;
    return null;
  }
}

/**
 * Runs enhanceAIGroup in a Web Worker.
 * Falls back to the main-thread implementation if the worker is unavailable.
 */
export async function enhanceAIGroupInWorker(
  aiGroup: AIGroup,
  claudeMdStats?: ClaudeMdStats,
  precedingSlash?: PrecedingSlashInfo
): Promise<EnhancedAIGroup> {
  const w = getWorker();

  if (!w) {
    // Fallback: run synchronously on the main thread
    const { enhanceAIGroup } = await import('@renderer/utils/aiGroupEnhancer');
    return enhanceAIGroup(aiGroup, claudeMdStats, precedingSlash);
  }

  return new Promise<EnhancedAIGroup>((resolve, reject) => {
    const id = nextRequestId++;
    pending.set(id, { resolve, reject });

    const message: AIGroupEnhancerRequest = {
      id,
      aiGroup,
      claudeMdStats,
      precedingSlash,
    };
    w.postMessage(message);
  });
}
