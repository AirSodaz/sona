/**
 * @file transcriptStore.ts
 * Canonical multi-session Zustand store and state mutations.
 *
 * Store boundary architecture:
 * - `transcriptStore.ts` owns the canonical multi-session state, active session routing, and mutations.
 * - Standard UI components should consume active session state via `useTranscriptSessionStore`.
 * - Direct mutation or multi-session access should use `useTranscriptStore.getState()`.
 */
import { v4 as uuidv4 } from 'uuid';
import { create, type StateCreator } from 'zustand';
import { areSpeakerTagsEqual } from '../types/speakerNormalization';
import type {
  AppMode,
  HistorySummaryPayload,
  IngestionFocus,
  ProcessingStatus,
  TranscriptSegment,
  TranscriptSummaryState,
  TranscriptUpdate,
} from '../types/transcript';
import { needsWordBoundarySpace } from '../utils/cjkTypography';
import {
  findSegmentAndIndexForTime,
  performSegmentSplit,
  stripHtmlTags,
} from '../utils/segmentUtils';
import {
  normalizeTranscriptSegment,
  normalizeTranscriptSegments,
  normalizeTranscriptUpdate,
} from '../utils/transcriptTiming';

import {
  type AutoSaveState,
  type AutoSaveStatus,
  createDefaultSummaryState,
  DEFAULT_LLM_STATE,
  type LlmState,
  rekeyCurrentSummaryState as rekeyCurrentSummaryStateEntry,
  resolveTranscriptHistoryKey,
} from './transcriptSidecarState';

let activeBatchItemIdResolver: (() => string | null) | null = null;

export function registerBatchActiveItemIdResolver(resolver: () => string | null): void {
  activeBatchItemIdResolver = resolver;
}

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
  createSessionId: string;
  recordingSessionId: string | null;
  sessions: Record<string, SessionData>;
  // --- Runtime (Global) ---
  mode: AppMode;
  ingestionFocus: IngestionFocus;
  processingStatus: ProcessingStatus;
  processingProgress: number;
  isRecording: boolean;
  isCaptionMode: boolean;
  isPaused: boolean;
  isAgentRecording: boolean;
  agentRecordingHistoryId: string | null;
  // --- Sidecar (Keyed by historyId or 'current') ---
  summaryStates: Record<string, TranscriptSummaryState>;
  llmStates: Record<string, LlmState>;
  autoSaveStates: Record<string, AutoSaveState>;

  // --- Actions ---
  // Coordinator / Session Pointers
  setRecordingSessionId: (id: string | null) => void;
  openSession: (
    args:
      | string
      | {
          segments: TranscriptSegment[];
          sourceHistoryId: string | null;
          title?: string | null;
          icon?: string | null;
          audioUrl?: string | null;
        }
  ) => void;
  loadTranscriptSession: (
    segments: TranscriptSegment[],
    sourceHistoryId: string | null,
    title?: string | null,
    icon?: string | null
  ) => void;
  activateSession: (sessionId: string) => void;
  evictSession: (sessionId: string) => void;
  evictSessionsByHistoryIds: (historyIds: string[]) => void;
  evictNonRecordingSessions: (keepSessionIds?: string[]) => void;
  updateSessionMetaByHistoryId: (
    historyId: string,
    meta: { title?: string; icon?: string | null }
  ) => void;
  clearActiveTranscriptSession: (options?: { clearAudio?: boolean; title?: string | null }) => void;
  clearTranscriptSegments: () => void;
  clearSegments: () => void;
  syncSavedRecordingMeta: (
    title: string,
    historyId: string,
    icon: string | null | undefined,
    audioUrl?: string | null
  ) => void;

  // Runtime
  setMode: (mode: AppMode, focus?: IngestionFocus) => void;
  setIngestionFocus: (focus: IngestionFocus) => void;
  setProcessingStatus: (status: ProcessingStatus) => void;
  setProcessingProgress: (progress: number) => void;
  setIsRecording: (isRecording: boolean) => void;
  setIsCaptionMode: (isCaptionMode: boolean) => void;
  setIsPaused: (isPaused: boolean) => void;
  setIsAgentRecording: (isAgentRecording: boolean, historyId?: string | null) => void;
  // Session Data Mutations (Applies to activeSessionId)
  setSourceHistoryId: (id: string | null) => void;
  setTitle: (title: string | null) => void;
  setIcon: (icon: string | null) => void;
  setSegments: (segments: TranscriptSegment[]) => void;
  setSegmentsForSession: (sessionId: string, segments: TranscriptSegment[]) => void;
  setSegmentsForHistory: (historyId: string, segments: TranscriptSegment[]) => void;
  addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;
  upsertSegment: (segment: TranscriptSegment) => void;
  updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;
  patchSegmentTranslation: (id: string, translation: string) => void;
  patchSegmentTranslations: (updates: Array<{ id: string; translation: string }>) => void;
  patchSegmentTranslationsForSession: (
    sessionId: string,
    updates: Array<{ id: string; translation: string }>
  ) => void;
  patchSegmentTranslationsForHistory: (
    historyId: string,
    updates: Array<{ id: string; translation: string }>
  ) => void;
  updateSegmentForSession: (
    sessionId: string,
    id: string,
    updates: Partial<Omit<TranscriptSegment, 'id'>>
  ) => void;
  deleteSegment: (id: string) => void;
  mergeSegments: (id1: string, id2: string) => void;
  splitTranscriptSegment: (
    id: string,
    leftText: string,
    rightText: string,
    leftTranslation?: string,
    rightTranslation?: string
  ) => string | null;
  finalizeLastSegment: () => void;
  finalizeLastSegmentForSession: (sessionId: string) => void;
  applyTranscriptUpdate: (update: TranscriptUpdate, activeSegmentId?: string | null) => void;
  applyTranscriptUpdateToSession: (
    sessionId: string,
    update: TranscriptUpdate,
    activeSegmentId?: string | null
  ) => void;
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
  setAutoSaveState: (historyId: string, status: AutoSaveStatus, errorMessage?: string) => void;
  clearAutoSaveState: (historyId?: string) => void;
}

