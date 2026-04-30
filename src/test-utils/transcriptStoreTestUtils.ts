import type { AppConfig } from '../types/config';
import type {
  AppMode,
  HistorySummaryPayload,
  ProcessingStatus,
  SummaryTemplateId,
  TranscriptSegment,
  TranscriptUpdate,
} from '../types/transcript';
import type { LlmState } from '../stores/transcriptSidecarStore';
import { DEFAULT_CONFIG, useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import {
  INITIAL_TRANSCRIPT_PLAYBACK_STATE,
  INITIAL_TRANSCRIPT_SESSION_STATE,
} from '../stores/transcriptSessionState';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import {
  DEFAULT_LLM_STATE,
  INITIAL_TRANSCRIPT_HISTORY_SIDECAR_STATE,
} from '../stores/transcriptSidecarState';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import {
  applyTranscriptUpdate,
  clearActiveTranscriptSession,
  finalizeLastTranscriptSegment,
  loadTranscriptSession,
  mergeTranscriptSegments,
  openTranscriptSession,
  setTranscriptSegments,
  updateTranscriptSegment,
  upsertTranscriptSegmentAndSetActive,
} from '../stores/transcriptCoordinator';

type LegacyTranscriptState = ReturnType<typeof getTranscriptTestState>;
type TranscriptStateSelector<T> = (state: LegacyTranscriptState) => T;
type TranscriptStatePatch =
  | Partial<LegacyTranscriptState>
  | ((state: LegacyTranscriptState) => Partial<LegacyTranscriptState>);

type TranscriptStoreHook = {
  <T>(selector: TranscriptStateSelector<T>): T;
  (): LegacyTranscriptState;
  getState: () => LegacyTranscriptState;
  setState: (patch: TranscriptStatePatch) => void;
  subscribe: (
    listener: (state: LegacyTranscriptState, previousState: LegacyTranscriptState) => void,
  ) => () => void;
};

interface SessionPatch {
  segments?: TranscriptSegment[];
  editingSegmentId?: string | null;
  aligningSegmentIds?: Set<string>;
  sourceHistoryId?: string | null;
  title?: string | null;
  icon?: string | null;
}

interface PlaybackPatch {
  activeSegmentId?: string | null;
  activeSegmentIndex?: number;
  audioFile?: File | null;
  audioUrl?: string | null;
  currentTime?: number;
  isPlaying?: boolean;
  lastSeekTimestamp?: number;
  seekRequest?: { time: number; timestamp: number } | null;
}

interface RuntimePatch {
  mode?: AppMode;
  processingStatus?: ProcessingStatus;
  processingProgress?: number;
  isRecording?: boolean;
  isCaptionMode?: boolean;
  isPaused?: boolean;
}

interface SidecarPatch {
  llmStates?: Record<string, LlmState>;
  summaryStates?: ReturnType<typeof useTranscriptSidecarStore.getState>['summaryStates'];
  autoSaveStates?: ReturnType<typeof useTranscriptSidecarStore.getState>['autoSaveStates'];
}

const SESSION_KEYS = new Set<keyof SessionPatch>([
  'segments',
  'editingSegmentId',
  'aligningSegmentIds',
  'sourceHistoryId',
  'title',
  'icon',
]);

const PLAYBACK_KEYS = new Set<keyof PlaybackPatch>([
  'activeSegmentId',
  'activeSegmentIndex',
  'audioFile',
  'audioUrl',
  'currentTime',
  'isPlaying',
  'lastSeekTimestamp',
  'seekRequest',
]);

const RUNTIME_KEYS = new Set<keyof RuntimePatch>([
  'mode',
  'processingStatus',
  'processingProgress',
  'isRecording',
  'isCaptionMode',
  'isPaused',
]);

const SIDECAR_KEYS = new Set<keyof SidecarPatch>([
  'llmStates',
  'summaryStates',
  'autoSaveStates',
]);

function syncEffectiveConfig(): void {
  useEffectiveConfigStore.getState().syncConfig();
}

function applySessionPatch(patch: SessionPatch): void {
  if (Object.keys(patch).length === 0) {
    return;
  }

  useTranscriptSessionStore.setState((state) => ({
    ...state,
    ...patch,
  }));
}

function applyPlaybackPatch(patch: PlaybackPatch): void {
  if (Object.keys(patch).length === 0) {
    return;
  }

  useTranscriptPlaybackStore.setState((state) => ({
    ...state,
    ...patch,
  }));
}

function applyRuntimePatch(patch: RuntimePatch): void {
  if (Object.keys(patch).length === 0) {
    return;
  }

  useTranscriptRuntimeStore.setState((state) => ({
    ...state,
    ...patch,
  }));
}

function applySidecarPatch(patch: SidecarPatch): void {
  if (Object.keys(patch).length === 0) {
    return;
  }

  useTranscriptSidecarStore.setState((state) => ({
    ...state,
    ...patch,
  }));
}

function applyTranscriptStatePatch(patch: Partial<LegacyTranscriptState>): void {
  const sessionPatch: SessionPatch = {};
  const playbackPatch: PlaybackPatch = {};
  const runtimePatch: RuntimePatch = {};
  const sidecarPatch: SidecarPatch = {};

  Object.entries(patch).forEach(([rawKey, value]) => {
    if (rawKey === 'config') {
      useConfigStore.setState((state) => ({
        ...state,
        config: value as AppConfig,
      }));
      useEffectiveConfigStore.setState((state) => ({
        ...state,
        config: value as AppConfig,
      }));
      return;
    }

    if (SESSION_KEYS.has(rawKey as keyof SessionPatch)) {
      sessionPatch[rawKey as keyof SessionPatch] = value as never;
      return;
    }

    if (PLAYBACK_KEYS.has(rawKey as keyof PlaybackPatch)) {
      playbackPatch[rawKey as keyof PlaybackPatch] = value as never;
      return;
    }

    if (RUNTIME_KEYS.has(rawKey as keyof RuntimePatch)) {
      runtimePatch[rawKey as keyof RuntimePatch] = value as never;
      return;
    }

    if (SIDECAR_KEYS.has(rawKey as keyof SidecarPatch)) {
      sidecarPatch[rawKey as keyof SidecarPatch] = value as never;
    }
  });

  applySessionPatch(sessionPatch);
  applyPlaybackPatch(playbackPatch);
  applyRuntimePatch(runtimePatch);
  applySidecarPatch(sidecarPatch);
}

export function resetTranscriptStores(): void {
  useConfigStore.setState((state) => ({
    ...state,
    config: { ...DEFAULT_CONFIG },
  }));
  useProjectStore.setState((state) => ({
    ...state,
    projects: [],
    activeProjectId: null,
    isLoading: false,
    error: null,
  }));
  useTranscriptSessionStore.setState((state) => ({
    ...state,
    ...INITIAL_TRANSCRIPT_SESSION_STATE,
    aligningSegmentIds: new Set<string>(),
  }));
  useTranscriptPlaybackStore.setState((state) => ({
    ...state,
    ...INITIAL_TRANSCRIPT_PLAYBACK_STATE,
  }));
  useTranscriptRuntimeStore.setState((state) => ({
    ...state,
    mode: 'live',
    processingStatus: 'idle',
    processingProgress: 0,
    isRecording: false,
    isCaptionMode: false,
    isPaused: false,
  }));
  useTranscriptSidecarStore.setState((state) => ({
    ...state,
    ...INITIAL_TRANSCRIPT_HISTORY_SIDECAR_STATE,
  }));
  syncEffectiveConfig();
}

export function setTranscriptTestState(patch: Partial<LegacyTranscriptState>): void {
  applyTranscriptStatePatch(patch);
}

export function getTranscriptTestState() {
  const sessionStore = useTranscriptSessionStore.getState();
  const playbackStore = useTranscriptPlaybackStore.getState();
  const runtimeStore = useTranscriptRuntimeStore.getState();
  const sidecarStore = useTranscriptSidecarStore.getState();
  const config = useEffectiveConfigStore.getState().config;

  return {
    ...sessionStore,
    ...playbackStore,
    ...runtimeStore,
    ...sidecarStore,
    config,
    setSourceHistoryId: (id: string | null) => {
      useTranscriptSessionStore.getState().setSourceHistoryId(id);
      useTranscriptSidecarStore.getState().rekeyCurrentSummaryState(id);
    },
    setTitle: useTranscriptSessionStore.getState().setTitle,
    setIcon: useTranscriptSessionStore.getState().setIcon,
    addSegment: useTranscriptSessionStore.getState().addSegment,
    upsertSegment: useTranscriptSessionStore.getState().upsertSegment,
    upsertSegmentAndSetActive: (segment: TranscriptSegment) => {
      upsertTranscriptSegmentAndSetActive(segment);
    },
    applyTranscriptUpdate: (update: TranscriptUpdate, activeSegmentId?: string | null) => {
      applyTranscriptUpdate(update, activeSegmentId);
    },
    updateSegment: (id: string, updates: Partial<Omit<TranscriptSegment, 'id'>>) => {
      updateTranscriptSegment(id, updates);
    },
    deleteSegment: useTranscriptSessionStore.getState().deleteSegment,
    mergeSegments: (id1: string, id2: string) => {
      mergeTranscriptSegments(id1, id2);
    },
    setSegments: (segments: TranscriptSegment[]) => {
      setTranscriptSegments(segments);
    },
    loadTranscript: (
      segments: TranscriptSegment[],
      sourceHistoryId: string | null,
      title?: string | null,
      icon?: string | null,
    ) => {
      loadTranscriptSession(segments, sourceHistoryId, title, icon);
    },
    openTranscriptSession: (session: {
      segments: TranscriptSegment[];
      sourceHistoryId: string | null;
      title?: string | null;
      icon?: string | null;
      audioUrl?: string | null;
    }) => {
      openTranscriptSession(session);
    },
    clearActiveTranscriptSession: (options?: { clearAudio?: boolean; title?: string | null }) => {
      clearActiveTranscriptSession(options);
    },
    finalizeLastSegment: () => {
      finalizeLastTranscriptSegment();
    },
    clearSegments: () => {
      clearActiveTranscriptSession();
    },
    setActiveSegmentId: useTranscriptPlaybackStore.getState().setActiveSegmentId,
    setEditingSegmentId: useTranscriptSessionStore.getState().setEditingSegmentId,
    setMode: useTranscriptRuntimeStore.getState().setMode,
    setProcessingStatus: useTranscriptRuntimeStore.getState().setProcessingStatus,
    setProcessingProgress: useTranscriptRuntimeStore.getState().setProcessingProgress,
    addAligningSegmentId: useTranscriptSessionStore.getState().addAligningSegmentId,
    removeAligningSegmentId: useTranscriptSessionStore.getState().removeAligningSegmentId,
    getLlmState: useTranscriptSidecarStore.getState().getLlmState,
    updateLlmState: useTranscriptSidecarStore.getState().updateLlmState,
    getSummaryState: useTranscriptSidecarStore.getState().getSummaryState,
    setSummaryState: useTranscriptSidecarStore.getState().setSummaryState,
    updateSummaryState: useTranscriptSidecarStore.getState().updateSummaryState,
    setActiveSummaryTemplate: useTranscriptSidecarStore.getState().setActiveSummaryTemplate,
    hydrateSummaryState: (payload: HistorySummaryPayload, historyId?: string) => {
      useTranscriptSidecarStore.getState().hydrateSummaryState(payload, historyId);
    },
    clearSummaryState: useTranscriptSidecarStore.getState().clearSummaryState,
    setAutoSaveState: useTranscriptSidecarStore.getState().setAutoSaveState,
    clearAutoSaveState: useTranscriptSidecarStore.getState().clearAutoSaveState,
    setIsTranslationVisible: (visible: boolean) => {
      useTranscriptSidecarStore.getState().updateLlmState({ isTranslationVisible: visible });
    },
    setIsTranslating: (translating: boolean) => {
      useTranscriptSidecarStore.getState().updateLlmState({ isTranslating: translating });
    },
    setTranslationProgress: (progress: number) => {
      useTranscriptSidecarStore.getState().updateLlmState({ translationProgress: progress });
    },
    setIsPolishing: (polishing: boolean) => {
      useTranscriptSidecarStore.getState().updateLlmState({ isPolishing: polishing });
    },
    setPolishProgress: (progress: number) => {
      useTranscriptSidecarStore.getState().updateLlmState({ polishProgress: progress });
    },
    setAudioFile: useTranscriptPlaybackStore.getState().setAudioFile,
    setAudioUrl: useTranscriptPlaybackStore.getState().setAudioUrl,
    setCurrentTime: useTranscriptPlaybackStore.getState().setCurrentTime,
    setIsPlaying: useTranscriptPlaybackStore.getState().setIsPlaying,
    setIsRecording: useTranscriptRuntimeStore.getState().setIsRecording,
    setIsCaptionMode: useTranscriptRuntimeStore.getState().setIsCaptionMode,
    setIsPaused: useTranscriptRuntimeStore.getState().setIsPaused,
    requestSeek: useTranscriptPlaybackStore.getState().requestSeek,
    setConfig: (patch: Partial<AppConfig>) => {
      useConfigStore.getState().setConfig(patch);
      syncEffectiveConfig();
    },
    setSummaryTemplate: (templateId: SummaryTemplateId) => {
      useTranscriptSidecarStore.getState().setActiveSummaryTemplate(templateId);
    },
    defaultLlmState: { ...DEFAULT_LLM_STATE },
  };
}

export function syncTranscriptEffectiveConfig(): void {
  syncEffectiveConfig();
}

export const useTranscriptStore = ((selector?: TranscriptStateSelector<unknown>) => {
  const state = getTranscriptTestState();
  return selector ? selector(state) : state;
}) as TranscriptStoreHook;

useTranscriptStore.getState = getTranscriptTestState;
useTranscriptStore.setState = (patch) => {
  const partial = typeof patch === 'function'
    ? patch(getTranscriptTestState())
    : patch;
  applyTranscriptStatePatch(partial);
};
useTranscriptStore.subscribe = (listener) => {
  let previousState = getTranscriptTestState();
  const notify = () => {
    const nextState = getTranscriptTestState();
    listener(nextState, previousState);
    previousState = nextState;
  };

  const unlisteners = [
    useTranscriptSessionStore.subscribe(notify),
    useTranscriptPlaybackStore.subscribe(notify),
    useTranscriptRuntimeStore.subscribe(notify),
    useTranscriptSidecarStore.subscribe(notify),
    useEffectiveConfigStore.subscribe(notify),
  ];

  return () => {
    unlisteners.forEach((unlisten) => unlisten());
  };
};
