import { useEffect } from 'react';
import { transcriptAutoSaveRuntime } from '../services/transcriptAutoSaveRuntime';
import type { TranscriptSegment } from '../types/transcript';

export async function flushPendingAutoSave(
  historyId?: string | null,
  segments?: TranscriptSegment[] | null
): Promise<void> {
  await transcriptAutoSaveRuntime.flushPending(historyId, segments);
}

export function hasPendingAutoSave(historyId: string): boolean {
  return transcriptAutoSaveRuntime.hasPendingWork(historyId);
}

export function resetAutoSaveBaselineIfTracked(
  historyId: string,
  segments: TranscriptSegment[]
): void {
  transcriptAutoSaveRuntime.resetBaselineIfTracked(historyId, segments);
}

export function rebaselineTranscriptAutoSave(
  historyId: string,
  segments: TranscriptSegment[]
): void {
  transcriptAutoSaveRuntime.rebaseline(historyId, segments);
}

/**
 * Initializes the transcript auto-save runtime for the app shell.
 */
export function useAutoSaveTranscript() {
  useEffect(() => {
    transcriptAutoSaveRuntime.start();

    return () => {
      transcriptAutoSaveRuntime.stop();
    };
  }, []);
}
