/**
 * contextWorker — Web Worker that runs processSessionContextWithPhases off the main thread.
 *
 * Vite bundles this as a separate ES module chunk via:
 *   new Worker(new URL('./contextWorker.ts', import.meta.url), { type: 'module' })
 *
 * Structured clone (used by postMessage) handles Map, Date, and nested objects natively,
 * so no manual serialization is needed for the input/output types.
 */

import { processSessionContextWithPhases } from '@renderer/utils/contextTracker';
import type { ClaudeMdFileInfo } from '@renderer/types/data';
import type { MentionedFileInfo } from '@renderer/types/contextInjection';
import type { ContextStats, ContextPhaseInfo } from '@renderer/types/contextInjection';
import type { ChatItem } from '@renderer/types/groups';

export interface ContextWorkerRequest {
  id: number;
  items: ChatItem[];
  projectRoot: string;
  claudeMdTokenData: Record<string, ClaudeMdFileInfo> | undefined;
  mentionedFileTokenData: Map<string, MentionedFileInfo> | undefined;
  directoryTokenData: Record<string, ClaudeMdFileInfo> | undefined;
}

export interface ContextWorkerResponse {
  id: number;
  statsMap?: Map<string, ContextStats>;
  phaseInfo?: ContextPhaseInfo;
  error?: string;
}

// Cast to avoid tsconfig lib conflict between dom and webworker.
// We only use the subset we need rather than adding "webworker" to tsconfig lib.
interface WorkerScope {
  onmessage: ((event: MessageEvent<ContextWorkerRequest>) => void) | null;
  postMessage: (message: ContextWorkerResponse) => void;
}
const workerSelf = self as unknown as WorkerScope;

workerSelf.onmessage = (event: MessageEvent<ContextWorkerRequest>) => {
  const { id, items, projectRoot, claudeMdTokenData, mentionedFileTokenData, directoryTokenData } =
    event.data;
  try {
    const { statsMap, phaseInfo } = processSessionContextWithPhases(
      items,
      projectRoot,
      claudeMdTokenData,
      mentionedFileTokenData,
      directoryTokenData
    );
    workerSelf.postMessage({ id, statsMap, phaseInfo } satisfies ContextWorkerResponse);
  } catch (err) {
    workerSelf.postMessage({
      id,
      error: err instanceof Error ? err.message : String(err),
    } satisfies ContextWorkerResponse);
  }
};
