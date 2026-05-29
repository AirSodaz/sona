import { useTranscriptStore, type TranscriptStore, type SessionData } from './transcriptStore';

type PlaybackStoreActions = Pick<TranscriptStore, 'setAudioFile' | 'setAudioUrl' | 'setCurrentTime' | 'setIsPlaying' | 'setActiveSegmentId' | 'resetActiveSegmentIndex' | 'requestSeek' | 'openSession' | 'clearActiveTranscriptSession'> & { clearSession: TranscriptStore['clearActiveTranscriptSession'] };

type PlaybackStoreState = SessionData & PlaybackStoreActions;

let cachedActions: PlaybackStoreActions | null = null;
function getActions(): PlaybackStoreActions {
  if (!cachedActions) {
    const state = useTranscriptStore.getState();
    cachedActions = {
      setAudioFile: state.setAudioFile,
      setAudioUrl: state.setAudioUrl,
      setCurrentTime: state.setCurrentTime,
      setIsPlaying: state.setIsPlaying,
      setActiveSegmentId: state.setActiveSegmentId,
      resetActiveSegmentIndex: state.resetActiveSegmentIndex,
      requestSeek: state.requestSeek,
      openSession: state.openSession,
      clearActiveTranscriptSession: state.clearActiveTranscriptSession,
      clearSession: state.clearActiveTranscriptSession,
    };
  }
  return cachedActions;
}

let cachedFacadeState: PlaybackStoreState | null = null;
let lastSessionRef: SessionData | null = null;

function getFacadeState(state: TranscriptStore): PlaybackStoreState {
  const activeSession = state.sessions[state.activeSessionId] || ({} as SessionData);
  if (lastSessionRef === activeSession && cachedFacadeState) {
    return cachedFacadeState;
  }
  lastSessionRef = activeSession;
  cachedFacadeState = { ...activeSession, ...getActions() } as PlaybackStoreState;
  return cachedFacadeState;
}

export const useTranscriptPlaybackStore = Object.assign(
  <T>(selector: (state: PlaybackStoreState) => T) => {
    return useTranscriptStore((state) => selector(getFacadeState(state)));
  },
  {
    getState: () => getFacadeState(useTranscriptStore.getState()),
    setState: (updater: Partial<PlaybackStoreState> | ((state: PlaybackStoreState) => Partial<PlaybackStoreState>)) => {
      const currentFacadeState = useTranscriptPlaybackStore.getState();
      const updates = typeof updater === 'function' ? updater(currentFacadeState) : updater;

      useTranscriptStore.setState((s) => ({
        sessions: {
          ...s.sessions,
          [s.activeSessionId]: {
            ...(s.sessions[s.activeSessionId] || {}),
            ...updates
          }
        }
      }));
    },
    subscribe: (listener: (state: PlaybackStoreState, prevState: PlaybackStoreState) => void) => {
        let lastSessionId = useTranscriptStore.getState().activeSessionId;
        let lastActiveSession = useTranscriptStore.getState().sessions[lastSessionId];
        let lastFullState = useTranscriptPlaybackStore.getState();

        return useTranscriptStore.subscribe((state) => {
            const nextSessionId = state.activeSessionId;
            const nextActiveSession = state.sessions[nextSessionId];

            if (nextActiveSession !== lastActiveSession || nextSessionId !== lastSessionId) {
                const nextFullState = useTranscriptPlaybackStore.getState();
                listener(nextFullState, lastFullState);
                lastActiveSession = nextActiveSession;
                lastSessionId = nextSessionId;
                lastFullState = nextFullState;
            }
        });
    },
  }
);