// Helper to get active session
function getActiveSession(state: TranscriptStore): SessionData {
  return state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
}

type StoreSet = Parameters<StateCreator<TranscriptStore>>[0];
type StoreGet = Parameters<StateCreator<TranscriptStore>>[1];

// Helper to mutate active session
function updateActiveSession(
  set: StoreSet,
  updater: (session: SessionData) => Partial<SessionData>
) {
  set((state: TranscriptStore) => {
    const session = state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
    const patch = updater(session);
    if (!patch || Object.keys(patch).length === 0) {
      return state;
    }
    return {
      sessions: {
        ...state.sessions,
        [state.activeSessionId]: { ...session, ...patch },
      },
    };
  });
}

function pauseSessionIfPlaying(
  sessions: Record<string, SessionData>,
  sessionId: string
): Record<string, SessionData> {
  const session = sessions[sessionId];
  if (!session?.isPlaying) {
    return sessions;
  }
  return {
    ...sessions,
    [sessionId]: { ...session, isPlaying: false },
  };
}

function revokeBlobAudioUrl(audioUrl: string | null | undefined): void {
  if (audioUrl?.startsWith('blob:')) {
    URL.revokeObjectURL(audioUrl);
  }
}

function applyTranslationPatchToSegments(
  segments: TranscriptSegment[],
  translationMap: Map<string, string>
): TranscriptSegment[] | null {
  let changed = false;
  const nextSegments = segments.map((s) => {
    const tr = translationMap.get(s.id);
    if (tr !== undefined && (s.translation !== tr || s.draftTranslation !== undefined)) {
      changed = true;
      return { ...s, translation: tr, draftTranslation: undefined };
    }
    return s;
  });
  return changed ? nextSegments : null;
}

function resolveHistoryKey(historyId: string | undefined, get: StoreGet): string {
  const activeSession = getActiveSession(get());
  return resolveTranscriptHistoryKey(historyId, activeSession.sourceHistoryId);
}

