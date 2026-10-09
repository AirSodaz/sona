/**
 * @file transcriptSessionStore.ts
 * Active-session selector projection facade for standard UI components.
 *
 * Store boundary architecture:
 * - Projects the active session slice from `useTranscriptStore` with stable selectors.
 * - Exposes active session actions (`sessionActions`) bound to the active session.
 * - UI components should prefer this store over subscribing to multi-session state directly.
 */
import {
  DEFAULT_SESSION_DATA,
  type SessionData,
  type TranscriptStore,
  useTranscriptStore,
} from './transcriptStore';

export type SessionStoreActions = Pick<
  TranscriptStore,
  | 'setSourceHistoryId'
  | 'setTitle'
  | 'setIcon'
  | 'setSegments'
  | 'addSegment'
  | 'upsertSegment'
  | 'updateSegment'
  | 'deleteSegment'
  | 'mergeSegments'
  | 'splitTranscriptSegment'
  | 'finalizeLastSegment'
  | 'applyTranscriptUpdate'
  | 'upsertTranscriptSegmentAndSetActive'
  | 'setEditingSegmentId'
  | 'addAligningSegmentId'
  | 'removeAligningSegmentId'
  | 'openSession'
  | 'loadTranscriptSession'
  | 'clearActiveTranscriptSession'
  | 'clearTranscriptSegments'
  | 'syncSavedRecordingMeta'
  | 'clearSegments'
>;

export type SessionStoreState = SessionData & SessionStoreActions;

export const sessionActions: SessionStoreActions = {
  setSourceHistoryId: (id) => useTranscriptStore.getState().setSourceHistoryId(id),
  setTitle: (title) => useTranscriptStore.getState().setTitle(title),
  setIcon: (icon) => useTranscriptStore.getState().setIcon(icon),
  setSegments: (segments) => useTranscriptStore.getState().setSegments(segments),
  addSegment: (segment) => useTranscriptStore.getState().addSegment(segment),
  upsertSegment: (segment) => useTranscriptStore.getState().upsertSegment(segment),
  updateSegment: (id, updates) => useTranscriptStore.getState().updateSegment(id, updates),
  deleteSegment: (id) => useTranscriptStore.getState().deleteSegment(id),
  mergeSegments: (id1, id2) => useTranscriptStore.getState().mergeSegments(id1, id2),
  splitTranscriptSegment: (id, leftText, rightText) =>
    useTranscriptStore.getState().splitTranscriptSegment(id, leftText, rightText),
  finalizeLastSegment: () => useTranscriptStore.getState().finalizeLastSegment(),
  applyTranscriptUpdate: (update, activeSegmentId) =>
    useTranscriptStore.getState().applyTranscriptUpdate(update, activeSegmentId),
  upsertTranscriptSegmentAndSetActive: (segment) =>
    useTranscriptStore.getState().upsertTranscriptSegmentAndSetActive(segment),
  setEditingSegmentId: (id) => useTranscriptStore.getState().setEditingSegmentId(id),
  addAligningSegmentId: (id) => useTranscriptStore.getState().addAligningSegmentId(id),
  removeAligningSegmentId: (id) => useTranscriptStore.getState().removeAligningSegmentId(id),
  openSession: (args) => useTranscriptStore.getState().openSession(args),
  loadTranscriptSession: (segments, sourceHistoryId, title, icon) =>
    useTranscriptStore.getState().loadTranscriptSession(segments, sourceHistoryId, title, icon),
  clearActiveTranscriptSession: (options) =>
    useTranscriptStore.getState().clearActiveTranscriptSession(options),
  clearTranscriptSegments: () => useTranscriptStore.getState().clearTranscriptSegments(),
  syncSavedRecordingMeta: (title, historyId, icon, audioUrl) =>
    useTranscriptStore.getState().syncSavedRecordingMeta(title, historyId, icon, audioUrl),
  clearSegments: () => useTranscriptStore.getState().clearSegments(),
};

export const {
  setSourceHistoryId,
  setTitle,
  setIcon,
  setSegments,
  addSegment,
  upsertSegment,
  updateSegment,
  deleteSegment,
  mergeSegments,
  splitTranscriptSegment,
  finalizeLastSegment,
  applyTranscriptUpdate,
  upsertTranscriptSegmentAndSetActive,
  setEditingSegmentId,
  addAligningSegmentId,
  removeAligningSegmentId,
  openSession,
  loadTranscriptSession,
  clearActiveTranscriptSession,
  clearTranscriptSegments,
  syncSavedRecordingMeta,
  clearSegments,
} = sessionActions;

export function hasSessionContentChanged(a: SessionData, b: SessionData): boolean {
  return (
    a.segments !== b.segments ||
    a.sourceHistoryId !== b.sourceHistoryId ||
    a.title !== b.title ||
    a.icon !== b.icon ||
    a.editingSegmentId !== b.editingSegmentId ||
    a.aligningSegmentIds !== b.aligningSegmentIds
  );
}

let cachedSession: {
  sessionId: string;
  session: SessionData;
  state: SessionStoreState;
} | null = null;

export function resetSessionStoreCache(): void {
  cachedSession = null;
}

export function getSessionStoreState(state: TranscriptStore): SessionStoreState {
  const activeSession = state.sessions[state.activeSessionId] || DEFAULT_SESSION_DATA;
  if (
    cachedSession &&
    cachedSession.sessionId === state.activeSessionId &&
    !hasSessionContentChanged(cachedSession.session, activeSession)
  ) {
    Object.assign(cachedSession.state, activeSession);
    cachedSession.session = activeSession;
    return cachedSession.state;
  }
  const nextState: SessionStoreState = Object.assign(Object.create(sessionActions), activeSession);
  cachedSession = {
    sessionId: state.activeSessionId,
    session: activeSession,
    state: nextState,
  };
  return nextState;
}

export const transcriptSessionStore = {
  getState: (): SessionStoreState => getSessionStoreState(useTranscriptStore.getState()),
  setState: (
    updater: Partial<SessionStoreState> | ((state: SessionStoreState) => Partial<SessionStoreState>)
  ) => {
    const currentState = transcriptSessionStore.getState();
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
  subscribe: (listener: (state: SessionStoreState, prevState: SessionStoreState) => void) => {
    let lastSessionId = useTranscriptStore.getState().activeSessionId;
    let lastActiveSession =
      useTranscriptStore.getState().sessions[lastSessionId] || DEFAULT_SESSION_DATA;
    let lastFullState = transcriptSessionStore.getState();

    return useTranscriptStore.subscribe((state) => {
      const nextSessionId = state.activeSessionId;
      const nextActiveSession = state.sessions[nextSessionId] || DEFAULT_SESSION_DATA;

      if (
        nextSessionId !== lastSessionId ||
        hasSessionContentChanged(lastActiveSession, nextActiveSession)
      ) {
        const nextFullState = getSessionStoreState(state);
        const prevFullState = lastFullState;
        lastActiveSession = nextActiveSession;
        lastSessionId = nextSessionId;
        lastFullState = nextFullState;
        listener(nextFullState, prevFullState);
      }
    });
  },
};

export const useTranscriptSessionStore = Object.assign(
  <T>(selector: (state: SessionStoreState) => T) => {
    return useTranscriptStore((state) => selector(getSessionStoreState(state)));
  },
  transcriptSessionStore
);
