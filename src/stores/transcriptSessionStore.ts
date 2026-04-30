import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import type { TranscriptSegment } from '../types/transcript';
import { areSpeakerTagsEqual } from '../types/speaker';
import {
  normalizeTranscriptSegment,
  normalizeTranscriptSegments,
} from '../utils/transcriptTiming';
import {
  INITIAL_TRANSCRIPT_SESSION_STATE,
  type TranscriptSessionStateFields,
} from './transcriptSessionState';

export interface TranscriptSessionState extends TranscriptSessionStateFields {
  setSourceHistoryId: (id: string | null) => void;
  setTitle: (title: string | null) => void;
  setIcon: (icon: string | null) => void;
  addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;
  upsertSegment: (segment: TranscriptSegment) => void;
  updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;
  deleteSegment: (id: string) => void;
  mergeSegments: (id1: string, id2: string) => void;
  setSegments: (segments: TranscriptSegment[]) => void;
  loadTranscript: (segments: TranscriptSegment[], sourceHistoryId: string | null, title?: string | null, icon?: string | null) => void;
  openSession: (session: {
    segments: TranscriptSegment[];
    sourceHistoryId: string | null;
    title?: string | null;
    icon?: string | null;
  }) => void;
  clearSegments: (options?: { title?: string | null }) => void;
  finalizeLastSegment: () => void;
  setEditingSegmentId: (id: string | null) => void;
  addAligningSegmentId: (id: string) => void;
  removeAligningSegmentId: (id: string) => void;
}

export const useTranscriptSessionStore = create<TranscriptSessionState>((set, get) => ({
  ...INITIAL_TRANSCRIPT_SESSION_STATE,

  setSourceHistoryId: (sourceHistoryId) => set({ sourceHistoryId }),

  setTitle: (title) => set({ title }),

  setIcon: (icon) => set({ icon }),

  addSegment: (segment) => {
    const id = uuidv4();
    const newSegment: TranscriptSegment = normalizeTranscriptSegment({ ...segment, id });
    set((state) => ({
      segments: [...state.segments, newSegment].sort((a, b) => a.start - b.start),
    }));
    return id;
  },

  upsertSegment: (segment) => {
    set((state) => {
      const result = calculateSegmentUpdate(state.segments, normalizeTranscriptSegment(segment));
      return { segments: result.segments };
    });
  },

  updateSegment: (id, updates) => {
    set((state) => ({
      segments: state.segments.map((segment) => (
        segment.id === id ? normalizeTranscriptSegment({ ...segment, ...updates }) : segment
      )),
    }));
  },

  deleteSegment: (id) => {
    set((state) => ({
      segments: state.segments.filter((segment) => segment.id !== id),
      editingSegmentId: state.editingSegmentId === id ? null : state.editingSegmentId,
    }));
  },

  mergeSegments: (id1, id2) => {
    const state = get();
    const seg1 = state.segments.find((segment) => segment.id === id1);
    const seg2 = state.segments.find((segment) => segment.id === id2);

    if (!seg1 || !seg2) {
      return;
    }

    const [first, second] = seg1.start <= seg2.start ? [seg1, seg2] : [seg2, seg1];
    if (!areSpeakerTagsEqual(first.speaker, second.speaker)) {
      return;
    }

    const mergedSegment: TranscriptSegment = {
      id: first.id,
      start: first.start,
      end: second.end,
      text: `${first.text} ${second.text}`.trim(),
      isFinal: first.isFinal && second.isFinal,
      speaker: first.speaker,
    };

    set((currentState) => ({
      segments: currentState.segments
        .filter((segment) => segment.id !== second.id)
        .map((segment) => (
          segment.id === first.id ? normalizeTranscriptSegment(mergedSegment) : segment
        )),
    }));
  },

  setSegments: (segments) => {
    set({
      segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
    });
  },

  loadTranscript: (segments, sourceHistoryId, title, icon) => {
    get().openSession({ segments, sourceHistoryId, title, icon });
  },

  openSession: ({ segments, sourceHistoryId, title, icon }) => {
    set({
      segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
      sourceHistoryId,
      title: title || '',
      icon: icon || null,
      editingSegmentId: null,
      aligningSegmentIds: new Set<string>(),
    });
  },

  clearSegments: (options) => {
    set({
      ...INITIAL_TRANSCRIPT_SESSION_STATE,
      title: options?.title ?? null,
    });
  },

  finalizeLastSegment: () => {
    set((state) => {
      if (state.segments.length === 0) {
        return state;
      }

      const lastIndex = state.segments.length - 1;
      const lastSegment = state.segments[lastIndex];
      if (lastSegment.isFinal) {
        return state;
      }

      const segments = [...state.segments];
      segments[lastIndex] = { ...lastSegment, isFinal: true };
      return { segments };
    });
  },

  setEditingSegmentId: (editingSegmentId) => set({ editingSegmentId }),

  addAligningSegmentId: (id) => set((state) => {
    const next = new Set(state.aligningSegmentIds);
    next.add(id);
    return { aligningSegmentIds: next };
  }),

  removeAligningSegmentId: (id) => set((state) => {
    const next = new Set(state.aligningSegmentIds);
    next.delete(id);
    return { aligningSegmentIds: next };
  }),
}));

function calculateSegmentUpdate(
  segments: TranscriptSegment[],
  segment: TranscriptSegment,
): { segments: TranscriptSegment[]; index: number } {
  const length = segments.length;

  if (length > 0) {
    const lastIndex = length - 1;
    if (segments[lastIndex].id === segment.id) {
      const nextSegments = [...segments];
      nextSegments[lastIndex] = segment;
      return { segments: nextSegments, index: lastIndex };
    }
  }

  const existingIndex = segments.findIndex((candidate) => candidate.id === segment.id);
  if (existingIndex !== -1) {
    const nextSegments = [...segments];
    nextSegments[existingIndex] = segment;
    return { segments: nextSegments, index: existingIndex };
  }

  if (length === 0 || segments[length - 1].start <= segment.start) {
    return {
      segments: [...segments, segment],
      index: length,
    };
  }

  const nextSegments = [...segments, segment].sort((a, b) => a.start - b.start);
  return {
    segments: nextSegments,
    index: nextSegments.findIndex((candidate) => candidate.id === segment.id),
  };
}