function calculateSegmentUpdate(
  segments: TranscriptSegment[],
  segment: TranscriptSegment
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
  createSessionId: 'default',
  recordingSessionId: null,
  sessions: { default: { ...DEFAULT_SESSION_DATA } },
  mode: 'create',
  ingestionFocus: 'live',
  processingStatus: 'idle',
  processingProgress: 0,
  isRecording: false,
  isCaptionMode: false,
  isPaused: false,
  isAgentRecording: false,
  agentRecordingHistoryId: null,
  summaryStates: { current: createDefaultSummaryState() },
  llmStates: { current: { ...DEFAULT_LLM_STATE } },
  autoSaveStates: {},

  // --- Runtime ---
  setMode: (mode, focus) => {
    set((state) => {
      let nextMode: AppMode = state.mode;
      let nextFocus: IngestionFocus | undefined;
      let nextActiveSessionId = state.activeSessionId;
      let nextCreateSessionId = state.createSessionId;

      if ((mode as string) === 'live') {
        const recId = state.recordingSessionId;
        const targetSessionId =
          recId && state.sessions[recId]
            ? recId
            : state.createSessionId && state.sessions[state.createSessionId]
              ? state.createSessionId
              : 'default';
        nextMode = 'create';
        nextFocus = 'live';
        nextActiveSessionId = targetSessionId;
        nextCreateSessionId = targetSessionId;
      } else if ((mode as string) === 'batch') {
        const activeItemId = activeBatchItemIdResolver?.();
        const targetSessionId =
          activeItemId && state.sessions[activeItemId]
            ? activeItemId
            : state.createSessionId && state.sessions[state.createSessionId]
              ? state.createSessionId
              : 'default';
        nextMode = 'create';
        nextFocus = 'batch';
        nextActiveSessionId = targetSessionId;
        nextCreateSessionId = targetSessionId;
      } else if (mode === 'create') {
        const recId = state.recordingSessionId;
        nextMode = 'create';
        if (focus === undefined) {
          const createId = state.createSessionId;
          const recordingSessionExists = Boolean(recId && state.sessions[recId]);
          const activeBatchId =
            state.ingestionFocus === 'batch' ? activeBatchItemIdResolver?.() : null;
          const targetSessionId =
            recId && recordingSessionExists
              ? recId
              : createId && createId !== 'default' && state.sessions[createId]
                ? createId
                : activeBatchId && state.sessions[activeBatchId]
                  ? activeBatchId
                  : 'default';
          nextActiveSessionId = targetSessionId;
          nextCreateSessionId = targetSessionId;
          // Recording controls live in the live pane; never restore behind a stale batch focus.
          if (recordingSessionExists) {
            nextFocus = 'live';
          }
        } else {
          nextFocus = focus;
          const focusItemId =
            focus === 'live' ? recId : focus === 'batch' ? activeBatchItemIdResolver?.() : null;
          // An explicit focus selects that ingestion surface fresh: the recording session
          // (live) or active batch item, otherwise the empty default pane. Only a focus-less
          // tab switch restores the previous create session.
          const targetSessionId =
            focusItemId && state.sessions[focusItemId] ? focusItemId : 'default';
          nextActiveSessionId = targetSessionId;
          nextCreateSessionId = targetSessionId;
        }
      } else if (mode === 'projects' || mode === 'tools') {
        nextMode = mode;
        nextCreateSessionId =
          state.mode === 'create' ? state.activeSessionId : state.createSessionId;
      } else {
        return state;
      }

      const shouldPause = nextMode !== state.mode || nextActiveSessionId !== state.activeSessionId;
      const nextSessions = shouldPause
        ? pauseSessionIfPlaying(state.sessions, state.activeSessionId)
        : state.sessions;

      return {
        mode: nextMode,
        ...(nextFocus !== undefined ? { ingestionFocus: nextFocus } : {}),
        activeSessionId: nextActiveSessionId,
        createSessionId: nextCreateSessionId,
        ...(nextSessions !== state.sessions ? { sessions: nextSessions } : {}),
      };
    });
  },
  setIngestionFocus: (ingestionFocus) => {
    set((state) => {
      const isCreateMode = state.mode === 'create';
      let targetSessionId = state.activeSessionId;
      if (ingestionFocus === 'live') {
        const recId = state.recordingSessionId;
        targetSessionId = recId && state.sessions[recId] ? recId : 'default';
      } else if (ingestionFocus === 'batch') {
        const activeItemId = activeBatchItemIdResolver?.();
        targetSessionId = activeItemId && state.sessions[activeItemId] ? activeItemId : 'default';
      } else {
        return { ingestionFocus };
      }
      const nextSessions =
        isCreateMode && targetSessionId !== state.activeSessionId
          ? pauseSessionIfPlaying(state.sessions, state.activeSessionId)
          : state.sessions;
      return {
        ingestionFocus,
        ...(isCreateMode ? { activeSessionId: targetSessionId } : {}),
        ...(nextSessions !== state.sessions ? { sessions: nextSessions } : {}),
      };
    });
  },
  setProcessingStatus: (processingStatus) => set({ processingStatus }),
  setProcessingProgress: (processingProgress) => set({ processingProgress }),
  setIsRecording: (isRecording) => set({ isRecording }),
  setIsCaptionMode: (isCaptionMode) => set({ isCaptionMode }),
  setIsPaused: (isPaused) => set({ isPaused }),
  setIsAgentRecording: (isAgentRecording, agentRecordingHistoryId = null) =>
    set({ isAgentRecording, agentRecordingHistoryId }),
  // --- Coordinator / Pointers ---
  setRecordingSessionId: (id) =>
    set((state) => ({
      recordingSessionId: id,
      createSessionId: id ? id : state.createSessionId,
    })),
  openSession: (args) => {
    if (typeof args === 'string') {
      get().setAudioUrl(args);
      return;
    }
    const { segments = [], sourceHistoryId, title, icon, audioUrl } = args;
    const sessionId = sourceHistoryId || uuidv4();
    // Never clobber the in-progress recording (e.g. backup import reloading the draft from disk).
    if (get().recordingSessionId === sessionId) {
      return;
    }
    if (sourceHistoryId) {
      const autoSaveStates = { ...get().autoSaveStates };
      delete autoSaveStates[sourceHistoryId];
      delete autoSaveStates.current;
      set({ autoSaveStates });
    }
    set((state) => {
      const nextSessions =
        state.activeSessionId !== sessionId
          ? pauseSessionIfPlaying(state.sessions, state.activeSessionId)
          : state.sessions;
      return {
        activeSessionId: sessionId,
        createSessionId: state.mode === 'create' ? sessionId : state.createSessionId,
        sessions: {
          ...nextSessions,
          [sessionId]: {
            ...DEFAULT_SESSION_DATA,
            segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
            sourceHistoryId,
            title: title || '',
            icon: icon || null,
            audioUrl: audioUrl !== undefined ? audioUrl : null,
          },
        },
      };
    });
  },
  loadTranscriptSession: (segments, sourceHistoryId, title, icon) => {
    get().openSession({ segments, sourceHistoryId, title, icon });
  },
  activateSession: (sessionId) => {
    set((state) => {
      if (!state.sessions[sessionId]) {
        return state;
      }
      if (state.activeSessionId === sessionId) {
        return state;
      }
      const nextSessions = pauseSessionIfPlaying(state.sessions, state.activeSessionId);
      return {
        activeSessionId: sessionId,
        ...(nextSessions !== state.sessions ? { sessions: nextSessions } : {}),
      };
    });
  },
  evictSession: (sessionId) => {
    set((state) => {
      if (!sessionId || state.recordingSessionId === sessionId) {
        return state;
      }
      const existing = state.sessions[sessionId];
      if (!existing) {
        return state;
      }
      revokeBlobAudioUrl(existing.audioUrl);
      if (sessionId === 'default') {
        return {
          sessions: {
            ...state.sessions,
            default: { ...DEFAULT_SESSION_DATA },
          },
        };
      }
      const nextSessions = { ...state.sessions };
      delete nextSessions[sessionId];
      return {
        sessions: nextSessions,
        activeSessionId: state.activeSessionId === sessionId ? 'default' : state.activeSessionId,
        createSessionId: state.createSessionId === sessionId ? 'default' : state.createSessionId,
      };
    });
  },
  evictSessionsByHistoryIds: (historyIds) => {
    if (historyIds.length === 0) return;
    const idSet = new Set(historyIds);
    set((state) => {
      let changed = false;
      const nextSessions = { ...state.sessions };
      let nextActiveSessionId = state.activeSessionId;
      let nextCreateSessionId = state.createSessionId;

      for (const [sessionId, session] of Object.entries(state.sessions)) {
        if (sessionId === state.recordingSessionId) {
          continue;
        }
        const matchesHistory =
          (sessionId !== 'default' && idSet.has(sessionId)) ||
          Boolean(session.sourceHistoryId && idSet.has(session.sourceHistoryId));
        if (!matchesHistory) {
          continue;
        }
        changed = true;
        revokeBlobAudioUrl(session.audioUrl);
        if (sessionId === 'default') {
          nextSessions.default = { ...DEFAULT_SESSION_DATA };
        } else {
          delete nextSessions[sessionId];
          if (nextActiveSessionId === sessionId) {
            nextActiveSessionId = 'default';
          }
          if (nextCreateSessionId === sessionId) {
            nextCreateSessionId = 'default';
          }
        }
      }

      const nextAutoSaveStates = { ...state.autoSaveStates };
      let autoSaveChanged = false;
      for (const id of idSet) {
        if (nextAutoSaveStates[id]) {
          delete nextAutoSaveStates[id];
          autoSaveChanged = true;
        }
      }

      if (!changed && !autoSaveChanged) {
        return state;
      }
      return {
        ...(changed
          ? {
              sessions: nextSessions,
              activeSessionId: nextActiveSessionId,
              createSessionId: nextCreateSessionId,
            }
          : {}),
        ...(autoSaveChanged ? { autoSaveStates: nextAutoSaveStates } : {}),
      };
    });
  },
  evictNonRecordingSessions: (keepSessionIds = []) => {
    const keepSet = new Set(keepSessionIds.filter(Boolean));
    set((state) => {
      let changed = false;
      const nextSessions: Record<string, SessionData> = {};
      const evictedHistoryIds = new Set<string>();

      for (const [sessionId, session] of Object.entries(state.sessions)) {
        const shouldKeep =
          sessionId === 'default' ||
          sessionId === state.recordingSessionId ||
          keepSet.has(sessionId) ||
          Boolean(session.sourceHistoryId && keepSet.has(session.sourceHistoryId));
        if (shouldKeep) {
          nextSessions[sessionId] = session;
        } else {
          changed = true;
          revokeBlobAudioUrl(session.audioUrl);
          if (session.sourceHistoryId) {
            evictedHistoryIds.add(session.sourceHistoryId);
          }
        }
      }

      if (!changed) {
        return state;
      }

      const nextAutoSaveStates = { ...state.autoSaveStates };
      let autoSaveChanged = false;
      for (const id of evictedHistoryIds) {
        if (nextAutoSaveStates[id]) {
          delete nextAutoSaveStates[id];
          autoSaveChanged = true;
        }
      }

      return {
        sessions: nextSessions,
        activeSessionId: nextSessions[state.activeSessionId] ? state.activeSessionId : 'default',
        createSessionId: nextSessions[state.createSessionId] ? state.createSessionId : 'default',
        ...(autoSaveChanged ? { autoSaveStates: nextAutoSaveStates } : {}),
      };
    });
  },
  updateSessionMetaByHistoryId: (historyId, meta) => {
    if (!historyId) return;
    set((state) => {
      let changed = false;
      const nextSessions = { ...state.sessions };
      for (const [sessionId, session] of Object.entries(state.sessions)) {
        if (sessionId === historyId || session.sourceHistoryId === historyId) {
          const nextTitle = meta.title !== undefined ? meta.title : session.title;
          const nextIcon = meta.icon !== undefined ? meta.icon : session.icon;
          if (nextTitle !== session.title || nextIcon !== session.icon) {
            nextSessions[sessionId] = { ...session, title: nextTitle, icon: nextIcon };
            changed = true;
          }
        }
      }
      return changed ? { sessions: nextSessions } : state;
    });
  },
  clearActiveTranscriptSession: (options) => {
    const state = get();
    // The recording session is off-limits while a record session owns it; callers like
    // backup import and history deletion must not wipe the in-progress transcript.
    if (state.recordingSessionId && state.activeSessionId === state.recordingSessionId) {
      return;
    }
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
    const session = state.sessions[historyId];
    if (!session) return;

    set({
      sessions: {
        ...state.sessions,
        [historyId]: {
          ...session,
          sourceHistoryId: historyId,
          title,
          icon: icon || null,
          audioUrl: audioUrl !== undefined ? audioUrl : session.audioUrl,
        },
      },
    });
    get().rekeyCurrentSummaryState(historyId);
  },

  // --- Session Data Mutations ---
  setSourceHistoryId: (sourceHistoryId) => updateActiveSession(set, () => ({ sourceHistoryId })),
  setTitle: (title) => updateActiveSession(set, () => ({ title: title || '' })),
  setIcon: (icon) => updateActiveSession(set, () => ({ icon })),
  setSegments: (segments) =>
    updateActiveSession(set, () => ({
      segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
    })),
  setSegmentsForSession: (sessionId, segments) => {
    set((state: TranscriptStore) => ({
      sessions: {
        ...state.sessions,
        [sessionId]: {
          ...(state.sessions[sessionId] || DEFAULT_SESSION_DATA),
          segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
        },
      },
    }));
  },
  setSegmentsForHistory: (historyId, segments) => {
    const normalized = normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start);
    set((state) => {
      if (!historyId || historyId === 'current') {
        const activeSession = state.sessions[state.activeSessionId];
        if (!activeSession || activeSession.sourceHistoryId) {
          return state;
        }
        return {
          sessions: {
            ...state.sessions,
            [state.activeSessionId]: { ...activeSession, segments: normalized },
          },
        };
      }
      let changed = false;
      const nextSessions = { ...state.sessions };
      for (const [sessionId, session] of Object.entries(state.sessions)) {
        if (sessionId === historyId || session.sourceHistoryId === historyId) {
          nextSessions[sessionId] = { ...session, segments: normalized };
          changed = true;
        }
      }
      return changed ? { sessions: nextSessions } : state;
    });
  },
  addSegment: (segment) => {
    const id = uuidv4();
    const newSegment = normalizeTranscriptSegment({ ...segment, id });
    updateActiveSession(set, (session) => ({
      segments: [...session.segments, newSegment].sort((a, b) => a.start - b.start),
    }));
    return id;
  },
  upsertSegment: (segment) => {
    updateActiveSession(set, (session) => ({
      segments: calculateSegmentUpdate(session.segments, normalizeTranscriptSegment(segment))
        .segments,
    }));
  },
  updateSegment: (id, updates) => {
    updateActiveSession(set, (session) => ({
      segments: session.segments.map((s) =>
        s.id === id ? normalizeTranscriptSegment({ ...s, ...updates }) : s
      ),
    }));
  },
  patchSegmentTranslation: (id, translation) => {
    get().patchSegmentTranslations([{ id, translation }]);
  },
  patchSegmentTranslations: (updates) => {
    if (updates.length === 0) return;
    const translationMap = new Map(updates.map((u) => [u.id, u.translation]));
    updateActiveSession(set, (session) => {
      const nextSegments = applyTranslationPatchToSegments(session.segments, translationMap);
      return nextSegments ? { segments: nextSegments } : {};
    });
  },
  patchSegmentTranslationsForSession: (sessionId, updates) => {
    if (updates.length === 0) return;
    const translationMap = new Map(updates.map((u) => [u.id, u.translation]));
    set((state) => {
      const session = state.sessions[sessionId];
      if (!session) return state;
      const nextSegments = applyTranslationPatchToSegments(session.segments, translationMap);
      if (!nextSegments) return state;
      return {
        sessions: {
          ...state.sessions,
          [sessionId]: { ...session, segments: nextSegments },
        },
      };
    });
  },
  patchSegmentTranslationsForHistory: (historyId, updates) => {
    if (updates.length === 0) return;
    const translationMap = new Map(updates.map((u) => [u.id, u.translation]));
    set((state) => {
      if (!historyId || historyId === 'current') {
        const activeSession = state.sessions[state.activeSessionId];
        if (!activeSession || activeSession.sourceHistoryId) {
          return state;
        }
        const nextSegments = applyTranslationPatchToSegments(
          activeSession.segments,
          translationMap
        );
        if (!nextSegments) return state;
        return {
          sessions: {
            ...state.sessions,
            [state.activeSessionId]: { ...activeSession, segments: nextSegments },
          },
        };
      }
      let changed = false;
      const nextSessions = { ...state.sessions };
      for (const [sessionId, session] of Object.entries(state.sessions)) {
        if (sessionId === historyId || session.sourceHistoryId === historyId) {
          const nextSegments = applyTranslationPatchToSegments(session.segments, translationMap);
          if (nextSegments) {
            nextSessions[sessionId] = { ...session, segments: nextSegments };
            changed = true;
          }
        }
      }
      return changed ? { sessions: nextSessions } : state;
    });
  },
  updateSegmentForSession: (sessionId, id, updates) => {
    set((state: TranscriptStore) => {
      const session = state.sessions[sessionId];
      if (!session) return state;
      return {
        sessions: {
          ...state.sessions,
          [sessionId]: {
            ...session,
            segments: session.segments.map((s) =>
              s.id === id ? normalizeTranscriptSegment({ ...s, ...updates }) : s
            ),
          },
        },
      };
    });
  },
  deleteSegment: (id) =>
    updateActiveSession(set, (session) => ({
      segments: session.segments.filter((s) => s.id !== id),
      editingSegmentId: session.editingSegmentId === id ? null : session.editingSegmentId,
      activeSegmentId: session.activeSegmentId === id ? null : session.activeSegmentId,
    })),
  mergeSegments: (id1, id2) => {
    updateActiveSession(set, (session) => {
      const seg1 = session.segments.find((s) => s.id === id1);
      const seg2 = session.segments.find((s) => s.id === id2);
      if (!seg1 || !seg2) return {};
      const [first, second] = seg1.start <= seg2.start ? [seg1, seg2] : [seg2, seg1];
      if (!areSpeakerTagsEqual(first.speaker, second.speaker)) return {};
      const stripParagraph = (t: string) => t.replace(/<\/?p[^>]*>/gi, '');
      const t1 = first.translation?.trim();
      const t2 = second.translation?.trim();
      const mergedTranslation =
        t1 && t2 ? `${t1}${needsWordBoundarySpace(t1, t2) ? ' ' : ''}${t2}` : t1 || t2 || undefined;
      const text1 = stripParagraph(first.text);
      const text2 = stripParagraph(second.text);
      const textSpace = needsWordBoundarySpace(text1, text2) ? ' ' : '';
      const merged: TranscriptSegment = {
        id: first.id,
        start: first.start,
        end: second.end,
        text: `<p>${text1}${textSpace}${text2}</p>`,
        isFinal: first.isFinal && second.isFinal,
        speaker: first.speaker,
        speakerAttribution: first.speakerAttribution,
        translation: mergedTranslation,
        draftTranslation: undefined,
      };
      return {
        segments: session.segments
          .filter((s) => s.id !== second.id)
          .map((s) => (s.id === first.id ? normalizeTranscriptSegment(merged) : s)),
      };
    });
  },
  splitTranscriptSegment: (id, leftText, rightText, leftTranslation, rightTranslation) => {
    const session = getActiveSession(get());
    const segment = session.segments.find((s) => s.id === id);
    if (!segment) return null;
    const plainText = stripHtmlTags(leftText) + stripHtmlTags(rightText);
    const caretOffset = stripHtmlTags(leftText).length;
    const newSegmentId = uuidv4();
    const { segmentLeft, segmentRight } = performSegmentSplit(
      segment,
      caretOffset,
      plainText,
      leftText,
      rightText,
      newSegmentId,
      leftTranslation,
      rightTranslation
    );

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
      editingSegmentId: newSegmentId,
    }));
    return newSegmentId;
  },
  finalizeLastSegment: () =>
    updateActiveSession(set, (session) => {
      if (session.segments.length === 0) return {};
      const lastIndex = session.segments.length - 1;
      const lastSegment = session.segments[lastIndex];
      if (lastSegment.isFinal) return {};
      const segments = [...session.segments];
      segments[lastIndex] = { ...lastSegment, isFinal: true };
      return { segments };
    }),
  finalizeLastSegmentForSession: (sessionId) =>
    set((state: TranscriptStore) => {
      const session = state.sessions[sessionId];
      if (!session || session.segments.length === 0) return state;
      const lastIndex = session.segments.length - 1;
      const lastSegment = session.segments[lastIndex];
      if (lastSegment.isFinal) return state;
      const segments = [...session.segments];
      segments[lastIndex] = { ...lastSegment, isFinal: true };
      return {
        sessions: {
          ...state.sessions,
          [sessionId]: { ...session, segments },
        },
      };
    }),
  applyTranscriptUpdate: (update, activeSegmentIdParam) => {
    updateActiveSession(set, (session) => {
      const normalizedUpdate = normalizeTranscriptUpdate(update);
      const removeIds = new Set(normalizedUpdate.removeIds);
      let nextSegments =
        removeIds.size > 0
          ? session.segments.filter((s) => !removeIds.has(s.id))
          : [...session.segments];

      normalizedUpdate.upsertSegments.forEach((seg) => {
        const existingIndex = nextSegments.findIndex((c) => c.id === seg.id);
        if (existingIndex !== -1) nextSegments[existingIndex] = seg;
        else nextSegments = [...nextSegments, seg].sort((a, b) => a.start - b.start);
      });

      let nextActiveId = session.activeSegmentId;
      let nextActiveIndex = -1;
      if (activeSegmentIdParam !== undefined) {
        nextActiveId = activeSegmentIdParam;
        nextActiveIndex = nextActiveId ? nextSegments.findIndex((s) => s.id === nextActiveId) : -1;
      } else if (nextActiveId && removeIds.has(nextActiveId)) {
        const activeIndex = nextSegments.findIndex((s) => s.id === nextActiveId);
        if (activeIndex === -1) nextActiveId = null;
        else nextActiveIndex = activeIndex;
      }
      return {
        segments: nextSegments,
        activeSegmentId: nextActiveId,
        activeSegmentIndex: nextActiveIndex,
      };
    });
  },
  applyTranscriptUpdateToSession: (sessionId, update, activeSegmentIdParam) => {
    set((state: TranscriptStore) => {
      const session = state.sessions[sessionId] || DEFAULT_SESSION_DATA;
      const normalizedUpdate = normalizeTranscriptUpdate(update);
      const removeIds = new Set(normalizedUpdate.removeIds);
      let nextSegments =
        removeIds.size > 0
          ? session.segments.filter((s) => !removeIds.has(s.id))
          : [...session.segments];

      normalizedUpdate.upsertSegments.forEach((seg) => {
        const existingIndex = nextSegments.findIndex((c) => c.id === seg.id);
        if (existingIndex !== -1) nextSegments[existingIndex] = seg;
        else nextSegments = [...nextSegments, seg].sort((a, b) => a.start - b.start);
      });

      let nextActiveId = session.activeSegmentId;
      let nextActiveIndex = -1;
      if (activeSegmentIdParam !== undefined) {
        nextActiveId = activeSegmentIdParam;
        nextActiveIndex = nextActiveId ? nextSegments.findIndex((s) => s.id === nextActiveId) : -1;
      } else if (nextActiveId && removeIds.has(nextActiveId)) {
        const activeIndex = nextSegments.findIndex((s) => s.id === nextActiveId);
        if (activeIndex === -1) nextActiveId = null;
        else nextActiveIndex = activeIndex;
      }

      return {
        sessions: {
          ...state.sessions,
          [sessionId]: {
            ...session,
            segments: nextSegments,
            activeSegmentId: nextActiveId,
            activeSegmentIndex: nextActiveIndex,
          },
        },
      };
    });
  },
  upsertTranscriptSegmentAndSetActive: (segment) =>
    updateActiveSession(set, (session) => {
      const nextSegmentsData = calculateSegmentUpdate(
        session.segments,
        normalizeTranscriptSegment(segment)
      );
      let nextSegments = nextSegmentsData.segments;
      const isCaptionMode = get().isCaptionMode;
      if (isCaptionMode && nextSegments.length > 50) {
        nextSegments = nextSegments.slice(-50);
      }
      const activeIndex = nextSegments.findIndex((c) => c.id === segment.id);
      return {
        segments: nextSegments,
        activeSegmentId: segment.id,
        activeSegmentIndex: activeIndex === -1 ? nextSegments.length - 1 : activeIndex,
      };
    }),
  setEditingSegmentId: (id) => updateActiveSession(set, () => ({ editingSegmentId: id })),
  addAligningSegmentId: (id) =>
    updateActiveSession(set, (session) => {
      const next = new Set(session.aligningSegmentIds);
      next.add(id);
      return { aligningSegmentIds: next };
    }),
  removeAligningSegmentId: (id) =>
    updateActiveSession(set, (session) => {
      const next = new Set(session.aligningSegmentIds);
      next.delete(id);
      return { aligningSegmentIds: next };
    }),

  // --- Playback Mutations ---
  setAudioFile: (file) =>
    updateActiveSession(set, (session) => {
      if (session.audioUrl) URL.revokeObjectURL(session.audioUrl);
      return {
        audioFile: file,
        audioUrl: file ? URL.createObjectURL(file) : null,
        isPlaying: false,
        currentTime: 0,
        activeSegmentId: null,
        activeSegmentIndex: -1,
        seekRequest: null,
        lastSeekTimestamp: 0,
      };
    }),
  setAudioUrl: (audioUrl) =>
    updateActiveSession(set, () => ({
      audioUrl,
      isPlaying: false,
      currentTime: 0,
      activeSegmentId: null,
      activeSegmentIndex: -1,
      seekRequest: null,
      lastSeekTimestamp: 0,
    })),
  setCurrentTime: (time) =>
    updateActiveSession(set, (session) => {
      const { segment, index } = findSegmentAndIndexForTime(
        session.segments,
        time,
        session.activeSegmentIndex
      );
      if (segment?.id !== session.activeSegmentId) {
        return {
          currentTime: time,
          activeSegmentId: segment?.id || null,
          activeSegmentIndex: index,
        };
      }
      return { currentTime: time };
    }),
  setIsPlaying: (isPlaying) => updateActiveSession(set, () => ({ isPlaying })),
  setActiveSegmentId: (activeSegmentId, activeSegmentIndex = -1) =>
    updateActiveSession(set, () => ({ activeSegmentId, activeSegmentIndex })),
  resetActiveSegmentIndex: () => updateActiveSession(set, () => ({ activeSegmentIndex: -1 })),
  requestSeek: (time) => {
    get().setCurrentTime(time);
    const timestamp = Date.now();
    updateActiveSession(set, () => ({
      seekRequest: { time, timestamp },
      lastSeekTimestamp: timestamp,
    }));
  },

  // --- Sidecar Mutations ---
  getLlmState: (historyId) => {
    const id = resolveHistoryKey(historyId, get);
    return get().llmStates[id] || { ...DEFAULT_LLM_STATE };
  },
  updateLlmState: (updates, historyId) =>
    set((state) => {
      const id = resolveHistoryKey(historyId, get);
      return {
        llmStates: {
          ...state.llmStates,
          [id]: { ...(state.llmStates[id] || { ...DEFAULT_LLM_STATE }), ...updates },
        },
      };
    }),
  getSummaryState: (historyId) => {
    const id = resolveHistoryKey(historyId, get);
    return get().summaryStates[id] || createDefaultSummaryState();
  },
  setSummaryState: (summaryState, historyId) =>
    set((state) => {
      const id = resolveHistoryKey(historyId, get);
      return {
        summaryStates: {
          ...state.summaryStates,
          [id]: { ...createDefaultSummaryState(), ...summaryState, record: summaryState.record },
        },
      };
    }),
  updateSummaryState: (updates, historyId) =>
    set((state) => {
      const id = resolveHistoryKey(historyId, get);
      return {
        summaryStates: {
          ...state.summaryStates,
          [id]: { ...(state.summaryStates[id] || createDefaultSummaryState()), ...updates },
        },
      };
    }),
  hydrateSummaryState: (payload: HistorySummaryPayload, historyId) => {
    // Basic hydrate implementation to satisfy the type.
    const activeTemplateId = payload.activeTemplateId || 'default';
    get().setSummaryState(
      {
        activeTemplateId,
        record: payload.record ?? undefined,
        streamingContent: undefined,
        streamingThought: undefined,
        isGenerating: false,
        generationProgress: 0,
      },
      historyId
    );
  },
  clearSummaryState: (historyId) =>
    set((state) => {
      const id = resolveHistoryKey(historyId, get);
      if (!state.summaryStates[id]) return state;
      const summaryStates = { ...state.summaryStates };
      delete summaryStates[id];
      return { summaryStates };
    }),
  rekeyCurrentSummaryState: (nextHistoryId) =>
    set((state) => ({
      summaryStates: rekeyCurrentSummaryStateEntry(state.summaryStates, nextHistoryId),
    })),
  setAutoSaveState: (historyId, status, errorMessage) =>
    set((state) => {
      if (!historyId || historyId === 'current') return state;
      return {
        autoSaveStates: {
          ...state.autoSaveStates,
          [historyId]: {
            status,
            updatedAt: Date.now(),
            errorMessage: status === 'error' ? errorMessage : undefined,
          },
        },
      };
    }),
  clearAutoSaveState: (historyId) =>
    set((state) => {
      const id = resolveHistoryKey(historyId, get);
      if (!id || id === 'current' || !state.autoSaveStates[id]) return state;
      const autoSaveStates = { ...state.autoSaveStates };
      delete autoSaveStates[id];
      return { autoSaveStates };
    }),
}));

