import type { TranscriptSegment, TranscriptUpdate, TranscriptTimingUnit } from '../types/transcript';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { useTranscriptPlaybackStore } from './transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from './transcriptRuntimeStore';
import { useTranscriptSessionStore } from './transcriptSessionStore';
import { useTranscriptSidecarStore } from './transcriptSidecarStore';
import { v4 as uuidv4 } from 'uuid';
import { editorHtmlToTranscriptText, splitTranscriptText } from '../components/transcript/richText';
import { stripHtmlTags } from '../utils/segmentUtils';

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
  const previousHistoryId = useTranscriptSessionStore.getState().sourceHistoryId;
  if (sourceHistoryId && sourceHistoryId !== previousHistoryId) {
    useTranscriptSidecarStore.getState().clearAutoSaveState(sourceHistoryId);
  }

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
  audioUrl?: string | null,
): void {
  useTranscriptSessionStore.getState().setSourceHistoryId(historyId);
  useTranscriptSidecarStore.getState().rekeyCurrentSummaryState(historyId);
  useTranscriptSessionStore.getState().setTitle(title);
  useTranscriptSessionStore.getState().setIcon(icon || null);
  if (audioUrl !== undefined) {
    useTranscriptPlaybackStore.getState().setAudioUrl(audioUrl);
  }
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

export function splitTranscriptSegment(
  id: string,
  caretOffset: number,
  currentHtml: string
): string | null {
  const sessionStore = useTranscriptSessionStore.getState();
  const segment = sessionStore.segments.find((s) => s.id === id);
  if (!segment) return null;

  const fullText = editorHtmlToTranscriptText(currentHtml);
  const [leftText, rightText] = splitTranscriptText(fullText, caretOffset);

  const plainText = stripHtmlTags(fullText);
  const totalLength = plainText.length;

  const leftUnits: TranscriptTimingUnit[] = [];
  const rightUnits: TranscriptTimingUnit[] = [];
  let splitTime = segment.start;
  let splitTimeFound = false;

  if (segment.timing && segment.timing.units && segment.timing.units.length > 0) {
    const units = segment.timing.units;
    let cumulativeLen = 0;
    for (const unit of units) {
      cumulativeLen += stripHtmlTags(unit.text).length;
      if (cumulativeLen <= caretOffset) {
        leftUnits.push(unit);
      } else {
        rightUnits.push(unit);
      }
    }
    if (rightUnits.length > 0) {
      splitTime = rightUnits[0].start;
      splitTimeFound = true;
    } else if (leftUnits.length > 0) {
      splitTime = leftUnits[leftUnits.length - 1].end;
      splitTimeFound = true;
    }
  }

  const leftTokens: string[] = [];
  const rightTokens: string[] = [];
  const leftTimestamps: number[] = [];
  const rightTimestamps: number[] = [];
  const leftDurations: number[] = [];
  const rightDurations: number[] = [];

  const hasLegacyTimestamps = Boolean(segment.tokens && segment.timestamps && segment.tokens.length > 0 && segment.tokens.length === segment.timestamps.length);

  if (hasLegacyTimestamps && segment.tokens && segment.timestamps) {
    let cumulativeLen = 0;
    for (let i = 0; i < segment.tokens.length; i++) {
      const token = segment.tokens[i];
      cumulativeLen += stripHtmlTags(token).length;
      if (cumulativeLen <= caretOffset) {
        leftTokens.push(token);
        leftTimestamps.push(segment.timestamps[i]);
        if (segment.durations) leftDurations.push(segment.durations[i]);
      } else {
        rightTokens.push(token);
        rightTimestamps.push(segment.timestamps[i]);
        if (segment.durations) rightDurations.push(segment.durations[i]);
      }
    }
    if (!splitTimeFound) {
      if (rightTimestamps.length > 0) {
        splitTime = rightTimestamps[0];
        splitTimeFound = true;
      } else if (leftTimestamps.length > 0 && segment.durations && leftDurations.length > 0) {
        splitTime = leftTimestamps[leftTimestamps.length - 1] + leftDurations[leftDurations.length - 1];
        splitTimeFound = true;
      }
    }
  }

  if (!splitTimeFound) {
    const ratio = totalLength > 0 ? Math.min(1, Math.max(0, caretOffset / totalLength)) : 0.5;
    const duration = segment.end - segment.start;
    splitTime = Math.round((segment.start + ratio * duration) * 100) / 100;
  }

  // Bound splitTime safety
  splitTime = Math.min(segment.end, Math.max(segment.start, splitTime));

  const newSegmentId = uuidv4();

  const segmentLeft: TranscriptSegment = {
    ...segment,
    end: splitTime,
    text: leftText,
    timing: segment.timing ? {
      ...segment.timing,
      units: leftUnits,
    } : undefined,
    tokens: segment.tokens ? leftTokens : undefined,
    timestamps: segment.timestamps ? leftTimestamps : undefined,
    durations: segment.durations ? leftDurations : undefined,
  };

  const segmentRight: TranscriptSegment = {
    id: newSegmentId,
    start: splitTime,
    end: segment.end,
    text: rightText,
    isFinal: true,
    speaker: segment.speaker,
    speakerAttribution: segment.speakerAttribution,
    timing: segment.timing ? {
      ...segment.timing,
      units: rightUnits,
    } : undefined,
    tokens: segment.tokens ? rightTokens : undefined,
    timestamps: segment.timestamps ? rightTimestamps : undefined,
    durations: segment.durations ? rightDurations : undefined,
  };

  // Replace segment with segmentLeft, and insert segmentRight after it
  const nextSegments = [...sessionStore.segments];
  const index = nextSegments.findIndex((s) => s.id === id);
  if (index !== -1) {
    nextSegments[index] = segmentLeft;
    nextSegments.splice(index + 1, 0, segmentRight);
  } else {
    // Fallback if not found in list (should not happen)
    nextSegments.push(segmentLeft, segmentRight);
  }

  sessionStore.setSegments(nextSegments);
  sessionStore.setEditingSegmentId(newSegmentId);

  return newSegmentId;
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
