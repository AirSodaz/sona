/**
 * @file transcriptCoordinator.ts
 *
 * Store boundary architecture:
 * - `transcriptStore.ts`: Canonical multi-session Zustand store and state mutations.
 * - `transcriptSessionStore.ts`: Active-session selector projection facade for standard UI components.
 * - `transcriptCoordinator.ts`: Action forwarding bridge for callers requiring flat imports.
 *   - For active-session state or actions, prefer `useTranscriptSessionStore` or `sessionActions`.
 *   - For direct multi-session coordination outside React, prefer `useTranscriptStore.getState()`.
 */

import type { TranscriptUpdate } from '../types/transcript';
import {
  clearActiveTranscriptSession,
  clearSegments,
  clearTranscriptSegments,
  finalizeLastSegment as finalizeLastTranscriptSegment,
  loadTranscriptSession,
  mergeSegments as mergeTranscriptSegments,
  openSession as openTranscriptSession,
  sessionActions,
  setSegments as setTranscriptSegments,
  splitTranscriptSegment,
  syncSavedRecordingMeta,
  updateSegment as updateTranscriptSegment,
  upsertTranscriptSegmentAndSetActive,
} from './transcriptSessionStore';
import { useTranscriptStore } from './transcriptStore';

export {
  clearActiveTranscriptSession,
  clearSegments,
  clearTranscriptSegments,
  finalizeLastTranscriptSegment,
  loadTranscriptSession,
  mergeTranscriptSegments,
  openTranscriptSession,
  setTranscriptSegments,
  splitTranscriptSegment,
  syncSavedRecordingMeta,
  updateTranscriptSegment,
  upsertTranscriptSegmentAndSetActive,
};

export const deleteTranscriptSegment = sessionActions.deleteSegment;
export const applyTranscriptUpdate = sessionActions.applyTranscriptUpdate;

export const applyTranscriptUpdateToSession = (
  sessionId: string,
  update: TranscriptUpdate,
  activeSegmentId?: string | null
): void => {
  useTranscriptStore.getState().applyTranscriptUpdateToSession(sessionId, update, activeSegmentId);
};

export const setRecordingSessionId = (id: string | null): void => {
  useTranscriptStore.getState().setRecordingSessionId(id);
};

let activeRecorderStopHandler: (() => Promise<unknown>) | null = null;

export const registerActiveRecorderStopHandler = (
  handler: (() => Promise<unknown>) | null
): void => {
  activeRecorderStopHandler = handler;
};

export const stopActiveRecording = async (): Promise<boolean> => {
  const handler = activeRecorderStopHandler;
  if (handler) {
    activeRecorderStopHandler = null;
    try {
      await handler();
      return true;
    } catch (error) {
      console.error('Failed to cleanly stop active recording from coordinator:', error);
    }
  }
  return false;
};
