/**
 * @file transcriptCoordinator.ts
 *
 * Store boundary architecture:
 * - `transcriptStore.ts`: Canonical multi-session Zustand store and state mutations.
 * - `transcriptSessionStore.ts`: Active-session selector projection facade for standard UI components.
 * - `transcriptCoordinator.ts`: Stateless action forwarding facade for backward-compatible flat imports.
 *   - For active-session state or actions, prefer `useTranscriptSessionStore` or `sessionActions`.
 *   - For direct multi-session coordination outside React, prefer `useTranscriptStore.getState()`.
 */

import { logger } from '../utils/logger';

export {
  applyTranscriptUpdate,
  clearActiveTranscriptSession,
  clearSegments,
  clearTranscriptSegments,
  deleteTranscriptSegment,
  finalizeLastTranscriptSegment,
  loadTranscriptSession,
  mergeTranscriptSegments,
  openTranscriptSession,
  setTranscriptSegments,
  splitTranscriptSegment,
  syncSavedRecordingMeta,
  updateTranscriptSegment,
  upsertTranscriptSegmentAndSetActive,
} from './transcriptSessionStore';

export {
  applyTranscriptUpdateToSession,
  evictNonRecordingSessions,
  evictTranscriptSessionsByHistoryIds,
  finalizeLastSegmentForSession,
  patchSegmentTranslationsForHistory,
  patchSegmentTranslationsForSession,
  setRecordingSessionId,
  setSegmentsForHistory,
  setSegmentsForSession,
  updateSessionMetaByHistoryId,
  updateTranscriptSegmentForSession,
} from './transcriptStore';

type ActiveRecorderStopHandler = (options?: { discard?: boolean }) => Promise<unknown>;
let activeRecorderStopHandler: ActiveRecorderStopHandler | null = null;
let isStoppingRecording = false;

export const registerActiveRecorderStopHandler = (
  handler: ActiveRecorderStopHandler | null
): void => {
  activeRecorderStopHandler = handler;
};

export const stopActiveRecording = async (options?: { discard?: boolean }): Promise<boolean> => {
  const handler = activeRecorderStopHandler;
  if (isStoppingRecording) {
    if (options?.discard) {
      // The stop in flight re-reads the discard flag at its last safe point.
      if (!handler) {
        logger.warn(
          '[TranscriptCoordinator] Discard request ignored: no handler registered for stop in-flight'
        );
        return false;
      }
      try {
        await handler({ discard: true });
        return true;
      } catch (error) {
        logger.error('Failed to forward discard to in-flight recording stop:', error);
        return false;
      }
    }
    logger.warn(
      '[TranscriptCoordinator] Stop active recording invocation ignored: stop already in-flight'
    );
    return false;
  }

  if (!handler) {
    logger.warn(
      '[TranscriptCoordinator] Stop active recording invocation ignored: no handler registered'
    );
    return false;
  }

  isStoppingRecording = true;
  try {
    await handler(options);
    return true;
  } catch (error) {
    logger.error('Failed to cleanly stop active recording:', error);
    return false;
  } finally {
    activeRecorderStopHandler = null;
    isStoppingRecording = false;
  }
};
