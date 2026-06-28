import { useTranscriptStore, type TranscriptStore } from './transcriptStore';

export { useTranscriptStore } from './transcriptStore';

export const openTranscriptSession = (...args: Parameters<TranscriptStore['openSession']>) => {
  return useTranscriptStore.getState().openSession(...args);
};

export const loadTranscriptSession = (...args: Parameters<TranscriptStore['loadTranscriptSession']>) => {
  return useTranscriptStore.getState().loadTranscriptSession(...args);
};

export const clearActiveTranscriptSession = (...args: Parameters<TranscriptStore['clearActiveTranscriptSession']>) => {
  return useTranscriptStore.getState().clearActiveTranscriptSession(...args);
};

export const clearTranscriptSegments = (...args: Parameters<TranscriptStore['clearTranscriptSegments']>) => {
  return useTranscriptStore.getState().clearTranscriptSegments(...args);
};

export const syncSavedRecordingMeta = (...args: Parameters<TranscriptStore['syncSavedRecordingMeta']>) => {
  return useTranscriptStore.getState().syncSavedRecordingMeta(...args);
};

export const setTranscriptSegments = (...args: Parameters<TranscriptStore['setSegments']>) => {
  return useTranscriptStore.getState().setSegments(...args);
};

export const updateTranscriptSegment = (...args: Parameters<TranscriptStore['updateSegment']>) => {
  return useTranscriptStore.getState().updateSegment(...args);
};

export const deleteTranscriptSegment = (...args: Parameters<TranscriptStore['deleteSegment']>) => {
  return useTranscriptStore.getState().deleteSegment(...args);
};

export const mergeTranscriptSegments = (...args: Parameters<TranscriptStore['mergeSegments']>) => {
  return useTranscriptStore.getState().mergeSegments(...args);
};

export const splitTranscriptSegment = (...args: Parameters<TranscriptStore['splitTranscriptSegment']>) => {
  return useTranscriptStore.getState().splitTranscriptSegment(...args);
};

export const finalizeLastTranscriptSegment = (...args: Parameters<TranscriptStore['finalizeLastSegment']>) => {
  return useTranscriptStore.getState().finalizeLastSegment(...args);
};

export const applyTranscriptUpdate = (...args: Parameters<TranscriptStore['applyTranscriptUpdate']>) => {
  return useTranscriptStore.getState().applyTranscriptUpdate(...args);
};

export const upsertTranscriptSegmentAndSetActive = (...args: Parameters<TranscriptStore['upsertTranscriptSegmentAndSetActive']>) => {
  return useTranscriptStore.getState().upsertTranscriptSegmentAndSetActive(...args);
};

export const applyTranscriptUpdateToSession = (
  ...args: Parameters<TranscriptStore['applyTranscriptUpdateToSession']>
) => {
  return useTranscriptStore.getState().applyTranscriptUpdateToSession(...args);
};

export const setRecordingSessionId = (
  ...args: Parameters<TranscriptStore['setRecordingSessionId']>
) => {
  return useTranscriptStore.getState().setRecordingSessionId(...args);
};
