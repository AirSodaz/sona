import type { TranscriptSegment } from '../types/transcript';

export interface AutoSaveLifecycleHooks {
  cancelPending: (historyIds: string[]) => void;
  rebaseline: (historyId: string, segments: TranscriptSegment[]) => void;
}

let autoSaveLifecycleHooks: AutoSaveLifecycleHooks | null = null;

export function registerAutoSaveLifecycleHooks(hooks: AutoSaveLifecycleHooks): void {
  autoSaveLifecycleHooks = hooks;
}

export function getAutoSaveLifecycleHooks(): AutoSaveLifecycleHooks | null {
  return autoSaveLifecycleHooks;
}
