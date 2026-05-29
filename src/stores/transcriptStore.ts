import { create, type StateCreator } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import type { TranscriptSegment, TranscriptUpdate, AppMode, ProcessingStatus, TranscriptSummaryState, HistorySummaryPayload } from '../types/transcript';
import { areSpeakerTagsEqual } from '../types/speaker';
import { normalizeTranscriptSegment, normalizeTranscriptSegments, normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { findSegmentAndIndexForTime, stripHtmlTags, performSegmentSplit } from '../utils/segmentUtils';
import { editorHtmlToTranscriptText, splitTranscriptText } from '../components/transcript/richText';
import { createDefaultSummaryState, DEFAULT_LLM_STATE, type LlmState, type AutoSaveStatus, type AutoSaveState, rekeyCurrentSummaryState as rekeyCurrentSummaryStateEntry, resolveTranscriptHistoryKey } from './transcriptSidecarState';

export interface SessionData {
  // Session fields
  segments: TranscriptSegment[];
  sourceHistoryId: string | null;
  title: string;
  icon: string | null;
  editingSegmentId: string | null;
  aligningSegmentIds: Set<string>;

  // Playback fields
  audioFile: File | null;
  audioUrl: string | null;
  currentTime: number;
  isPlaying: boolean;
  activeSegmentId: string | null;
  activeSegmentIndex: number;
  seekRequest: { time: number; timestamp: number } | null;
  lastSeekTimestamp: number;
}

export const DEFAULT_SESSION_DATA: SessionData = {
  segments: [],
  sourceHistoryId: null,
  title: '',
  icon: null,
  editingSegmentId: null,
  aligningSegmentIds: new Set(),
  audioFile: null,
  audioUrl: null,
  currentTime: 0,
  isPlaying: false,
  activeSegmentId: null,
  activeSegmentIndex: -1,
  seekRequest: null,
  lastSeekTimestamp: 0,
};

export interface TranscriptStore {
  // --- Core State ---
  activeSessionId: string;
  sessions: Record<string, SessionData>;

  // --- Runtime (Global) ---
  mode: AppMode;
  processingStatus: ProcessingStatus;
  processingProgress: number;
  isRecording: boolean;
  isCaptionMode: boolean;
  isPaused: boolean;

  // --- Sidecar (Keyed by historyId or 'current') ---
  summaryStates: Record<string, TranscriptSummaryState>;
  llmStates: Record<string, LlmState>;
  autoSaveStates: Record<string, AutoSaveState>;

  // --- Actions ---
  // Coordinator / Session Pointers
  openSession: (args: { segments: TranscriptSegment[], sourceHistoryId: string | null, title?: string | null, icon?: string | null, audioUrl?: string | null }) => void;
  loadTranscriptSession: (segments: TranscriptSegment[], sourceHistoryId: string | null, title?: string | null, icon?: string | null) => void;
  clearActiveTranscriptSession: (options?: { clearAudio?: boolean, title?: string | null }) => void;
  clearTranscriptSegments: () => void;
  syncSavedRecordingMeta: (title: string, historyId: string, icon: string | null | undefined, audioUrl?: string | null) => void;

  // Runtime
  setMode: (mode: AppMode) => void;
  setProcessingStatus: (status: ProcessingStatus) => void;
  setProcessingProgress: (progress: number) => void;
  setIsRecording: (isRecording: boolean) => void;
  setIsCaptionMode: (isCaptionMode: boolean) => void;
  setIsPaused: (isPaused: boolean) => void;

  // Session Data Mutations (Applies to activeSessionId)
  setSourceHistoryId: (id: string | null) => void;
  setTitle: (title: string | null) => void;
  setIcon: (icon: string | null) => void;
  setSegments: (segments: TranscriptSegment[]) => void;
  addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;
  upsertSegment: (segment: TranscriptSegment) => void;
  updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;
  deleteSegment: (id: string) => void;
  mergeSegments: (id1: string, id2: string) => void;
  splitTranscriptSegment: (id: string, caretOffset: number, currentHtml: string) => string | null;
  finalizeLastSegment: () => void;
  applyTranscriptUpdate: (update: TranscriptUpdate, activeSegmentId?: string | null) => void;
  upsertTranscriptSegmentAndSetActive: (segment: TranscriptSegment) => void;
  setEditingSegmentId: (id: string | null) => void;
  addAligningSegmentId: (id: string) => void;
  removeAligningSegmentId: (id: string) => void;

  // Playback Mutations (Applies to activeSessionId)
  setAudioFile: (file: File | null) => void;
  setAudioUrl: (url: string | null) => void;
  setCurrentTime: (time: number) => void;
  setIsPlaying: (isPlaying: boolean) => void;
  setActiveSegmentId: (id: string | null, index?: number) => void;
  resetActiveSegmentIndex: () => void;
  requestSeek: (time: number) => void;

  // Sidecar Mutations
  getLlmState: (historyId?: string) => LlmState;
  updateLlmState: (updates: Partial<LlmState>, historyId?: string) => void;
  getSummaryState: (historyId?: string) => TranscriptSummaryState;
  setSummaryState: (summaryState: Partial<TranscriptSummaryState>, historyId?: string) => void;
  updateSummaryState: (updates: Partial<TranscriptSummaryState>, historyId?: string) => void;
  hydrateSummaryState: (payload: HistorySummaryPayload, historyId?: string) => void;
  clearSummaryState: (historyId?: string) => void;
  rekeyCurrentSummaryState: (nextHistoryId: string | null) => void;
  setAutoSaveState: (historyId: string, status: AutoSaveStatus) => void;
  clearAutoSaveState: (historyId?: string) => void;
}

// Helper to get active session
function getActiveSession(state: TranscriptStore): SessionData {
  return state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
}

type StoreSet = Parameters<StateCreator<TranscriptStore>>[0];
type StoreGet = Parameters<StateCreator<TranscriptStore>>[1];

// Helper to mutate active session
function updateActiveSession(set: StoreSet, updater: (session: SessionData) => Partial<SessionData>) {
  set((state: TranscriptStore) => {
    const session = state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
    return {
      sessions: {
        ...state.sessions,
        [state.activeSessionId]: { ...session, ...updater(session) }
      }
    };
  });
}

function resolveHistoryKey(historyId: string | undefined, get: StoreGet): string {
  const activeSession = getActiveSession(get());
  return resolveTranscriptHistoryKey(historyId, activeSession.sourceHistoryId);
}

function calculateSegmentUpdate(segments: TranscriptSegment[], segment: TranscriptSegment): { segments: TranscriptSegment[]; index: number } {
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
    return { segments: [...segments, segment], index: length };
  }
  const nextSegments = [...segments, segment].sort((a, b) => a.start - b.start);
  return {
    segments: nextSegments,
    index: nextSegments.findIndex((candidate) => candidate.id === segment.id),
  };
}