export const applyTranscriptUpdateToSession = (
  sessionId: string,
  update: TranscriptUpdate,
  activeSegmentId?: string | null
): void => {
  useTranscriptStore.getState().applyTranscriptUpdateToSession(sessionId, update, activeSegmentId);
};

export const updateTranscriptSegmentForSession = (
  sessionId: string,
  id: string,
  updates: Partial<Omit<TranscriptSegment, 'id'>>
): void => {
  useTranscriptStore.getState().updateSegmentForSession(sessionId, id, updates);
};

export const finalizeLastSegmentForSession = (sessionId: string): void => {
  useTranscriptStore.getState().finalizeLastSegmentForSession(sessionId);
};

export const patchSegmentTranslation = (id: string, translation: string): void => {
  useTranscriptStore.getState().patchSegmentTranslation(id, translation);
};
export const patchSegmentTranslations = (
  updates: Array<{ id: string; translation: string }>
): void => {
  useTranscriptStore.getState().patchSegmentTranslations(updates);
};

export const setRecordingSessionId = (id: string | null): void => {
  useTranscriptStore.getState().setRecordingSessionId(id);
};

export const evictTranscriptSessionsByHistoryIds = (historyIds: string[]): void => {
  useTranscriptStore.getState().evictSessionsByHistoryIds(historyIds);
};

export const evictNonRecordingSessions = (keepSessionIds?: string[]): void => {
  useTranscriptStore.getState().evictNonRecordingSessions(keepSessionIds);
};

export const updateSessionMetaByHistoryId = (
  historyId: string,
  meta: { title?: string; icon?: string | null }
): void => {
  useTranscriptStore.getState().updateSessionMetaByHistoryId(historyId, meta);
};

export const setSegmentsForSession = (sessionId: string, segments: TranscriptSegment[]): void => {
  useTranscriptStore.getState().setSegmentsForSession(sessionId, segments);
};

export const setSegmentsForHistory = (historyId: string, segments: TranscriptSegment[]): void => {
  useTranscriptStore.getState().setSegmentsForHistory(historyId, segments);
};

export const patchSegmentTranslationsForSession = (
  sessionId: string,
  updates: Array<{ id: string; translation: string }>
): void => {
  useTranscriptStore.getState().patchSegmentTranslationsForSession(sessionId, updates);
};

export const patchSegmentTranslationsForHistory = (
  historyId: string,
  updates: Array<{ id: string; translation: string }>
): void => {
  useTranscriptStore.getState().patchSegmentTranslationsForHistory(historyId, updates);
};
