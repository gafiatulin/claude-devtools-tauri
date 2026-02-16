/**
 * aiGroupEnhancer.worker — Web Worker that runs enhanceAIGroup off the main thread.
 *
 * Vite bundles this as a separate ES module chunk via:
 *   new Worker(new URL('./aiGroupEnhancer.worker.ts', import.meta.url), { type: 'module' })
 *
 * Structured clone (used by postMessage) handles Map, Date, and nested objects natively,
 * so no manual serialization is needed for the input/output types.
 */

import { enhanceAIGroup } from '@renderer/utils/aiGroupEnhancer';

import type { ClaudeMdStats } from '@renderer/types/claudeMd';
import type { AIGroup, EnhancedAIGroup } from '@renderer/types/groups';
import type { PrecedingSlashInfo } from '@renderer/utils/aiGroupEnhancer';

export interface AIGroupEnhancerRequest {
  id: number;
  aiGroup: AIGroup;
  claudeMdStats?: ClaudeMdStats;
  precedingSlash?: PrecedingSlashInfo;
}

export interface AIGroupEnhancerResponse {
  id: number;
  enhanced?: EnhancedAIGroup;
  error?: string;
}

interface WorkerScope {
  onmessage: ((event: MessageEvent<AIGroupEnhancerRequest>) => void) | null;
  postMessage: (message: AIGroupEnhancerResponse) => void;
}
const workerSelf = self as unknown as WorkerScope;

workerSelf.onmessage = (event: MessageEvent<AIGroupEnhancerRequest>) => {
  const { id, aiGroup, claudeMdStats, precedingSlash } = event.data;
  try {
    const enhanced = enhanceAIGroup(aiGroup, claudeMdStats, precedingSlash);
    workerSelf.postMessage({ id, enhanced } satisfies AIGroupEnhancerResponse);
  } catch (err) {
    workerSelf.postMessage({
      id,
      error: err instanceof Error ? err.message : String(err),
    } satisfies AIGroupEnhancerResponse);
  }
};
