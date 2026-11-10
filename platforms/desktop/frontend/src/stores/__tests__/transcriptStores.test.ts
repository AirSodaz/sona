import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../types/transcript';
import { DEFAULT_CONFIG, useConfigStore } from '../configStore';
import { getEffectiveConfigSnapshot, useEffectiveConfigStore } from '../effectiveConfigStore';
import { useProjectStore } from '../projectStore';
import {
  applyTranscriptUpdate,
  clearActiveTranscriptSession,
  openTranscriptSession,
  syncSavedRecordingMeta,
  updateTranscriptSegmentForSession,
} from '../transcriptCoordinator';
import { useTranscriptPlaybackStore } from '../transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../transcriptRuntimeStore';
import { getSessionStoreState, useTranscriptSessionStore } from '../transcriptSessionStore';
import { useTranscriptSidecarStore } from '../transcriptSidecarStore';
import {
  DEFAULT_SESSION_DATA,
  registerBatchActiveItemIdResolver,
  useTranscriptStore as useCanonicalTranscriptStore,
} from '../transcriptStore';

vi.mock('../../services/tauri/app', () => ({
  resolveEffectiveConfig: vi.fn(async (globalConfig: any) => globalConfig),
}));

describe('Transcript Stores', () => {
  beforeEach(() => {
    resetTranscriptStores();
  });

  it('session store manages segment CRUD and clears edit state on delete', () => {
    const sessionStore = useTranscriptSessionStore.getState();
    const firstId = sessionStore.addSegment({
      text: 'First',
      start: 0,
      end: 1,
      isFinal: true,
    });
    const secondId = sessionStore.addSegment({
      text: 'Second',
      start: 1,
      end: 2,
      isFinal: false,
    });

    sessionStore.setEditingSegmentId(secondId);
    sessionStore.updateSegment(secondId, { text: 'Second updated' });
    sessionStore.deleteSegment(firstId);

    expect(useTranscriptSessionStore.getState().segments).toEqual([
      expect.objectContaining({
        id: secondId,
        text: 'Second updated',
      }),
    ]);
    expect(useTranscriptSessionStore.getState().editingSegmentId).toBe(secondId);

    sessionStore.deleteSegment(secondId);

    expect(useTranscriptSessionStore.getState().segments).toEqual([]);
    expect(useTranscriptSessionStore.getState().editingSegmentId).toBeNull();
  });

  it('opens and clears a transcript session, then syncs saved metadata back into session and sidecar state', () => {
    useTranscriptSidecarStore.getState().setSummaryState(
      {
        activeTemplateId: 'meeting',
        record: {
          templateId: 'meeting',
          content: 'Unsaved summary',
          generatedAt: '2026-04-30T00:00:00.000Z',
          sourceFingerprint: 'fp-1',
        },
      },
      'current'
    );

    openTranscriptSession({
      segments: [{ id: 'seg-1', text: 'Hello', start: 0, end: 1, isFinal: true }],
      sourceHistoryId: 'history-1',
      title: 'Draft',
      icon: 'system:mic',
      audioUrl: 'asset:///draft.wav',
    });

    expect(useTranscriptSessionStore.getState()).toEqual(
      expect.objectContaining({
        sourceHistoryId: 'history-1',
        title: 'Draft',
        icon: 'system:mic',
      })
    );
    expect(useTranscriptPlaybackStore.getState().audioUrl).toBe('asset:///draft.wav');

    syncSavedRecordingMeta('Saved title', 'history-1', 'system:file');

    expect(useTranscriptSessionStore.getState()).toEqual(
      expect.objectContaining({
        sourceHistoryId: 'history-1',
        title: 'Saved title',
        icon: 'system:file',
      })
    );
    expect(useTranscriptSidecarStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptSidecarStore.getState().getSummaryState('history-1').record?.content).toBe(
      'Unsaved summary'
    );

    clearActiveTranscriptSession({ clearAudio: true, title: 'Empty' });

    expect(useTranscriptSessionStore.getState()).toEqual(
      expect.objectContaining({
        segments: [],
        sourceHistoryId: null,
        title: 'Empty',
        icon: null,
      })
    );
    expect(useTranscriptPlaybackStore.getState()).toEqual(
      expect.objectContaining({
        audioFile: null,
        audioUrl: null,
        activeSegmentId: null,
        activeSegmentIndex: -1,
      })
    );
  });

  it('clears stale auto-save status when opening a persisted transcript session', () => {
    const sidecarStore = useTranscriptSidecarStore.getState();

    sidecarStore.setAutoSaveState('hist-1', 'saving');
    sidecarStore.setAutoSaveState('hist-2', 'saved');

    openTranscriptSession({
      segments: [{ id: 'seg-1', text: 'Loaded', start: 0, end: 1, isFinal: true }],
      sourceHistoryId: 'hist-1',
      title: 'Loaded transcript',
    });

    expect(useTranscriptSidecarStore.getState().autoSaveStates['hist-1']).toBeUndefined();
    expect(useTranscriptSidecarStore.getState().autoSaveStates['hist-2']).toEqual({
      status: 'saved',
      updatedAt: expect.any(Number),
    });
  });

  it('playback store tracks seek requests and active segment highlighting from the session snapshot', () => {
    openTranscriptSession({
      segments: [
        { id: 'seg-1', text: 'First', start: 0, end: 5, isFinal: true },
        { id: 'seg-2', text: 'Second', start: 5, end: 10, isFinal: true },
      ],
      sourceHistoryId: null,
    });

    const playbackStore = useTranscriptPlaybackStore.getState();
    playbackStore.requestSeek(7.5);

    expect(useTranscriptPlaybackStore.getState()).toEqual(
      expect.objectContaining({
        currentTime: 7.5,
        activeSegmentId: 'seg-2',
        activeSegmentIndex: 1,
        seekRequest: {
          time: 7.5,
          timestamp: expect.any(Number),
        },
      })
    );

    playbackStore.clearSession();

    expect(useTranscriptPlaybackStore.getState()).toEqual(
      expect.objectContaining({
        audioUrl: null,
        currentTime: 0,
        activeSegmentId: null,
        activeSegmentIndex: -1,
      })
    );
  });

  it('runtime store updates workbench and recording flags independently of transcript content', () => {
    const runtimeStore = useTranscriptRuntimeStore.getState();

    runtimeStore.setMode('create');
    runtimeStore.setIngestionFocus('batch');
    runtimeStore.setProcessingStatus('processing');
    runtimeStore.setProcessingProgress(45);
    runtimeStore.setIsRecording(true);
    runtimeStore.setIsPaused(true);
    runtimeStore.setIsCaptionMode(true);

    expect(useTranscriptRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        mode: 'create',
        ingestionFocus: 'batch',
        processingStatus: 'processing',
        processingProgress: 45,
        isRecording: true,
        isPaused: true,
        isCaptionMode: true,
      })
    );
  });

  it('sidecar store updates llm and auto-save state and rekeys current summary records', () => {
    const sidecarStore = useTranscriptSidecarStore.getState();

    sidecarStore.updateLlmState({ isPolishing: true, polishProgress: 50 }, 'current');
    sidecarStore.setAutoSaveState('hist-1', 'saving');
    sidecarStore.setSummaryState(
      {
        activeTemplateId: 'general',
        record: {
          templateId: 'general',
          content: 'Current summary',
          generatedAt: '2026-04-30T01:00:00.000Z',
          sourceFingerprint: 'fp-current',
        },
      },
      'current'
    );
    sidecarStore.rekeyCurrentSummaryState('hist-2');

    expect(useTranscriptSidecarStore.getState().getLlmState('current')).toEqual(
      expect.objectContaining({
        isPolishing: true,
        polishProgress: 50,
      })
    );
    expect(useTranscriptSidecarStore.getState().autoSaveStates['hist-1']).toEqual({
      status: 'saving',
      updatedAt: expect.any(Number),
    });
    expect(useTranscriptSidecarStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptSidecarStore.getState().getSummaryState('hist-2').record?.content).toBe(
      'Current summary'
    );
  });

  it('applies transcript updates atomically and keeps active selection aligned', () => {
    openTranscriptSession({
      segments: [{ id: 'seg-partial', text: 'Hello world.', start: 0, end: 2, isFinal: false }],
      sourceHistoryId: null,
    });

    applyTranscriptUpdate(
      {
        removeIds: ['seg-partial'],
        upsertSegments: [
          { id: 'seg-final-1', text: 'Hello.', start: 0, end: 1, isFinal: true },
          { id: 'seg-final-2', text: 'World.', start: 1, end: 2, isFinal: true },
        ],
      },
      'seg-final-2'
    );

    expect(useTranscriptSessionStore.getState().segments.map((segment) => segment.id)).toEqual([
      'seg-final-1',
      'seg-final-2',
    ]);
    expect(useTranscriptPlaybackStore.getState()).toEqual(
      expect.objectContaining({
        activeSegmentId: 'seg-final-2',
        activeSegmentIndex: 1,
      })
    );
  });

  it('effective config store falls back to global settings for metadata-only Tags', async () => {
    useConfigStore.setState((state) => ({
      ...state,
      config: {
        ...DEFAULT_CONFIG,
        summaryTemplateId: 'general',
        translationLanguage: 'zh',
      },
    }));
    useProjectStore.setState((state) => ({
      ...state,
      activeProjectId: 'project-1',
      projects: [
        {
          id: 'project-1',
          name: 'Project 1',
          description: '',
          icon: '',
          createdAt: 1,
          updatedAt: 1,
        },
      ],
    }));

    await useEffectiveConfigStore.getState().syncConfig();

    expect(useEffectiveConfigStore.getState().config).toEqual(
      expect.objectContaining({
        summaryTemplateId: 'general',
        translationLanguage: 'zh',
      })
    );
    expect(getEffectiveConfigSnapshot()).toEqual(
      expect.objectContaining({
        summaryTemplateId: 'general',
        translationLanguage: 'zh',
      })
    );
  });

  it('getSessionStoreState maintains referential stability when active session is unchanged', () => {
    const state = useTranscriptStore.getState();
    const firstCall = getSessionStoreState(state);
    const secondCall = getSessionStoreState(state);

    expect(firstCall).toBe(secondCall);

    useTranscriptStore.setState({ processingStatus: 'processing' });
    const afterUnrelatedUpdate = getSessionStoreState(useTranscriptStore.getState());
    expect(afterUnrelatedUpdate).toBe(firstCall);

    useTranscriptSessionStore.getState().addSegment({
      text: 'New segment',
      start: 0,
      end: 1,
      isFinal: true,
    });
    const afterSessionUpdate = getSessionStoreState(useTranscriptStore.getState());
    expect(afterSessionUpdate).not.toBe(firstCall);
  });

  it('preserves cached session state snapshot during playback updates without allocating new objects', () => {
    const initialSessionState = useTranscriptSessionStore.getState();
    const playbackStore = useTranscriptPlaybackStore.getState();

    playbackStore.setCurrentTime(1.5);
    const afterFirstTick = useTranscriptSessionStore.getState();
    expect(afterFirstTick).toBe(initialSessionState);

    playbackStore.setCurrentTime(3.0);
    const afterSecondTick = useTranscriptSessionStore.getState();
    expect(afterSecondTick).toBe(initialSessionState);
  });

  it('triggers transcriptSessionStore.subscribe on session content updates but ignores playback updates', () => {
    const listener = vi.fn();
    const unsubscribe = useTranscriptSessionStore.subscribe(listener);

    useTranscriptPlaybackStore.getState().setCurrentTime(5.0);
    expect(listener).not.toHaveBeenCalled();

    useTranscriptSessionStore.getState().setTitle('New Title');
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'New Title' }),
      expect.objectContaining({ title: '' })
    );

    unsubscribe();
  });

  it('updateTranscriptSegmentForSession isolates updates to the targeted session without touching other sessions', () => {
    const sessionA = 'session-a';
    const sessionB = 'session-b';
    const store = useTranscriptStore.getState();

    store.setSegmentsForSession(sessionA, [
      { id: 'seg-a1', text: 'Original text A', start: 0, end: 1, isFinal: true },
    ]);
    openTranscriptSession({
      sourceHistoryId: sessionB,
      segments: [{ id: 'seg-b1', text: 'Original text B', start: 0, end: 1, isFinal: true }],
    });
    expect(useTranscriptStore.getState().activeSessionId).toBe(sessionB);

    // Update segment in session A while session B is active
    updateTranscriptSegmentForSession(sessionA, 'seg-a1', { text: 'Polished text A' });

    // Assert session A segment was updated
    const updatedSessionA = useTranscriptStore.getState().sessions[sessionA];
    expect(updatedSessionA.segments[0].text).toBe('Polished text A');

    // Assert session B (active session) was untouched
    const untouchedSessionB = useTranscriptStore.getState().sessions[sessionB];
    expect(untouchedSessionB.segments[0].text).toBe('Original text B');
  });

  it('preserves activeSessionId during background recording and synchronizes when switching modes', () => {
    const realStore = useCanonicalTranscriptStore;

    // Set up a live recording session
    realStore.setState({
      activeSessionId: 'live-rec-123',
      recordingSessionId: 'live-rec-123',
      ingestionFocus: 'live',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'live-rec-123': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'live-rec-123',
          title: 'Live Session',
        },
      },
    });

    // Switch to projects mode -> activeSessionId is NOT clobbered to default
    realStore.getState().setMode('projects');
    expect(realStore.getState().mode).toBe('projects');
    expect(realStore.getState().activeSessionId).toBe('live-rec-123');

    // Switch to tools mode -> activeSessionId remains preserved
    realStore.getState().setMode('tools');
    expect(realStore.getState().mode).toBe('tools');
    expect(realStore.getState().activeSessionId).toBe('live-rec-123');

    // Switch back to create mode -> activeSessionId restores to recordingSessionId
    realStore.getState().setMode('create');
    expect(realStore.getState().mode).toBe('create');
    expect(realStore.getState().activeSessionId).toBe('live-rec-123');
  });

  it('resets activeSessionId to default when switching to live focus if not recording', () => {
    const realStore = useCanonicalTranscriptStore;

    // Set up a session where a document was active but recordingSessionId is null
    realStore.setState({
      activeSessionId: 'completed-rec-456',
      recordingSessionId: null,
      ingestionFocus: 'batch',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'completed-rec-456': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'completed-rec-456',
          title: 'Completed Recording',
        },
      },
    });

    // Switching mode to projects does not clobber activeSessionId
    realStore.getState().setMode('projects');
    expect(realStore.getState().activeSessionId).toBe('completed-rec-456');

    // Returning to create mode with live focus sets activeSessionId to default
    realStore.getState().setMode('create', 'live');
    expect(realStore.getState().mode).toBe('create');
    expect(realStore.getState().ingestionFocus).toBe('live');
    expect(realStore.getState().activeSessionId).toBe('default');

    // Calling setIngestionFocus('live') with an active document sets activeSessionId to default
    realStore.setState({ activeSessionId: 'completed-rec-456' });
    realStore.getState().setIngestionFocus('live');
    expect(realStore.getState().activeSessionId).toBe('default');
  });

  it('preserves existing transcript session when switching between create and projects modes without explicit focus', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'saved-transcript-789',
      recordingSessionId: null,
      mode: 'create',
      ingestionFocus: 'live',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'saved-transcript-789': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'saved-transcript-789',
          title: 'Saved Meeting',
          segments: [
            { id: 'seg-1', text: 'Important transcript content', start: 0, end: 1, isFinal: true },
          ],
        },
      },
    });

    // Switch to projects mode
    realStore.getState().setMode('projects');
    expect(realStore.getState().mode).toBe('projects');

    // Switch back to create mode without focus argument (tab switch)
    realStore.getState().setMode('create');
    expect(realStore.getState().mode).toBe('create');
    expect(realStore.getState().activeSessionId).toBe('saved-transcript-789');
    expect(realStore.getState().sessions['saved-transcript-789'].segments).toHaveLength(1);
    expect(realStore.getState().sessions['saved-transcript-789'].segments[0].text).toBe(
      'Important transcript content'
    );
  });

  it('does not adopt projects item into create session when returning to create tab', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'create-session-123',
      createSessionId: 'create-session-123',
      recordingSessionId: null,
      mode: 'create',
      ingestionFocus: 'live',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'create-session-123': {
          ...DEFAULT_SESSION_DATA,
          title: 'Draft in Create',
          segments: [
            { id: 'seg-create', text: 'Create draft content', start: 0, end: 1, isFinal: true },
          ],
        },
        'project-item-456': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'project-item-456',
          title: 'Project Item',
          segments: [
            { id: 'seg-proj', text: 'Project history content', start: 0, end: 2, isFinal: true },
          ],
        },
      },
    });

    // Switch to projects mode and open a project item
    realStore.getState().setMode('projects');
    realStore.getState().openSession({
      sourceHistoryId: 'project-item-456',
      title: 'Project Item',
      segments: [
        { id: 'seg-proj', text: 'Project history content', start: 0, end: 2, isFinal: true },
      ],
    });
    expect(realStore.getState().activeSessionId).toBe('project-item-456');
    expect(realStore.getState().createSessionId).toBe('create-session-123');

    // Switch back to create mode via tab navigation (no focus arg)
    realStore.getState().setMode('create');
    expect(realStore.getState().mode).toBe('create');
    expect(realStore.getState().activeSessionId).toBe('create-session-123');
    expect(realStore.getState().sessions['create-session-123'].segments[0].text).toBe(
      'Create draft content'
    );
  });
  it('isolates background recording writes from active foreground session', () => {
    const realStore = useCanonicalTranscriptStore;
    const recSessionId = 'bg-rec-789';
    const activeDocId = 'foreground-doc-123';

    realStore.setState({
      activeSessionId: activeDocId,
      recordingSessionId: recSessionId,
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        [activeDocId]: {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: activeDocId,
          title: 'Foreground Document',
          audioUrl: 'asset://foreground.wav',
          segments: [{ id: 'fg-1', text: 'Foreground segment', start: 0, end: 1, isFinal: true }],
        },
        [recSessionId]: {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: recSessionId,
          title: 'Background Recording',
          audioUrl: null,
          segments: [],
        },
      },
    });

    const newAudioUrl = 'asset://recorded.wav';
    const newSegments: TranscriptSegment[] = [
      { id: 'rec-1', text: 'Recorded segment', start: 0, end: 2, isFinal: true },
    ];

    realStore.setState((state) => ({
      sessions: {
        ...state.sessions,
        [recSessionId]: {
          ...(state.sessions[recSessionId] || DEFAULT_SESSION_DATA),
          audioUrl: newAudioUrl,
        },
      },
    }));
    realStore.getState().setSegmentsForSession(recSessionId, newSegments);

    const fgSession = realStore.getState().sessions[activeDocId];
    expect(fgSession.audioUrl).toBe('asset://foreground.wav');
    expect(fgSession.segments).toHaveLength(1);
    expect(fgSession.segments[0].text).toBe('Foreground segment');

    const bgSession = realStore.getState().sessions[recSessionId];
    expect(bgSession.audioUrl).toBe('asset://recorded.wav');
    expect(bgSession.segments).toHaveLength(1);
    expect(bgSession.segments[0].text).toBe('Recorded segment');
  });

  it('clearActiveTranscriptSession is a no-op while the active session is the recording session', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'rec-1',
      recordingSessionId: 'rec-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'rec-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'rec-1',
          segments: [{ id: 'r1', text: 'Live', start: 0, end: 1, isFinal: false }],
        },
      },
    });

    realStore.getState().clearActiveTranscriptSession({ clearAudio: true });

    expect(realStore.getState().sessions['rec-1'].segments).toHaveLength(1);
    expect(realStore.getState().sessions['rec-1'].sourceHistoryId).toBe('rec-1');
  });

  it('clearActiveTranscriptSession clears a foreground detail while a background recording continues', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'detail-1',
      recordingSessionId: 'rec-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'detail-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'detail-1',
          segments: [{ id: 'd1', text: 'Detail', start: 0, end: 1, isFinal: true }],
        },
        'rec-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'rec-1',
          segments: [{ id: 'r1', text: 'Live', start: 0, end: 1, isFinal: false }],
        },
      },
    });

    realStore.getState().clearActiveTranscriptSession({ clearAudio: true });

    expect(realStore.getState().sessions['detail-1'].segments).toHaveLength(0);
    expect(realStore.getState().sessions['rec-1'].segments).toHaveLength(1);
  });

  it('openSession does not clobber the in-progress recording session', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'rec-1',
      recordingSessionId: 'rec-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'rec-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'rec-1',
          segments: [{ id: 'r1', text: 'Live', start: 0, end: 1, isFinal: false }],
        },
      },
    });

    realStore.getState().openSession({
      sourceHistoryId: 'rec-1',
      segments: [{ id: 'disk', text: 'Stale disk copy', start: 0, end: 1, isFinal: true }],
    });

    expect(realStore.getState().sessions['rec-1'].segments).toHaveLength(1);
    expect(realStore.getState().sessions['rec-1'].segments[0].text).toBe('Live');
  });

  it('finalizeLastSegmentForSession finalizes only the targeted session', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'detail-1',
      recordingSessionId: 'rec-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'detail-1': {
          ...DEFAULT_SESSION_DATA,
          segments: [{ id: 'd1', text: 'Detail', start: 0, end: 1, isFinal: false }],
        },
        'rec-1': {
          ...DEFAULT_SESSION_DATA,
          segments: [{ id: 'r1', text: 'Live', start: 0, end: 1, isFinal: false }],
        },
      },
    });

    realStore.getState().finalizeLastSegmentForSession('rec-1');

    expect(realStore.getState().sessions['rec-1'].segments[0].isFinal).toBe(true);
    expect(realStore.getState().sessions['detail-1'].segments[0].isFinal).toBe(false);
  });

  it('activateSession rebinds the active pointer without disturbing the create pointer', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'create-1',
      createSessionId: 'create-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'create-1': { ...DEFAULT_SESSION_DATA, title: 'Create' },
        'detail-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'detail-1', title: 'Detail' },
      },
    });

    realStore.getState().activateSession('detail-1');
    expect(realStore.getState().activeSessionId).toBe('detail-1');
    expect(realStore.getState().createSessionId).toBe('create-1');

    realStore.getState().activateSession('missing');
    expect(realStore.getState().activeSessionId).toBe('detail-1');
  });

  it('keeps recording intact across projects detail open/close and tab switches', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'rec-1',
      createSessionId: 'rec-1',
      recordingSessionId: 'rec-1',
      mode: 'create',
      ingestionFocus: 'live',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'rec-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'rec-1',
          segments: [{ id: 'r1', text: 'Live', start: 0, end: 1, isFinal: false }],
        },
      },
    });

    // Switch to projects and open a completed detail.
    realStore.getState().setMode('projects');
    realStore.getState().openSession({
      sourceHistoryId: 'detail-1',
      segments: [{ id: 'd1', text: 'Detail', start: 0, end: 1, isFinal: true }],
    });
    expect(realStore.getState().activeSessionId).toBe('detail-1');

    // Closing the detail must not wipe the background recording.
    realStore.getState().clearActiveTranscriptSession({ clearAudio: true });
    expect(realStore.getState().sessions['rec-1'].segments).toHaveLength(1);

    // Returning to create restores the recording session and live focus.
    realStore.getState().setMode('create');
    expect(realStore.getState().activeSessionId).toBe('rec-1');
    expect(realStore.getState().ingestionFocus).toBe('live');
    expect(realStore.getState().sessions['rec-1'].segments[0].text).toBe('Live');
  });

  it('updates background sessions via setSegmentsForHistory and patchSegmentTranslationsForHistory', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'create-1',
      createSessionId: 'create-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'create-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'create-1',
          segments: [{ id: 'c1', text: 'Create text', start: 0, end: 1, isFinal: true }],
        },
        'batch-item-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'hist-bg',
          segments: [{ id: 'b1', text: 'Before polish', start: 0, end: 1, isFinal: true }],
        },
      },
    });

    realStore
      .getState()
      .setSegmentsForHistory('hist-bg', [
        { id: 'b1', text: 'After polish', start: 0, end: 1, isFinal: true },
      ]);
    expect(realStore.getState().sessions['batch-item-1'].segments[0].text).toBe('After polish');
    expect(realStore.getState().sessions['create-1'].segments[0].text).toBe('Create text');

    realStore
      .getState()
      .patchSegmentTranslationsForHistory('hist-bg', [{ id: 'b1', translation: '润色后' }]);
    expect(realStore.getState().sessions['batch-item-1'].segments[0].translation).toBe('润色后');
    expect(realStore.getState().sessions['create-1'].segments[0].translation).toBeUndefined();
  });

  it('evicts deleted history sessions without disturbing an active recording session', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      activeSessionId: 'detail-1',
      createSessionId: 'hist-create',
      recordingSessionId: 'rec-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'detail-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'detail-1' },
        'hist-create': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'hist-create' },
        'rec-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'rec-1' },
      },
    });

    realStore.getState().evictSessionsByHistoryIds(['hist-create', 'detail-1', 'rec-1']);

    expect(realStore.getState().sessions['hist-create']).toBeUndefined();
    expect(realStore.getState().sessions['detail-1']).toBeUndefined();
    expect(realStore.getState().sessions['rec-1']).toBeDefined();
    expect(realStore.getState().activeSessionId).toBe('default');
    expect(realStore.getState().createSessionId).toBe('default');
  });

  it('pauses playback on the leaving session when switching tabs or activating another session', () => {
    const realStore = useCanonicalTranscriptStore;
    realStore.setState({
      mode: 'create',
      activeSessionId: 'create-1',
      createSessionId: 'create-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'create-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'create-1', isPlaying: true },
        'detail-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'detail-1', isPlaying: true },
      },
    });

    realStore.getState().setMode('projects');
    expect(realStore.getState().sessions['create-1'].isPlaying).toBe(false);

    realStore.getState().activateSession('detail-1');
    expect(realStore.getState().sessions['detail-1'].isPlaying).toBe(true);

    realStore.getState().activateSession('create-1');
    expect(realStore.getState().sessions['detail-1'].isPlaying).toBe(false);
  });

  describe('mergeSegments translation preservation', () => {
    it('merges two segments with English translations with a space separator', () => {
      const sessionStore = useTranscriptSessionStore.getState();
      const seg1Id = sessionStore.addSegment({
        text: '<p>Hello</p>',
        start: 0,
        end: 1,
        isFinal: true,
        translation: 'Hello',
        draftTranslation: 'draft1',
      });
      const seg2Id = sessionStore.addSegment({
        text: '<p>world</p>',
        start: 1,
        end: 2,
        isFinal: true,
        translation: 'world',
        draftTranslation: 'draft2',
      });

      sessionStore.mergeSegments(seg1Id, seg2Id);

      const segments = useTranscriptSessionStore.getState().segments;
      expect(segments).toHaveLength(1);
      expect(segments[0].translation).toBe('Hello world');
      expect(segments[0].draftTranslation).toBeUndefined();
    });

    it('merges two segments with CJK translations without space', () => {
      const sessionStore = useTranscriptSessionStore.getState();
      const seg1Id = sessionStore.addSegment({
        text: '<p>你好</p>',
        start: 0,
        end: 1,
        isFinal: true,
        translation: '你好',
      });
      const seg2Id = sessionStore.addSegment({
        text: '<p>世界</p>',
        start: 1,
        end: 2,
        isFinal: true,
        translation: '世界',
      });

      sessionStore.mergeSegments(seg1Id, seg2Id);

      const segments = useTranscriptSessionStore.getState().segments;
      expect(segments).toHaveLength(1);
      expect(segments[0].translation).toBe('你好世界');
    });

    it('preserves single translation when one segment has no translation', () => {
      const sessionStore = useTranscriptSessionStore.getState();
      const seg1Id = sessionStore.addSegment({
        text: '<p>First</p>',
        start: 0,
        end: 1,
        isFinal: true,
        translation: 'Translated part',
      });
      const seg2Id = sessionStore.addSegment({
        text: '<p>Second</p>',
        start: 1,
        end: 2,
        isFinal: true,
      });

      sessionStore.mergeSegments(seg1Id, seg2Id);

      const segments = useTranscriptSessionStore.getState().segments;
      expect(segments).toHaveLength(1);
      expect(segments[0].translation).toBe('Translated part');
    });

    it('retains undivided translation on segmentLeft on split segments in sessionStore', () => {
      const sessionStore = useTranscriptSessionStore.getState();
      const segId = sessionStore.addSegment({
        text: '<p>First half Second half</p>',
        start: 0,
        end: 10,
        isFinal: true,
        translation: '前半段 后半段',
        draftTranslation: 'draft',
      });

      sessionStore.splitTranscriptSegment(segId, '<p>First half</p>', '<p>Second half</p>');

      const segments = useTranscriptSessionStore.getState().segments;
      expect(segments).toHaveLength(2);
      expect(segments[0].translation).toBe('前半段 后半段');
      expect(segments[0].draftTranslation).toBeUndefined();
      expect(segments[1].translation).toBeUndefined();
      expect(segments[1].draftTranslation).toBeUndefined();
    });
  });

  describe('coordinator session utilities and batch focus restore', () => {
    it('evictNonRecordingSessions keeps default, recordingSessionId, and requested keepSessionIds only', () => {
      useTranscriptStore.setState({
        activeSessionId: 'projects-doc',
        createSessionId: 'create-old',
        recordingSessionId: 'rec-live-1',
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'rec-live-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'rec-live-1' },
          'create-old': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'create-old' },
          'projects-doc': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'projects-doc' },
          'kept-doc': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'kept-doc' },
        },
      });

      useTranscriptStore.getState().evictNonRecordingSessions(['kept-doc']);

      const state = useTranscriptStore.getState();
      expect(Object.keys(state.sessions).sort()).toEqual(['default', 'kept-doc', 'rec-live-1']);
      expect(state.createSessionId).toBe('default');
      expect(state.activeSessionId).toBe('default');
    });

    it('updateSessionMetaByHistoryId updates title and icon across non-active sessions matching historyId', () => {
      useTranscriptStore.setState({
        activeSessionId: 'other-doc',
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'other-doc': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'other-doc', title: 'Other' },
          'batch-slot-1': {
            ...DEFAULT_SESSION_DATA,
            sourceHistoryId: 'hist-target',
            title: 'Old Title',
            icon: 'system:file',
          },
        },
      });

      useTranscriptStore
        .getState()
        .updateSessionMetaByHistoryId('hist-target', { title: 'New Title', icon: '🎯' });

      expect(useTranscriptStore.getState().sessions['batch-slot-1'].title).toBe('New Title');
      expect(useTranscriptStore.getState().sessions['batch-slot-1'].icon).toBe('🎯');
      expect(useTranscriptStore.getState().sessions['other-doc'].title).toBe('Other');
    });

    it('patchSegmentTranslationsForSession patches only the addressed session even when activeSessionId changed', () => {
      useTranscriptStore.setState({
        activeSessionId: 'session-b',
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'session-a': {
            ...DEFAULT_SESSION_DATA,
            segments: [{ id: 's1', text: 'Hello', start: 0, end: 1, isFinal: true }],
          },
          'session-b': {
            ...DEFAULT_SESSION_DATA,
            segments: [{ id: 's1', text: 'Other', start: 0, end: 1, isFinal: true }],
          },
        },
      });

      useTranscriptStore
        .getState()
        .patchSegmentTranslationsForSession('session-a', [{ id: 's1', translation: '你好' }]);

      expect(useTranscriptStore.getState().sessions['session-a'].segments[0].translation).toBe(
        '你好'
      );
      expect(
        useTranscriptStore.getState().sessions['session-b'].segments[0].translation
      ).toBeUndefined();
    });

    it('restores batch active item via activeBatchItemIdResolver when returning to Create with no focus arg', () => {
      useTranscriptStore.setState({
        mode: 'projects',
        ingestionFocus: 'batch',
        activeSessionId: 'projects-1',
        createSessionId: 'default',
        sessions: {
          default: { ...DEFAULT_SESSION_DATA },
          'projects-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'projects-1' },
          'batch-item-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'hist-batch-1' },
        },
      });
      registerBatchActiveItemIdResolver(() => 'batch-item-1');
      try {
        useTranscriptRuntimeStore.getState().setMode('create');

        expect(useTranscriptStore.getState().activeSessionId).toBe('batch-item-1');
        expect(useTranscriptStore.getState().createSessionId).toBe('batch-item-1');
      } finally {
        registerBatchActiveItemIdResolver(() => null);
      }
    });
  });
});
