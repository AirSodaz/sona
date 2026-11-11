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
import { documentMutationCoordinator } from '../services/task/documentMutationCoordinator';
import type { EffectivePipelineSnapshot } from '../types/project';
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
  rekeySessionSidecarEntries,
  resolveSidecarKey,
  resolveTranscriptHistoryKey,
  toSessionSidecarKey,
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
  projectId?: string | null;
  pipelineSnapshot?: EffectivePipelineSnapshot | null;
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
  projectId: undefined,
  pipelineSnapshot: undefined,
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
  createLiveSessionId: string;
  createBatchAdhocSessionId: string | null;
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
  // --- Sidecar (Keyed by historyId or `session:${sessionId}`) ---
  summaryStates: Record<string, TranscriptSummaryState>;
  llmStates: Record<string, LlmState>;
  autoSaveStates: Record<string, AutoSaveState>;

  // --- Actions ---
  // Coordinator / Session Pointers
  setCreateLiveSessionId: (id: string) => void;
  setCreateBatchAdhocSessionId: (id: string | null) => void;
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
          projectId?: string | null;
          pipelineSnapshot?: EffectivePipelineSnapshot | null;
        }
  ) => void;
  openLiveDraftSession: (args: {
    segments: TranscriptSegment[];
    sourceHistoryId: string | null;
    title?: string | null;
    icon?: string | null;
    audioUrl?: string | null;
    projectId?: string | null;
    pipelineSnapshot?: EffectivePipelineSnapshot | null;
  }) => void;
  openBatchAdhocDraft: (args: {
    segments: TranscriptSegment[];
    sourceHistoryId: string | null;
    title?: string | null;
    icon?: string | null;
    audioUrl?: string | null;
    projectId?: string | null;
    pipelineSnapshot?: EffectivePipelineSnapshot | null;
  }) => void;
  closeBatchAdhocDraft: (sessionId?: string) => void;
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
  clearTranscriptSession: (
    sessionId: string,
    options?: { clearAudio?: boolean; title?: string | null }
  ) => void;
  clearActiveTranscriptSession: (options?: { clearAudio?: boolean; title?: string | null }) => void;
  clearTranscriptSegments: () => void;
  clearSegments: () => void;
  syncSavedRecordingMeta: (
    title: string,
    historyId: string,
    icon: string | null | undefined,
    audioUrl?: string | null,
    originSessionId?: string
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
  // Session Data Mutations (Applies to activeSessionId or explicit sessionId)
  setSourceHistoryId: (id: string | null) => void;
  setSourceHistoryIdForSession: (sessionId: string, id: string | null) => void;
  setTitle: (title: string | null) => void;
  setTitleForSession: (sessionId: string, title: string | null) => void;
  setIcon: (icon: string | null) => void;
  setIconForSession: (sessionId: string, icon: string | null) => void;
  setSessionProjectContext: (
    sessionId: string,
    projectId: string | null,
    pipelineSnapshot?: EffectivePipelineSnapshot | null
  ) => void;
  setSegments: (segments: TranscriptSegment[]) => void;
  setSegmentsForSession: (sessionId: string, segments: TranscriptSegment[]) => void;
  setSegmentsForHistory: (historyId: string, segments: TranscriptSegment[]) => void;
  addSegment: (segment: Omit<TranscriptSegment, 'id'>) => string;
  addSegmentForSession: (sessionId: string, segment: Omit<TranscriptSegment, 'id'>) => string;
  upsertSegment: (segment: TranscriptSegment) => void;
  upsertSegmentForSession: (sessionId: string, segment: TranscriptSegment) => void;
  updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => void;
  patchSegmentTranslation: (id: string, translation: string) => void;
  patchSegmentTranslationForSession: (sessionId: string, id: string, translation: string) => void;
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
  deleteSegmentForSession: (sessionId: string, id: string) => void;
  mergeSegments: (id1: string, id2: string) => void;
  mergeSegmentsForSession: (sessionId: string, id1: string, id2: string) => void;
  splitTranscriptSegment: (
    id: string,
    leftText: string,
    rightText: string,
    leftTranslation?: string,
    rightTranslation?: string
  ) => string | null;
  splitTranscriptSegmentForSession: (
    sessionId: string,
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
  upsertTranscriptSegmentAndSetActiveForSession: (
    sessionId: string,
    segment: TranscriptSegment
  ) => void;
  setEditingSegmentId: (id: string | null) => void;
  setEditingSegmentIdForSession: (sessionId: string, id: string | null) => void;
  addAligningSegmentId: (id: string) => void;
  addAligningSegmentIdForSession: (sessionId: string, id: string) => void;
  removeAligningSegmentId: (id: string) => void;
  removeAligningSegmentIdForSession: (sessionId: string, id: string) => void;

  // Playback Mutations
  setAudioFile: (file: File | null) => void;
  setAudioFileForSession: (sessionId: string, file: File | null) => void;
  setAudioUrl: (url: string | null) => void;
  setAudioUrlForSession: (sessionId: string, url: string | null) => void;
  setCurrentTime: (time: number) => void;
  setCurrentTimeForSession: (sessionId: string, time: number) => void;
  setIsPlaying: (isPlaying: boolean) => void;
  setIsPlayingForSession: (sessionId: string, isPlaying: boolean) => void;
  setActiveSegmentId: (id: string | null, index?: number) => void;
  setActiveSegmentIdForSession: (sessionId: string, id: string | null, index?: number) => void;
  resetActiveSegmentIndex: () => void;
  resetActiveSegmentIndexForSession: (sessionId: string) => void;
  requestSeek: (time: number) => void;
  requestSeekForSession: (sessionId: string, time: number) => void;

  // Sidecar Mutations
  getLlmState: (historyId?: string) => LlmState;
  updateLlmState: (updates: Partial<LlmState>, historyId?: string) => void;
  getSummaryState: (historyId?: string) => TranscriptSummaryState;
  setSummaryState: (summaryState: Partial<TranscriptSummaryState>, historyId?: string) => void;
  updateSummaryState: (updates: Partial<TranscriptSummaryState>, historyId?: string) => void;
  hydrateSummaryState: (payload: HistorySummaryPayload, historyId?: string) => void;
  clearSummaryState: (historyId?: string) => void;
  rekeyCurrentSummaryState: (nextHistoryId: string | null) => void;
  rekeySessionSidecar: (sessionId: string, nextHistoryId: string) => void;
  setAutoSaveState: (historyId: string, status: AutoSaveStatus, errorMessage?: string) => void;
  clearAutoSaveState: (historyId?: string) => void;
}

export function selectCreateBoundSessionId(
  state: Pick<
    TranscriptStore,
    | 'ingestionFocus'
    | 'recordingSessionId'
    | 'createLiveSessionId'
    | 'createBatchAdhocSessionId'
    | 'sessions'
  >,
  activeBatchItemId?: string | null
): string {
  if (state.ingestionFocus === 'live') {
    if (state.recordingSessionId && state.sessions[state.recordingSessionId]) {
      return state.recordingSessionId;
    }
    return state.createLiveSessionId && state.sessions[state.createLiveSessionId]
      ? state.createLiveSessionId
      : 'default';
  }
  if (state.createBatchAdhocSessionId && state.sessions[state.createBatchAdhocSessionId]) {
    return state.createBatchAdhocSessionId;
  }
  return activeBatchItemId && state.sessions[activeBatchItemId] ? activeBatchItemId : 'default';
}

// Helper to get active session
function getActiveSession(state: TranscriptStore): SessionData {
  return state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
}

type StoreSet = Parameters<StateCreator<TranscriptStore>>[0];
type StoreGet = Parameters<StateCreator<TranscriptStore>>[1];

// Helper to mutate any target session cleanly
function updateSession(
  set: StoreSet,
  sessionId: string,
  updater: (session: SessionData) => Partial<SessionData>
) {
  set((state: TranscriptStore) => {
    const session = state.sessions[sessionId] || DEFAULT_SESSION_DATA;
    const patch = updater(session);
    if (!patch || Object.keys(patch).length === 0) {
      return state;
    }
    return {
      sessions: {
        ...state.sessions,
        [sessionId]: { ...session, ...patch },
      },
    };
  });
}

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
  const state = get();
  const activeSession = getActiveSession(state);
  return resolveTranscriptHistoryKey(
    historyId,
    activeSession.sourceHistoryId,
    state.activeSessionId
  );
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
  createLiveSessionId: 'default',
  createBatchAdhocSessionId: null,
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
  summaryStates: {},
  llmStates: {},
  autoSaveStates: {},

  // --- Runtime ---
  setMode: (mode, focus) => {
    set((state) => {
      let nextMode: AppMode = state.mode;
      let nextFocus: IngestionFocus | undefined;
      let nextActiveSessionId = state.activeSessionId;
      let nextCreateLiveSessionId = state.createLiveSessionId;
      const activeBatchId = activeBatchItemIdResolver?.();

      if ((mode as string) === 'live') {
        nextMode = 'create';
        nextFocus = 'live';
        nextActiveSessionId = selectCreateBoundSessionId(
          { ...state, ingestionFocus: 'live' },
          activeBatchId
        );
        if (nextActiveSessionId !== state.recordingSessionId) {
          nextCreateLiveSessionId = nextActiveSessionId;
        }
      } else if ((mode as string) === 'batch') {
        nextMode = 'create';
        nextFocus = 'batch';
        nextActiveSessionId = selectCreateBoundSessionId(
          { ...state, ingestionFocus: 'batch' },
          activeBatchId
        );
      } else if (mode === 'create') {
        const recId = state.recordingSessionId;
        const recordingSessionExists = Boolean(recId && state.sessions[recId]);
        nextMode = 'create';
        if (focus === undefined) {
          if (recordingSessionExists) {
            nextFocus = 'live';
          }
          const resolvedFocus = nextFocus ?? state.ingestionFocus;
          nextActiveSessionId = selectCreateBoundSessionId(
            { ...state, ingestionFocus: resolvedFocus },
            activeBatchId
          );
        } else {
          nextFocus = focus;
          nextActiveSessionId = selectCreateBoundSessionId(
            { ...state, ingestionFocus: focus },
            activeBatchId
          );
        }
      } else if (mode === 'projects' || mode === 'tools') {
        nextMode = mode;
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
        createLiveSessionId: nextCreateLiveSessionId,
        ...(nextSessions !== state.sessions ? { sessions: nextSessions } : {}),
      };
    });
  },
  setIngestionFocus: (ingestionFocus) => {
    set((state) => {
      if (ingestionFocus !== 'live' && ingestionFocus !== 'batch') {
        return { ingestionFocus };
      }
      const isCreateMode = state.mode === 'create';
      const activeBatchId = activeBatchItemIdResolver?.();
      const targetSessionId = selectCreateBoundSessionId(
        { ...state, ingestionFocus },
        activeBatchId
      );
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
  setCreateLiveSessionId: (id) => set({ createLiveSessionId: id || 'default' }),
  setCreateBatchAdhocSessionId: (id) => set({ createBatchAdhocSessionId: id }),
  setRecordingSessionId: (id) =>
    set((state) => ({
      recordingSessionId: id,
      createLiveSessionId: id ? id : state.createLiveSessionId,
    })),
  openSession: (args) => {
    if (typeof args === 'string') {
      get().setAudioUrl(args);
      return;
    }
    const {
      segments = [],
      sourceHistoryId,
      title,
      icon,
      audioUrl,
      projectId,
      pipelineSnapshot,
    } = args;
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
      const shouldUpdateCreateLive = state.mode === 'create' && state.ingestionFocus === 'live';
      return {
        activeSessionId: sessionId,
        ...(shouldUpdateCreateLive ? { createLiveSessionId: sessionId } : {}),
        sessions: {
          ...nextSessions,
          [sessionId]: {
            ...DEFAULT_SESSION_DATA,
            segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
            sourceHistoryId,
            title: title || '',
            icon: icon || null,
            audioUrl: audioUrl !== undefined ? audioUrl : null,
            projectId,
            pipelineSnapshot,
          },
        },
      };
    });
  },
  openLiveDraftSession: (args) => {
    const {
      segments = [],
      sourceHistoryId,
      title,
      icon,
      audioUrl,
      projectId,
      pipelineSnapshot,
    } = args;
    const sessionId = sourceHistoryId || uuidv4();
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
        mode: 'create',
        ingestionFocus: 'live',
        createLiveSessionId: sessionId,
        activeSessionId: sessionId,
        sessions: {
          ...nextSessions,
          [sessionId]: {
            ...DEFAULT_SESSION_DATA,
            segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
            sourceHistoryId,
            title: title || '',
            icon: icon || null,
            audioUrl: audioUrl !== undefined ? audioUrl : null,
            projectId,
            pipelineSnapshot,
          },
        },
      };
    });
  },
  openBatchAdhocDraft: (args) => {
    const {
      segments = [],
      sourceHistoryId,
      title,
      icon,
      audioUrl,
      projectId,
      pipelineSnapshot,
    } = args;
    const sessionId = sourceHistoryId || uuidv4();
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
        mode: 'create',
        ingestionFocus: 'batch',
        createBatchAdhocSessionId: sessionId,
        activeSessionId: sessionId,
        sessions: {
          ...nextSessions,
          [sessionId]: {
            ...DEFAULT_SESSION_DATA,
            segments: normalizeTranscriptSegments(segments).sort((a, b) => a.start - b.start),
            sourceHistoryId,
            title: title || '',
            icon: icon || null,
            audioUrl: audioUrl !== undefined ? audioUrl : null,
            projectId,
            pipelineSnapshot,
          },
        },
      };
    });
  },
  closeBatchAdhocDraft: (sessionId) => {
    const state = get();
    const targetId = sessionId ?? state.createBatchAdhocSessionId;
    if (!targetId) {
      return;
    }
    set((s) => ({
      createBatchAdhocSessionId:
        s.createBatchAdhocSessionId === targetId ? null : s.createBatchAdhocSessionId,
    }));
    if (targetId !== activeBatchItemIdResolver?.()) {
      get().evictSession(targetId);
    }
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
      const nextAutoSaveStates = { ...state.autoSaveStates };
      let autoSaveChanged = false;
      const sessionSidecarKey = toSessionSidecarKey(sessionId);
      if (nextAutoSaveStates[sessionSidecarKey]) {
        delete nextAutoSaveStates[sessionSidecarKey];
        autoSaveChanged = true;
      }
      if (!existing) {
        return autoSaveChanged ? { autoSaveStates: nextAutoSaveStates } : state;
      }
      revokeBlobAudioUrl(existing.audioUrl);
      if (existing.sourceHistoryId && nextAutoSaveStates[existing.sourceHistoryId]) {
        delete nextAutoSaveStates[existing.sourceHistoryId];
        autoSaveChanged = true;
      }
      if (sessionId === 'default') {
        return {
          sessions: {
            ...state.sessions,
            default: { ...DEFAULT_SESSION_DATA },
          },
          ...(autoSaveChanged ? { autoSaveStates: nextAutoSaveStates } : {}),
        };
      }
      const nextSessions = { ...state.sessions };
      delete nextSessions[sessionId];
      return {
        sessions: nextSessions,
        activeSessionId: state.activeSessionId === sessionId ? 'default' : state.activeSessionId,
        createLiveSessionId:
          state.createLiveSessionId === sessionId ? 'default' : state.createLiveSessionId,
        createBatchAdhocSessionId:
          state.createBatchAdhocSessionId === sessionId ? null : state.createBatchAdhocSessionId,
        ...(autoSaveChanged ? { autoSaveStates: nextAutoSaveStates } : {}),
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
      let nextCreateLiveSessionId = state.createLiveSessionId;
      let nextCreateBatchAdhocSessionId = state.createBatchAdhocSessionId;

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
          if (nextCreateLiveSessionId === sessionId) {
            nextCreateLiveSessionId = 'default';
          }
          if (nextCreateBatchAdhocSessionId === sessionId) {
            nextCreateBatchAdhocSessionId = null;
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
        const sessionSidecarKey = toSessionSidecarKey(id);
        if (nextAutoSaveStates[sessionSidecarKey]) {
          delete nextAutoSaveStates[sessionSidecarKey];
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
              createLiveSessionId: nextCreateLiveSessionId,
              createBatchAdhocSessionId: nextCreateBatchAdhocSessionId,
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
      const evictedIds = new Set<string>();

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
          evictedIds.add(toSessionSidecarKey(sessionId));
          if (session.sourceHistoryId) {
            evictedIds.add(session.sourceHistoryId);
          }
        }
      }

      if (!changed) {
        return state;
      }

      const nextAutoSaveStates = { ...state.autoSaveStates };
      let autoSaveChanged = false;
      for (const id of evictedIds) {
        if (nextAutoSaveStates[id]) {
          delete nextAutoSaveStates[id];
          autoSaveChanged = true;
        }
      }

      return {
        sessions: nextSessions,
        activeSessionId: nextSessions[state.activeSessionId] ? state.activeSessionId : 'default',
        createLiveSessionId: nextSessions[state.createLiveSessionId]
          ? state.createLiveSessionId
          : 'default',
        createBatchAdhocSessionId:
          state.createBatchAdhocSessionId && nextSessions[state.createBatchAdhocSessionId]
            ? state.createBatchAdhocSessionId
            : null,
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
  clearTranscriptSession: (sessionId, options) => {
    const state = get();
    if (!sessionId || (state.recordingSessionId && sessionId === state.recordingSessionId)) {
      return;
    }
    const existing = state.sessions[sessionId];
    if (options?.clearAudio) {
      revokeBlobAudioUrl(existing?.audioUrl);
    }
    const sidecarKey = resolveSidecarKey(sessionId, existing?.sourceHistoryId);
    updateSession(set, sessionId, (session) => ({
      ...DEFAULT_SESSION_DATA,
      title: options?.title ?? session.title,
      audioFile: options?.clearAudio ? null : session.audioFile,
      audioUrl: options?.clearAudio ? null : session.audioUrl,
    }));
    get().clearSummaryState(sidecarKey);
  },
  clearActiveTranscriptSession: (options) => {
    const state = get();
    get().clearTranscriptSession(state.activeSessionId, options);
  },
  clearSegments: () => get().clearActiveTranscriptSession(),
  clearTranscriptSegments: () => get().clearActiveTranscriptSession(),
  syncSavedRecordingMeta: (title, historyId, icon, audioUrl, originSessionId) => {
    const state = get();
    const targetSessionId =
      originSessionId && state.sessions[originSessionId] ? originSessionId : historyId;
    const session = state.sessions[targetSessionId];
    if (!session) return;

    set({
      sessions: {
        ...state.sessions,
        [targetSessionId]: {
          ...session,
          sourceHistoryId: historyId,
          title,
          icon: icon || null,
          audioUrl: audioUrl !== undefined ? audioUrl : session.audioUrl,
        },
      },
    });
    get().rekeySessionSidecar(targetSessionId, historyId);
  },

  // --- Session Data Mutations ---
  setSourceHistoryIdForSession: (sessionId, sourceHistoryId) =>
    updateSession(set, sessionId, () => ({ sourceHistoryId })),
  setSourceHistoryId: (sourceHistoryId) =>
    get().setSourceHistoryIdForSession(get().activeSessionId, sourceHistoryId),
  setTitleForSession: (sessionId, title) =>
    updateSession(set, sessionId, () => ({ title: title || '' })),
  setTitle: (title) => get().setTitleForSession(get().activeSessionId, title),
  setIconForSession: (sessionId, icon) => updateSession(set, sessionId, () => ({ icon })),
  setIcon: (icon) => get().setIconForSession(get().activeSessionId, icon),
  setSessionProjectContext: (sessionId, projectId, pipelineSnapshot) =>
    updateSession(set, sessionId, (session) => ({
      projectId,
      ...(pipelineSnapshot !== undefined
        ? { pipelineSnapshot }
        : { pipelineSnapshot: session.pipelineSnapshot }),
    })),
  setSegments: (segments) => get().setSegmentsForSession(get().activeSessionId, segments),
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
  addSegmentForSession: (sessionId, segment) => {
    const id = uuidv4();
    const newSegment = normalizeTranscriptSegment({ ...segment, id });
    updateSession(set, sessionId, (session) => ({
      segments: [...session.segments, newSegment].sort((a, b) => a.start - b.start),
    }));
    return id;
  },
  addSegment: (segment) => get().addSegmentForSession(get().activeSessionId, segment),
  upsertSegmentForSession: (sessionId, segment) => {
    updateSession(set, sessionId, (session) => ({
      segments: calculateSegmentUpdate(session.segments, normalizeTranscriptSegment(segment))
        .segments,
    }));
  },
  upsertSegment: (segment) => get().upsertSegmentForSession(get().activeSessionId, segment),
  updateSegment: (id, updates) => get().updateSegmentForSession(get().activeSessionId, id, updates),
  patchSegmentTranslationForSession: (sessionId, id, translation) => {
    get().patchSegmentTranslationsForSession(sessionId, [{ id, translation }]);
  },
  patchSegmentTranslation: (id, translation) => {
    get().patchSegmentTranslations([{ id, translation }]);
  },
  patchSegmentTranslations: (updates) => {
    get().patchSegmentTranslationsForSession(get().activeSessionId, updates);
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
  deleteSegmentForSession: (sessionId, id) =>
    updateSession(set, sessionId, (session) => ({
      segments: session.segments.filter((s) => s.id !== id),
      editingSegmentId: session.editingSegmentId === id ? null : session.editingSegmentId,
      activeSegmentId: session.activeSegmentId === id ? null : session.activeSegmentId,
    })),
  deleteSegment: (id) => get().deleteSegmentForSession(get().activeSessionId, id),
  mergeSegmentsForSession: (sessionId, id1, id2) => {
    updateSession(set, sessionId, (session) => {
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
  mergeSegments: (id1, id2) => get().mergeSegmentsForSession(get().activeSessionId, id1, id2),
  splitTranscriptSegmentForSession: (
    sessionId,
    id,
    leftText,
    rightText,
    leftTranslation,
    rightTranslation
  ) => {
    const state = get();
    const session = state.sessions[sessionId] || DEFAULT_SESSION_DATA;
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

    updateSession(set, sessionId, () => ({
      segments: nextSegments,
      editingSegmentId: newSegmentId,
    }));
    return newSegmentId;
  },
  splitTranscriptSegment: (id, leftText, rightText, leftTranslation, rightTranslation) =>
    get().splitTranscriptSegmentForSession(
      get().activeSessionId,
      id,
      leftText,
      rightText,
      leftTranslation,
      rightTranslation
    ),
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
  upsertTranscriptSegmentAndSetActiveForSession: (sessionId, segment) =>
    updateSession(set, sessionId, (session) => {
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
  upsertTranscriptSegmentAndSetActive: (segment) =>
    get().upsertTranscriptSegmentAndSetActiveForSession(get().activeSessionId, segment),
  setEditingSegmentIdForSession: (sessionId, id) =>
    updateSession(set, sessionId, () => ({ editingSegmentId: id })),
  setEditingSegmentId: (id) => get().setEditingSegmentIdForSession(get().activeSessionId, id),
  addAligningSegmentIdForSession: (sessionId, id) =>
    updateSession(set, sessionId, (session) => {
      const next = new Set(session.aligningSegmentIds);
      next.add(id);
      return { aligningSegmentIds: next };
    }),
  addAligningSegmentId: (id) => get().addAligningSegmentIdForSession(get().activeSessionId, id),
  removeAligningSegmentIdForSession: (sessionId, id) =>
    updateSession(set, sessionId, (session) => {
      const next = new Set(session.aligningSegmentIds);
      next.delete(id);
      return { aligningSegmentIds: next };
    }),
  removeAligningSegmentId: (id) =>
    get().removeAligningSegmentIdForSession(get().activeSessionId, id),

  // --- Playback Mutations ---
  setAudioFileForSession: (sessionId, file) =>
    updateSession(set, sessionId, (session) => {
      revokeBlobAudioUrl(session.audioUrl);
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
  setAudioFile: (file) => get().setAudioFileForSession(get().activeSessionId, file),
  setAudioUrlForSession: (sessionId, audioUrl) =>
    updateSession(set, sessionId, () => ({
      audioUrl,
      isPlaying: false,
      currentTime: 0,
      activeSegmentId: null,
      activeSegmentIndex: -1,
      seekRequest: null,
      lastSeekTimestamp: 0,
    })),
  setAudioUrl: (audioUrl) => get().setAudioUrlForSession(get().activeSessionId, audioUrl),
  setCurrentTimeForSession: (sessionId, time) =>
    updateSession(set, sessionId, (session) => {
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
  setCurrentTime: (time) => get().setCurrentTimeForSession(get().activeSessionId, time),
  setIsPlayingForSession: (sessionId, isPlaying) =>
    updateSession(set, sessionId, () => ({ isPlaying })),
  setIsPlaying: (isPlaying) => get().setIsPlayingForSession(get().activeSessionId, isPlaying),
  setActiveSegmentIdForSession: (sessionId, activeSegmentId, activeSegmentIndex = -1) =>
    updateSession(set, sessionId, () => ({ activeSegmentId, activeSegmentIndex })),
  setActiveSegmentId: (activeSegmentId, activeSegmentIndex = -1) =>
    get().setActiveSegmentIdForSession(get().activeSessionId, activeSegmentId, activeSegmentIndex),
  resetActiveSegmentIndexForSession: (sessionId) =>
    updateSession(set, sessionId, () => ({ activeSegmentIndex: -1 })),
  resetActiveSegmentIndex: () => get().resetActiveSegmentIndexForSession(get().activeSessionId),
  requestSeekForSession: (sessionId, time) => {
    get().setCurrentTimeForSession(sessionId, time);
    const timestamp = Date.now();
    updateSession(set, sessionId, () => ({
      seekRequest: { time, timestamp },
      lastSeekTimestamp: timestamp,
    }));
  },
  requestSeek: (time) => get().requestSeekForSession(get().activeSessionId, time),

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
      summaryStates: rekeyCurrentSummaryStateEntry(
        state.summaryStates,
        nextHistoryId,
        state.activeSessionId
      ),
    })),
  rekeySessionSidecar: (sessionId, nextHistoryId) => {
    if (!nextHistoryId || nextHistoryId === 'current') return;
    documentMutationCoordinator.rekeyAlias(sessionId, nextHistoryId);
    set((state) => {
      const rekeyed = rekeySessionSidecarEntries(
        {
          llmStates: state.llmStates,
          summaryStates: state.summaryStates,
          autoSaveStates: state.autoSaveStates,
        },
        sessionId,
        nextHistoryId
      );
      if (
        rekeyed.llmStates === state.llmStates &&
        rekeyed.summaryStates === state.summaryStates &&
        rekeyed.autoSaveStates === state.autoSaveStates
      ) {
        return state;
      }
      return rekeyed;
    });
  },
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
