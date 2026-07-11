import { useTranscriptStore, type TranscriptStore, type SessionData } from './transcriptStore';

type SessionStoreActions = Pick<TranscriptStore, 'setSourceHistoryId' | 'setTitle' | 'setIcon' | 'setSegments' | 'addSegment' | 'upsertSegment' | 'updateSegment' | 'deleteSegment' | 'mergeSegments' | 'splitTranscriptSegment' | 'finalizeLastSegment' | 'applyTranscriptUpdate' | 'upsertTranscriptSegmentAndSetActive' | 'setEditingSegmentId' | 'addAligningSegmentId' | 'removeAligningSegmentId' | 'openSession' | 'loadTranscriptSession' | 'clearActiveTranscriptSession' | 'clearTranscriptSegments' | 'syncSavedRecordingMeta' | 'clearSegments'>;

type SessionStoreState = SessionData & SessionStoreActions;

let cachedActions: SessionStoreActions | null = null;
function getActions(): SessionStoreActions {
  if (!cachedActions) {
    const state = useTranscriptStore.getState();
    cachedActions = {
      setSourceHistoryId: state.setSourceHistoryId,
      setTitle: state.setTitle,
      setIcon: state.setIcon,
      setSegments: state.setSegments,
      addSegment: state.addSegment,
      upsertSegment: state.upsertSegment,
      updateSegment: state.updateSegment,
      deleteSegment: state.deleteSegment,
      mergeSegments: state.mergeSegments,
      splitTranscriptSegment: state.splitTranscriptSegment,
      finalizeLastSegment: state.finalizeLastSegment,
      applyTranscriptUpdate: state.applyTranscriptUpdate,
      upsertTranscriptSegmentAndSetActive: state.upsertTranscriptSegmentAndSetActive,
      setEditingSegmentId: state.setEditingSegmentId,
      addAligningSegmentId: state.addAligningSegmentId,
      removeAligningSegmentId: state.removeAligningSegmentId,
      openSession: state.openSession,
      loadTranscriptSession: state.loadTranscriptSession,
      clearActiveTranscriptSession: state.clearActiveTranscriptSession,
      clearTranscriptSegments: state.clearTranscriptSegments,
      syncSavedRecordingMeta: state.syncSavedRecordingMeta,
      clearSegments: state.clearSegments,
    };
  }
  return cachedActions;
}

let cachedFacadeState: SessionStoreState | null = null;
let lastSessionRef: SessionData | null = null;

function getFacadeState(state: TranscriptStore): SessionStoreState {
  const activeSession = state.sessions[state.activeSessionId] || ({} as SessionData);
  if (lastSessionRef === activeSession && cachedFacadeState) {
    return cachedFacadeState;
  }
  lastSessionRef = activeSession;
  cachedFacadeState = { ...activeSession, ...getActions() } as SessionStoreState;
  return cachedFacadeState;
}

export const useTranscriptSessionStore = Object.assign(
  <T>(selector: (state: SessionStoreState) => T) => {
    return useTranscriptStore((state) => selector(getFacadeState(state)));
  },
  {
    getState: () => getFacadeState(useTranscriptStore.getState()),
    setState: (updater: Partial<SessionStoreState> | ((state: SessionStoreState) => Partial<SessionStoreState>)) => {
      const currentFacadeState = useTranscriptSessionStore.getState();
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
    subscribe: (listener: (state: SessionStoreState, prevState: SessionStoreState) => void) => {
        let lastSessionId = useTranscriptStore.getState().activeSessionId;
        let lastActiveSession = useTranscriptStore.getState().sessions[lastSessionId];
        let lastFullState = useTranscriptSessionStore.getState();

        return useTranscriptStore.subscribe((state) => {
            const nextSessionId = state.activeSessionId;
            const nextActiveSession = state.sessions[nextSessionId];

            if (nextActiveSession !== lastActiveSession || nextSessionId !== lastSessionId) {
                const nextFullState = useTranscriptSessionStore.getState();
                listener(nextFullState, lastFullState);
                lastActiveSession = nextActiveSession;
                lastSessionId = nextSessionId;
                lastFullState = nextFullState;
            }
        });
    },
  }
);
