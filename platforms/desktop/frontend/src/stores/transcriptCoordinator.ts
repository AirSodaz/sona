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

export { useTranscriptStore } from './transcriptStore';
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