export const useTranscriptStore = create<TranscriptStore>((set, get) => ({
  activeSessionId: 'default',
  sessions: { 'default': { ...DEFAULT_SESSION_DATA } },

  mode: 'live',
  processingStatus: 'idle',
  processingProgress: 0,
  isRecording: false,
  isCaptionMode: false,
  isPaused: false,

  summaryStates: { 'current': createDefaultSummaryState() },
  llmStates: { 'current': { ...DEFAULT_LLM_STATE } },
  autoSaveStates: {},

  // --- Runtime ---
  setMode: (mode) => set({ mode }),
  setProcessingStatus: (processingStatus) => set({ processingStatus }),
  setProcessingProgress: (processingProgress) => set({ processingProgress }),
  setIsRecording: (isRecording) => set({ isRecording }),
  setIsCaptionMode: (isCaptionMode) => set({ isCaptionMode }),
  setIsPaused: (isPaused) => set({ isPaused }),

  // --- Coordinator / Pointers ---
  openSession: (args) => {
    if (typeof args === 'string') {
        get().setAudioUrl(args);
        return;
    }
    const { segments = [], sourceHistoryId, title, icon, audioUrl } = args;
    if (sourceHistoryId) {
      const autoSaveStates = { ...get().autoSaveStates };
      delete autoSaveStates[sourceHistoryId];
      delete autoSaveStates['current'];
      set({ autoSaveStates });
    }
    const sessionId = sourceHistoryId || uuidv4();
    set((state) => ({
      activeSessionId: sessionId,
      sessions: {
        ...state.sessions,
        [sessionId]: {
          ...DEFAULT_SESSION_DATA,
          segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
          sourceHistoryId,
          title: title || '',
          icon: icon || null,
          audioUrl: audioUrl !== undefined ? audioUrl : null,
        }
      }
    }));
  },
  loadTranscriptSession: (segments, sourceHistoryId, title, icon) => {
    get().openSession({ segments, sourceHistoryId, title, icon });
  },
  clearActiveTranscriptSession: (options) => {
    updateActiveSession(set, (session) => ({
      ...DEFAULT_SESSION_DATA,
      title: options?.title ?? session.title,
      audioFile: options?.clearAudio ? null : session.audioFile,
      audioUrl: options?.clearAudio ? null : session.audioUrl,
    }));
    get().clearSummaryState('current');
  },
  clearSegments: () => get().clearActiveTranscriptSession(),
  clearTranscriptSegments: () => get().clearActiveTranscriptSession(),
  syncSavedRecordingMeta: (title, historyId, icon, audioUrl) => {
    const state = get();
    const activeId = state.activeSessionId;
    const session = state.sessions[activeId];
    if (!session) return;

    // re-key the session in the dictionary
    const newSessions = { ...state.sessions };
    delete newSessions[activeId];
    newSessions[historyId] = {
      ...session,
      sourceHistoryId: historyId,
      title,
      icon: icon || null,
      audioUrl: audioUrl !== undefined ? audioUrl : session.audioUrl,
    };

    set({ activeSessionId: historyId, sessions: newSessions });
    get().rekeyCurrentSummaryState(historyId);
  },

  // --- Session Data Mutations ---
  setSourceHistoryId: (sourceHistoryId) => updateActiveSession(set, () => ({ sourceHistoryId })),
  setTitle: (title) => updateActiveSession(set, () => ({ title: title || '' })),
  setIcon: (icon) => updateActiveSession(set, () => ({ icon })),
  setSegments: (segments) => updateActiveSession(set, () => ({
    segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start)
  })),
  addSegment: (segment) => {
    const id = uuidv4();
    const newSegment = normalizeTranscriptSegment({ ...segment, id });
    updateActiveSession(set, (session) => ({
      segments: [...session.segments, newSegment].sort((a, b) => a.start - b.start)
    }));
    return id;
  },
  upsertSegment: (segment) => {
    updateActiveSession(set, (session) => ({
      segments: calculateSegmentUpdate(session.segments, normalizeTranscriptSegment(segment)).segments
    }));
  },
  updateSegment: (id, updates) => {
    updateActiveSession(set, (session) => ({
      segments: session.segments.map(s => s.id === id ? normalizeTranscriptSegment({ ...s, ...updates }) : s)
    }));
  },
  deleteSegment: (id) => updateActiveSession(set, (session) => ({
    segments: session.segments.filter(s => s.id !== id),
    editingSegmentId: session.editingSegmentId === id ? null : session.editingSegmentId,
    activeSegmentId: session.activeSegmentId === id ? null : session.activeSegmentId
  })),
  mergeSegments: (id1, id2) => {
    updateActiveSession(set, (session) => {
      const seg1 = session.segments.find(s => s.id === id1);
      const seg2 = session.segments.find(s => s.id === id2);
      if (!seg1 || !seg2) return {};
      const [first, second] = seg1.start <= seg2.start ? [seg1, seg2] : [seg2, seg1];
      if (!areSpeakerTagsEqual(first.speaker, second.speaker)) return {};
      const merged: TranscriptSegment = {
        id: first.id, start: first.start, end: second.end,
        text: `${first.text} ${second.text}`.trim(),
        isFinal: first.isFinal && second.isFinal,
        speaker: first.speaker, speakerAttribution: first.speakerAttribution,
      };
      return {
        segments: session.segments
          .filter(s => s.id !== second.id)
          .map(s => s.id === first.id ? normalizeTranscriptSegment(merged) : s)
      };
    });
  },
  splitTranscriptSegment: (id, caretOffset, currentHtml) => {
    const session = getActiveSession(get());
    const segment = session.segments.find((s) => s.id === id);
    if (!segment) return null;
    const fullText = editorHtmlToTranscriptText(currentHtml);
    const [leftText, rightText] = splitTranscriptText(fullText, caretOffset);
    const plainText = stripHtmlTags(fullText);
    const newSegmentId = uuidv4();
    const { segmentLeft, segmentRight } = performSegmentSplit(segment, caretOffset, plainText, leftText, rightText, newSegmentId);

    const nextSegments = [...session.segments];
    const index = nextSegments.findIndex((s) => s.id === id);
    if (index !== -1) {
      nextSegments[index] = segmentLeft;
      nextSegments.splice(index + 1, 0, segmentRight);
    } else {
      nextSegments.push(segmentLeft, segmentRight);
    }

    updateActiveSession(set, () => ({
      segments: nextSegments,
      editingSegmentId: newSegmentId
    }));
    return newSegmentId;
  },
  finalizeLastSegment: () => updateActiveSession(set, (session) => {
    if (session.segments.length === 0) return {};
    const lastIndex = session.segments.length - 1;
    const lastSegment = session.segments[lastIndex];
    if (lastSegment.isFinal) return {};
    const segments = [...session.segments];
    segments[lastIndex] = { ...lastSegment, isFinal: true };
    return { segments };
  }),
  applyTranscriptUpdate: (update, activeSegmentIdParam) => {
    updateActiveSession(set, (session) => {
      const normalizedUpdate = normalizeTranscriptUpdate(update);
      const removeIds = new Set(normalizedUpdate.removeIds);
      let nextSegments = removeIds.size > 0
        ? session.segments.filter(s => !removeIds.has(s.id))
        : [...session.segments];

      normalizedUpdate.upsertSegments.forEach(seg => {
        const existingIndex = nextSegments.findIndex(c => c.id === seg.id);
        if (existingIndex !== -1) nextSegments[existingIndex] = seg;
        else nextSegments = [...nextSegments, seg].sort((a, b) => a.start - b.start);
      });

      let nextActiveId = session.activeSegmentId;
      let nextActiveIndex = -1;
      if (activeSegmentIdParam !== undefined) {
        nextActiveId = activeSegmentIdParam;
        nextActiveIndex = nextActiveId ? nextSegments.findIndex(s => s.id === nextActiveId) : -1;
      } else if (nextActiveId && removeIds.has(nextActiveId)) {
        const activeIndex = nextSegments.findIndex(s => s.id === nextActiveId);
        if (activeIndex === -1) nextActiveId = null;
        else nextActiveIndex = activeIndex;
      }
      return { segments: nextSegments, activeSegmentId: nextActiveId, activeSegmentIndex: nextActiveIndex };
    });
  },
  upsertTranscriptSegmentAndSetActive: (segment) => updateActiveSession(set, (session) => {
    const nextSegmentsData = calculateSegmentUpdate(session.segments, normalizeTranscriptSegment(segment));
    let nextSegments = nextSegmentsData.segments;
    const isCaptionMode = get().isCaptionMode;
    if (isCaptionMode && nextSegments.length > 50) {
      nextSegments = nextSegments.slice(-50);
    }
    const activeIndex = nextSegments.findIndex(c => c.id === segment.id);
    return {
      segments: nextSegments,
      activeSegmentId: segment.id,
      activeSegmentIndex: activeIndex === -1 ? nextSegments.length - 1 : activeIndex
    };
  }),
  setEditingSegmentId: (id) => updateActiveSession(set, () => ({ editingSegmentId: id })),
  addAligningSegmentId: (id) => updateActiveSession(set, (session) => {
    const next = new Set(session.aligningSegmentIds);
    next.add(id);
    return { aligningSegmentIds: next };
  }),
  removeAligningSegmentId: (id) => updateActiveSession(set, (session) => {
    const next = new Set(session.aligningSegmentIds);
    next.delete(id);
    return { aligningSegmentIds: next };
  }),

  // --- Playback Mutations ---
  setAudioFile: (file) => updateActiveSession(set, (session) => {
    if (session.audioUrl) URL.revokeObjectURL(session.audioUrl);
    return {
      audioFile: file, audioUrl: file ? URL.createObjectURL(file) : null,
      isPlaying: false, currentTime: 0, activeSegmentId: null, activeSegmentIndex: -1,
      seekRequest: null, lastSeekTimestamp: 0
    };
  }),
  setAudioUrl: (audioUrl) => updateActiveSession(set, () => ({
    audioUrl, isPlaying: false, currentTime: 0, activeSegmentId: null,
    activeSegmentIndex: -1, seekRequest: null, lastSeekTimestamp: 0
  })),
  setCurrentTime: (time) => updateActiveSession(set, (session) => {
    const { segment, index } = findSegmentAndIndexForTime(session.segments, time, session.activeSegmentIndex);
    if (segment?.id !== session.activeSegmentId) {
      return { currentTime: time, activeSegmentId: segment?.id || null, activeSegmentIndex: index };
    }
    return { currentTime: time };
  }),
  setIsPlaying: (isPlaying) => updateActiveSession(set, () => ({ isPlaying })),
  setActiveSegmentId: (activeSegmentId, activeSegmentIndex = -1) => updateActiveSession(set, () => ({ activeSegmentId, activeSegmentIndex })),
  resetActiveSegmentIndex: () => updateActiveSession(set, () => ({ activeSegmentIndex: -1 })),
  requestSeek: (time) => {
    get().setCurrentTime(time);
    const timestamp = Date.now();
    updateActiveSession(set, () => ({ seekRequest: { time, timestamp }, lastSeekTimestamp: timestamp }));
  },

  // --- Sidecar Mutations ---
  getLlmState: (historyId) => {
    const id = resolveHistoryKey(historyId, get);
    return get().llmStates[id] || { ...DEFAULT_LLM_STATE };
  },
  updateLlmState: (updates, historyId) => set((state) => {
    const id = resolveHistoryKey(historyId, get);
    return {
      llmStates: { ...state.llmStates, [id]: { ...(state.llmStates[id] || { ...DEFAULT_LLM_STATE }), ...updates } }
    };
  }),
  getSummaryState: (historyId) => {
    const id = resolveHistoryKey(historyId, get);
    return get().summaryStates[id] || createDefaultSummaryState();
  },
  setSummaryState: (summaryState, historyId) => set((state) => {
    const id = resolveHistoryKey(historyId, get);
    return {
      summaryStates: { ...state.summaryStates, [id]: { ...createDefaultSummaryState(), ...summaryState, record: summaryState.record } }
    };
  }),
  updateSummaryState: (updates, historyId) => set((state) => {
    const id = resolveHistoryKey(historyId, get);
    return {
      summaryStates: { ...state.summaryStates, [id]: { ...(state.summaryStates[id] || createDefaultSummaryState()), ...updates } }
    };
  }),
  hydrateSummaryState: (payload: HistorySummaryPayload, historyId) => {
    // Basic hydrate implementation to satisfy the type.
    const activeTemplateId = payload.activeTemplateId || 'default';
    get().setSummaryState({ activeTemplateId, record: payload.record, streamingContent: undefined, isGenerating: false, generationProgress: 0 }, historyId);
  },
  clearSummaryState: (historyId) => set((state) => {
    const id = resolveHistoryKey(historyId, get);
    if (!state.summaryStates[id]) return state;
    const summaryStates = { ...state.summaryStates };
    delete summaryStates[id];
    return { summaryStates };
  }),
  rekeyCurrentSummaryState: (nextHistoryId) => set((state) => ({
    summaryStates: rekeyCurrentSummaryStateEntry(state.summaryStates, nextHistoryId)
  })),
  setAutoSaveState: (historyId, status) => set((state) => {
    if (!historyId || historyId === 'current') return state;
    return { autoSaveStates: { ...state.autoSaveStates, [historyId]: { status, updatedAt: Date.now() } } };
  }),
  clearAutoSaveState: (historyId) => set((state) => {
    const id = resolveHistoryKey(historyId, get);
    if (!id || id === 'current' || !state.autoSaveStates[id]) return state;
    const autoSaveStates = { ...state.autoSaveStates };
    delete autoSaveStates[id];
    return { autoSaveStates };
  })
}));
