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
  registerActiveRecorderStopHandler,
  setRecordingSessionId,
  stopActiveRecording,
  updateTranscriptSegmentForSession,
} from './transcriptStore';
