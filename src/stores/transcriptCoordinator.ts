import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { useTranscriptPlaybackStore } from './transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from './transcriptRuntimeStore';
import { useTranscriptSessionStore } from './transcriptSessionStore';
import { useTranscriptSidecarStore } from './transcriptSidecarStore';

interface OpenTranscriptSessionArgs {
  segments: TranscriptSegment[];
  sourceHistoryId: string | null;
  title?: string | null;
  icon?: string | null;
  audioUrl?: string | null;
}

export function openTranscriptSession({
  segments,
  sourceHistoryId,
  title,
  icon,
  audioUrl,
}: OpenTranscriptSessionArgs): void {
  useTranscriptSessionStore.getState().openSession({
    segments,
    sourceHistoryId,
    title,
    icon,
  });
  useTranscriptPlaybackStore.getState().openSession(audioUrl);
}

export function loadTranscriptSession(
  segments: TranscriptSegment[],
  sourceHistoryId: string | null,
  title?: string | null,
  icon?: string | null,
): void {
  openTranscriptSession({
    segments,
    sourceHistoryId,
    title,
    icon,
  });
}

export function clearActiveTranscriptSession(options?: {
  clearAudio?: boolean;
  title?: string | null;
}): void {
  useTranscriptSessionStore.getState().clearSegments({ title: options?.title });
  useTranscriptPlaybackStore.getState().clearSession({ clearAudio: options?.clearAudio });
  useTranscriptSidecarStore.getState().clearSummaryState('current');
}

export function clearTranscriptSegments(): void {
  clearActiveTranscriptSession();
}

export function syncSavedRecordingMeta(
  title: string,
  historyId: string,
  icon: string | undefined | null,
): void {
  useTranscriptSessionStore.getState().setSourceHistoryId(historyId);
  useTranscriptSidecarStore.getState().rekeyCurrentSummaryState(historyId);
  useTranscriptSessionStore.getState().setTitle(title);
  useTranscriptSessionStore.getState().setIcon(icon || null);
}

export function setTranscriptSegments(segments: TranscriptSegment[]): void {
  useTranscriptSessionStore.getState().setSegments(segments);
  useTranscriptPlaybackStore.getState().resetActiveSegmentIndex();
}

export function updateTranscriptSegment(
  id: string,
  updates: Partial<Omit<TranscriptSegment, 'id'>>,
): void {
  useTranscriptSessionStore.getState().updateSegment(id, updates);
}

export function deleteTranscriptSegment(id: string): void {
  useTranscriptSessionStore.getState().deleteSegment(id);

  const playbackStore = useTranscriptPlaybackStore.getState();
  if (playbackStore.activeSegmentId === id) {
    playbackStore.setActiveSegmentId(null);
  }
}

export function mergeTranscriptSegments(id1: string, id2: string): void {
  useTranscriptSessionStore.getState().mergeSegments(id1, id2);
}

export function finalizeLastTranscriptSegment(): void {
  useTranscriptSessionStore.getState().finalizeLastSegment();
}

export function applyTranscriptUpdate(update: TranscriptUpdate, activeSegmentId?: string | null): void {
  const normalizedUpdate = normalizeTranscriptUpdate(update);
  const sessionStore = useTranscriptSessionStore.getState();
  const playbackStore = useTranscriptPlaybackStore.getState();
  const removeIds = new Set(normalizedUpdate.removeIds);
  let nextSegments = removeIds.size > 0
    ? sessionStore.segments.filter((segment) => !removeIds.has(segment.id))
    : [...sessionStore.segments];

  normalizedUpdate.upsertSegments.forEach((segment) => {
    const existingIndex = nextSegments.findIndex((candidate) => candidate.id === segment.id);
    if (existingIndex !== -1) {
      nextSegments = nextSegments.map((candidate, index) => (
        index === existingIndex ? segment : candidate
      ));
      return;
    }

    nextSegments = [...nextSegments, segment].sort((a, b) => a.start - b.start);
  });

  sessionStore.setSegments(nextSegments);

  let nextActiveSegmentId = playbackStore.activeSegmentId;
  let nextActiveSegmentIndex: number;

  if (activeSegmentId !== undefined) {
    nextActiveSegmentId = activeSegmentId;
    nextActiveSegmentIndex = activeSegmentId
      ? nextSegments.findIndex((segment) => segment.id === activeSegmentId)
      : -1;
  } else if (nextActiveSegmentId && removeIds.has(nextActiveSegmentId)) {
    const activeIndex = nextSegments.findIndex((segment) => segment.id === nextActiveSegmentId);
    if (activeIndex === -1) {
      nextActiveSegmentId = null;
      nextActiveSegmentIndex = -1;
    } else {
      nextActiveSegmentIndex = activeIndex;
    }
  } else {
    nextActiveSegmentIndex = -1;
  }

  playbackStore.setActiveSegmentId(nextActiveSegmentId, nextActiveSegmentIndex);
}

export function upsertTranscriptSegmentAndSetActive(segment: TranscriptSegment): void {
  const sessionStore = useTranscriptSessionStore.getState();
  sessionStore.upsertSegment(segment);

  let nextSegments = useTranscriptSessionStore.getState().segments;
  const MAX_CAPTION_SEGMENTS = 50;
  const isCaptionMode = useTranscriptRuntimeStore.getState().isCaptionMode;
  if (isCaptionMode && nextSegments.length > MAX_CAPTION_SEGMENTS) {
    nextSegments = nextSegments.slice(-MAX_CAPTION_SEGMENTS);
    sessionStore.setSegments(nextSegments);
  }

  const activeIndex = nextSegments.findIndex((candidate) => candidate.id === segment.id);
  useTranscriptPlaybackStore.getState().setActiveSegmentId(
    segment.id,
    activeIndex === -1 ? nextSegments.length - 1 : activeIndex,
  );
}
