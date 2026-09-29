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

export function getPlaybackStoreState(state: TranscriptStore): PlaybackStoreState {
  const activeSession = state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
  return Object.assign(Object.create(playbackActions), activeSession);
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
        const nextFullState = getPlaybackStoreState(state);
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
    return useTranscriptStore((state) => selector(getPlaybackStoreState(state)));
  },
  transcriptPlaybackStore
);
