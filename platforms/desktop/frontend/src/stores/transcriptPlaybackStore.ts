import { useBoundSessionId } from '../contexts/TranscriptSessionContext';
import {
  DEFAULT_SESSION_DATA,
  type SessionData,
  type TranscriptStore,
  useTranscriptStore,
} from './transcriptStore';

export type PlaybackStoreActions = Pick<
  TranscriptStore,
  | 'setAudioFile'
  | 'setAudioUrl'
  | 'setCurrentTime'
  | 'setIsPlaying'
  | 'setActiveSegmentId'
  | 'resetActiveSegmentIndex'
  | 'requestSeek'
  | 'openSession'
  | 'clearActiveTranscriptSession'
> & { clearSession: TranscriptStore['clearActiveTranscriptSession'] };

export type PlaybackStoreState = SessionData & PlaybackStoreActions;

function createPlaybackActionsForSession(sessionId: string): PlaybackStoreActions {
  return {
    setAudioFile: (file) => useTranscriptStore.getState().setAudioFileForSession(sessionId, file),
    setAudioUrl: (url) => useTranscriptStore.getState().setAudioUrlForSession(sessionId, url),
    setCurrentTime: (time) =>
      useTranscriptStore.getState().setCurrentTimeForSession(sessionId, time),
    setIsPlaying: (isPlaying) =>
      useTranscriptStore.getState().setIsPlayingForSession(sessionId, isPlaying),
    setActiveSegmentId: (id, index) =>
      useTranscriptStore.getState().setActiveSegmentIdForSession(sessionId, id, index),
    resetActiveSegmentIndex: () =>
      useTranscriptStore.getState().resetActiveSegmentIndexForSession(sessionId),
    requestSeek: (time) => useTranscriptStore.getState().requestSeekForSession(sessionId, time),
    openSession: (args) => useTranscriptStore.getState().openSession(args),
    clearActiveTranscriptSession: (options) =>
      useTranscriptStore.getState().clearTranscriptSession(sessionId, options),
    clearSession: (options) =>
      useTranscriptStore.getState().clearTranscriptSession(sessionId, options),
  };
}

const playbackActionsById = new Map<string, PlaybackStoreActions>();

export function getPlaybackActionsFor(sessionId: string): PlaybackStoreActions {
  let cached = playbackActionsById.get(sessionId);
  if (!cached) {
    cached = createPlaybackActionsForSession(sessionId);
    playbackActionsById.set(sessionId, cached);
  }
  return cached;
}

export const playbackActions: PlaybackStoreActions = {
  setAudioFile: (file) => useTranscriptStore.getState().setAudioFile(file),
  setAudioUrl: (url) => useTranscriptStore.getState().setAudioUrl(url),
  setCurrentTime: (time) => useTranscriptStore.getState().setCurrentTime(time),
  setIsPlaying: (isPlaying) => useTranscriptStore.getState().setIsPlaying(isPlaying),
  setActiveSegmentId: (id, index) => useTranscriptStore.getState().setActiveSegmentId(id, index),
  resetActiveSegmentIndex: () => useTranscriptStore.getState().resetActiveSegmentIndex(),
  requestSeek: (time) => useTranscriptStore.getState().requestSeek(time),
  openSession: (args) => useTranscriptStore.getState().openSession(args),
  clearActiveTranscriptSession: (options) =>
    useTranscriptStore.getState().clearActiveTranscriptSession(options),
  clearSession: (options) => useTranscriptStore.getState().clearActiveTranscriptSession(options),
};

export function getPlaybackStoreState(
  state: TranscriptStore,
  sessionId = state.activeSessionId
): PlaybackStoreState {
  const session = state.sessions[sessionId] || DEFAULT_SESSION_DATA;
  return Object.assign(Object.create(getPlaybackActionsFor(sessionId)), session);
}

export const transcriptPlaybackStore = {
  getState: (): PlaybackStoreState => getPlaybackStoreState(useTranscriptStore.getState()),
  setState: (
    updater:
      | Partial<PlaybackStoreState>
      | ((state: PlaybackStoreState) => Partial<PlaybackStoreState>)
  ) => {
    const currentState = transcriptPlaybackStore.getState();
    const updates = typeof updater === 'function' ? updater(currentState) : updater;

    useTranscriptStore.setState((s) => ({
      sessions: {
        ...s.sessions,
        [s.activeSessionId]: {
          ...(s.sessions[s.activeSessionId] || DEFAULT_SESSION_DATA),
          ...updates,
        },
      },
    }));
  },
  subscribe: (listener: (state: PlaybackStoreState, prevState: PlaybackStoreState) => void) => {
    let lastSessionId = useTranscriptStore.getState().activeSessionId;
    let lastActiveSession = useTranscriptStore.getState().sessions[lastSessionId];
    let lastFullState = transcriptPlaybackStore.getState();

    return useTranscriptStore.subscribe((state) => {
      const nextSessionId = state.activeSessionId;
      const nextActiveSession = state.sessions[nextSessionId];

      if (nextActiveSession !== lastActiveSession || nextSessionId !== lastSessionId) {
        const nextFullState = getPlaybackStoreState(state, nextSessionId);
        const prevFullState = lastFullState;
        lastActiveSession = nextActiveSession;
        lastSessionId = nextSessionId;
        lastFullState = nextFullState;
        listener(nextFullState, prevFullState);
      }
    });
  },
};

export const useTranscriptPlaybackStore = Object.assign(
  <T>(selector: (state: PlaybackStoreState) => T) => {
    const boundSessionId = useBoundSessionId();
    return useTranscriptStore((state) => selector(getPlaybackStoreState(state, boundSessionId)));
  },
  transcriptPlaybackStore
);
