import { useEffect } from 'react';
import type { TranscriptSegment } from '../types/transcript';
import { transcriptAutoSaveRuntime } from '../services/transcriptAutoSaveRuntime';

export async function flushPendingAutoSave(
  historyId?: string | null,
  segments?: TranscriptSegment[] | null,
): Promise<void> {
  await transcriptAutoSaveRuntime.flushPending(historyId, segments);
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
