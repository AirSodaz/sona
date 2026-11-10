import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../historyService', () => ({
  historyService: {
    getAll: vi.fn(),
    saveRecording: vi.fn(),
    saveImportedFile: vi.fn(),
    deleteRecording: vi.fn(),
    deleteRecordings: vi.fn(),
    updateTranscript: vi.fn(),
    commitTranscriptEdit: vi.fn(),
    updateItemMeta: vi.fn(),
    updateProjectAssignments: vi.fn(),
    updateProjectAssignmentsByCurrentProject: vi.fn(),
  },
}));

import {
  openTranscriptSession,
  setTranscriptSegments,
  updateTranscriptSegment,
} from '../../stores/transcriptCoordinator';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { useTranscriptSidecarStore } from '../../stores/transcriptSidecarStore';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from '../../stores/transcriptStore';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import { historyService, type TranscriptEditCommitResult } from '../historyService';
import { transcriptAutoSaveRuntime } from '../transcriptAutoSaveRuntime';

describe('transcriptAutoSaveRuntime', () => {
  beforeEach(() => {
    resetTranscriptStores();
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.mocked(historyService.commitTranscriptEdit).mockResolvedValue({ status: 'unchanged' });
  });

  afterEach(() => {
    transcriptAutoSaveRuntime.stop();
    vi.useRealTimers();
  });

  it('persists saved transcripts when only speaker metadata changes', async () => {
    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [
        {
          id: 'seg-1',
          text: 'Hello',
          start: 0,
          end: 1,
          isFinal: true,
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
      ],
    });

    transcriptAutoSaveRuntime.start();

    updateTranscriptSegment('seg-1', {
      speaker: { id: 'speaker-1', label: 'Alice', kind: 'identified' },
    });

    await vi.advanceTimersByTimeAsync(2100);

    expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
      'history-1',
      expect.any(String),
      expect.any(Array),
      [
        expect.objectContaining({
          id: 'seg-1',
          speaker: { id: 'speaker-1', label: 'Alice', kind: 'identified' },
        }),
      ]
    );
  });

  it('serializes slow saves and commits the latest edit with the advanced baseline', async () => {
    let resolveFirst: (value: { status: 'unchanged' }) => void = () => {
      throw new Error('first save was not started');
    };
    vi.mocked(historyService.commitTranscriptEdit)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          })
      )
      .mockResolvedValue({ status: 'unchanged' });
    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [{ id: 'seg-1', text: 'Original', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    updateTranscriptSegment('seg-1', { text: 'First' });
    await vi.advanceTimersByTimeAsync(2000);
    updateTranscriptSegment('seg-1', { text: 'Second' });
    await vi.advanceTimersByTimeAsync(2000);
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledTimes(1);

    resolveFirst({ status: 'unchanged' });
    await vi.waitFor(() => expect(historyService.commitTranscriptEdit).toHaveBeenCalledTimes(2));

    const secondCall = vi.mocked(historyService.commitTranscriptEdit).mock.calls[1];
    expect(secondCall[2][0].text).toBe('First');
    expect(secondCall[3][0].text).toBe('Second');
    expect(secondCall[1]).toBe(vi.mocked(historyService.commitTranscriptEdit).mock.calls[0][1]);
  });

  it('rebaselines after external update without false conflict', async () => {
    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [{ id: 'seg-1', text: 'Original', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    // External update (e.g. polish or translation) finishes and rebaselines
    const polishedSegments = [
      { id: 'seg-1', text: 'Polished text', start: 0, end: 1, isFinal: true },
    ];
    setTranscriptSegments(polishedSegments);
    transcriptAutoSaveRuntime.rebaseline('history-1', polishedSegments);

    expect(useTranscriptSidecarStore.getState().autoSaveStates['history-1']?.status).toBe('saved');

    // User now modifies content in the editor
    updateTranscriptSegment('seg-1', { text: 'Polished text with user edit' });

    await vi.advanceTimersByTimeAsync(2100);

    // Commit should be called with the polished text as baseline, not the original text!
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
      'history-1',
      expect.any(String),
      [expect.objectContaining({ text: 'Polished text' })],
      [expect.objectContaining({ text: 'Polished text with user edit' })]
    );
  });

  it('does not trigger autosave while isPolishing or isTranslating is active', async () => {
    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [{ id: 'seg-1', text: 'Original', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    // Set polishing state
    useTranscriptSidecarStore.getState().updateLlmState({ isPolishing: true }, 'history-1');

    // Streaming chunks update the session store
    setTranscriptSegments([
      { id: 'seg-1', text: 'Streaming chunk 1', start: 0, end: 1, isFinal: true },
    ]);

    await vi.advanceTimersByTimeAsync(2100);

    // Auto-save should NOT be called during active polish!
    expect(historyService.commitTranscriptEdit).not.toHaveBeenCalled();
  });

  it('reconciles conflict when DB current segments already match pending segments', async () => {
    const currentDbSegments = [
      { id: 'seg-1', text: 'Already saved in DB', start: 0, end: 1, isFinal: true },
    ];
    vi.mocked(historyService.commitTranscriptEdit).mockResolvedValueOnce({
      status: 'conflict',
      currentSegments: currentDbSegments,
    });

    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [{ id: 'seg-1', text: 'Stale', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    // User's editor has the segments that match what DB already has
    updateTranscriptSegment('seg-1', { text: 'Already saved in DB' });

    await vi.advanceTimersByTimeAsync(2100);

    // Because currentSegments in DB matched pending segments, it reconciles as saved
    expect(useTranscriptSidecarStore.getState().autoSaveStates['history-1']?.status).toBe('saved');
  });

  it('stores error message on autoSaveState when commit fails with conflict', async () => {
    vi.mocked(historyService.commitTranscriptEdit).mockResolvedValueOnce({
      status: 'conflict',
      currentSegments: [
        { id: 'seg-1', text: 'Conflicting remote text', start: 0, end: 1, isFinal: true },
      ],
    });

    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [{ id: 'seg-1', text: 'Base', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    updateTranscriptSegment('seg-1', { text: 'Local conflicting edit' });

    await vi.advanceTimersByTimeAsync(2100);

    const state = useTranscriptSidecarStore.getState().autoSaveStates['history-1'];
    expect(state?.status).toBe('error');
    expect(state?.errorMessage).toBeDefined();
  });

  it('ignores stale in-flight conflict if session was rebaselined while commit was in flight', async () => {
    const { promise, resolve: resolveCommit } = Promise.withResolvers<TranscriptEditCommitResult>();
    vi.mocked(historyService.commitTranscriptEdit).mockReturnValueOnce(promise);

    openTranscriptSession({
      sourceHistoryId: 'history-1',
      segments: [{ id: 'seg-1', text: 'Base', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    updateTranscriptSegment('seg-1', { text: 'In flight edit' });
    await vi.advanceTimersByTimeAsync(2100);

    // External rebaseline happens (e.g. LLM finishes) while commit is in flight
    const rebaselinedSegments = [
      { id: 'seg-1', text: 'Rebaselined text', start: 0, end: 1, isFinal: true },
    ];
    setTranscriptSegments(rebaselinedSegments);
    transcriptAutoSaveRuntime.rebaseline('history-1', rebaselinedSegments);

    expect(useTranscriptSidecarStore.getState().autoSaveStates['history-1']?.status).toBe('saved');

    // Stale in-flight commit resolves with conflict
    resolveCommit({
      status: 'conflict',
      currentSegments: [{ id: 'seg-1', text: 'Old remote', start: 0, end: 1, isFinal: true }],
    });

    await vi.waitFor(() => {
      expect(historyService.commitTranscriptEdit).toHaveBeenCalledTimes(1);
    });

    // Stale conflict should NOT overwrite the newly rebaselined 'saved' state
    const state = useTranscriptSidecarStore.getState().autoSaveStates['history-1'];
    expect(state?.status).toBe('saved');
  });

  it('auto-saves background recording segments while user is active on another document', async () => {
    // 1. Setup foreground session
    openTranscriptSession({
      sourceHistoryId: 'foreground-doc',
      segments: [{ id: 'fg-1', text: 'Foreground Base', start: 0, end: 1, isFinal: true }],
    });

    // 2. Setup background recording session
    useTranscriptStore.setState((state) => ({
      ...state,
      recordingSessionId: 'background-rec',
      sessions: {
        ...state.sessions,
        'background-rec': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'background-rec',
          segments: [],
        },
      },
    }));
    useTranscriptRuntimeStore.setState({ isRecording: true });

    transcriptAutoSaveRuntime.start();

    // 3. New segment arrives for background recording
    const bgSegment = { id: 'bg-1', text: 'Live audio chunk', start: 0, end: 2, isFinal: true };
    useTranscriptStore.getState().setSegmentsForSession('background-rec', [bgSegment]);

    // Background recording uses LIVE_DRAFT_AUTO_SAVE_DELAY_MS (500ms)
    await vi.advanceTimersByTimeAsync(300);
    expect(historyService.commitTranscriptEdit).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(250); // total 550ms
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledTimes(1);
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
      'background-rec',
      expect.any(String),
      expect.any(Array),
      [expect.objectContaining({ id: 'bg-1', text: 'Live audio chunk' })]
    );
  });

  it('handles concurrent foreground editing and background recording without timer collision', async () => {
    openTranscriptSession({
      sourceHistoryId: 'foreground-doc',
      segments: [{ id: 'fg-1', text: 'Foreground Base', start: 0, end: 1, isFinal: true }],
    });

    useTranscriptStore.setState((state) => ({
      ...state,
      recordingSessionId: 'background-rec',
      sessions: {
        ...state.sessions,
        'background-rec': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'background-rec',
          segments: [],
        },
      },
    }));
    useTranscriptRuntimeStore.setState({ isRecording: true });

    transcriptAutoSaveRuntime.start();

    // At t=0: Foreground edit occurs (delay = 2000ms)
    updateTranscriptSegment('fg-1', { text: 'Foreground Edited' });

    // At t=100: Background recording segment arrives (delay = 500ms)
    await vi.advanceTimersByTimeAsync(100);
    const bgSegment = { id: 'bg-1', text: 'Background speech', start: 0, end: 1, isFinal: true };
    useTranscriptStore.getState().setSegmentsForSession('background-rec', [bgSegment]);

    // At t=650: Background 500ms debounce expires (100 + 500 = 600ms)
    await vi.advanceTimersByTimeAsync(550);
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledTimes(1);
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
      'background-rec',
      expect.any(String),
      expect.any(Array),
      [expect.objectContaining({ text: 'Background speech' })]
    );

    // At t=2100: Foreground 2000ms debounce expires
    await vi.advanceTimersByTimeAsync(1450);
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledTimes(2);
    expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
      'foreground-doc',
      expect.any(String),
      expect.any(Array),
      [expect.objectContaining({ text: 'Foreground Edited' })]
    );
  });

  it('flushes pending changes across all sessions when flushPending is called with no arguments', async () => {
    openTranscriptSession({
      sourceHistoryId: 'foreground-doc',
      segments: [{ id: 'fg-1', text: 'Foreground Base', start: 0, end: 1, isFinal: true }],
    });

    useTranscriptStore.setState((state) => ({
      ...state,
      recordingSessionId: 'background-rec',
      sessions: {
        ...state.sessions,
        'background-rec': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'background-rec',
          segments: [],
        },
      },
    }));
    useTranscriptRuntimeStore.setState({ isRecording: true });

    transcriptAutoSaveRuntime.start();

    // Trigger changes in both
    updateTranscriptSegment('fg-1', { text: 'Foreground Unflushed' });
    useTranscriptStore
      .getState()
      .setSegmentsForSession('background-rec', [
        { id: 'bg-1', text: 'Background Unflushed', start: 0, end: 1, isFinal: true },
      ]);

    // Immediately call flushPending() with no arguments (e.g. app quit / quitGuard)
    await transcriptAutoSaveRuntime.flushPending();

    const commitCalls = vi.mocked(historyService.commitTranscriptEdit).mock.calls;
    expect(commitCalls.length).toBe(2);
    const committedHistoryIds = commitCalls.map((c) => c[0]);
    expect(committedHistoryIds).toContain('foreground-doc');
    expect(committedHistoryIds).toContain('background-rec');
  });

  it('flushes active session when user switches away', async () => {
    openTranscriptSession({
      sourceHistoryId: 'session-1',
      segments: [{ id: 'seg-1', text: 'Original', start: 0, end: 1, isFinal: true }],
    });

    transcriptAutoSaveRuntime.start();

    // User edits session-1
    updateTranscriptSegment('seg-1', { text: 'Unsaved Edit Before Switch' });

    // User switches immediately to session-2 before 2000ms timer completes
    await vi.advanceTimersByTimeAsync(200);
    openTranscriptSession({
      sourceHistoryId: 'session-2',
      segments: [{ id: 'seg-2', text: 'Doc 2', start: 0, end: 1, isFinal: true }],
    });

    // Switching away should have immediately enqueued and drained the edit for session-1
    await vi.waitFor(() => {
      expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
        'session-1',
        expect.any(String),
        expect.any(Array),
        [expect.objectContaining({ text: 'Unsaved Edit Before Switch' })]
      );
    });
  });

  it('flushes pending edits when sessionId differs from sourceHistoryId on completed batch sessions', async () => {
    vi.mocked(historyService.commitTranscriptEdit).mockResolvedValue({
      status: 'unchanged',
    });

    useTranscriptStore.setState({
      activeSessionId: 'batch-queue-item-1',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'batch-queue-item-1': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'hist-batch-1',
          segments: [{ id: 'b-1', text: 'Original batch', start: 0, end: 1, isFinal: true }],
        },
      },
    });
    transcriptAutoSaveRuntime.rebaseline('hist-batch-1', [
      { id: 'b-1', text: 'Original batch', start: 0, end: 1, isFinal: true },
    ]);
    transcriptAutoSaveRuntime.start();

    updateTranscriptSegment('b-1', { text: 'Edited batch while viewing' });
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-batch-1')).toBe(true);

    // Explicitly flush by historyId (even when activeSessionId differs)
    useTranscriptStore.getState().activateSession('default');
    await transcriptAutoSaveRuntime.flushPending('hist-batch-1');

    expect(historyService.commitTranscriptEdit).toHaveBeenCalledWith(
      'hist-batch-1',
      expect.any(String),
      expect.any(Array),
      [expect.objectContaining({ text: 'Edited batch while viewing' })]
    );
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-batch-1')).toBe(false);
  });

  it('updates baseline quietly with resetBaselineIfTracked without setting saved toast status', () => {
    const initialSegments = [{ id: 's1', text: 'One', start: 0, end: 1, isFinal: true }];
    const reloadedSegments = [{ id: 's1', text: 'Two', start: 0, end: 1, isFinal: true }];

    transcriptAutoSaveRuntime.rebaseline('hist-quiet-1', initialSegments);
    useTranscriptStore.getState().clearAutoSaveState('hist-quiet-1');
    expect(useTranscriptSidecarStore.getState().autoSaveStates['hist-quiet-1']).toBeUndefined();

    transcriptAutoSaveRuntime.resetBaselineIfTracked('hist-quiet-1', reloadedSegments);
    transcriptAutoSaveRuntime.resetBaselineIfTracked('untracked-id', reloadedSegments);

    expect(useTranscriptSidecarStore.getState().autoSaveStates['hist-quiet-1']).toBeUndefined();
    expect(useTranscriptSidecarStore.getState().autoSaveStates['untracked-id']).toBeUndefined();
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-quiet-1')).toBe(false);
  });

  it('cancels debounced auto-save timers via cancelPending and cancelAllPending so delete/restore never writes stale edits', async () => {
    openTranscriptSession({
      sourceHistoryId: 'hist-del',
      segments: [{ id: 's-1', text: 'Initial', start: 0, end: 1, isFinal: true }],
    });
    transcriptAutoSaveRuntime.start();

    updateTranscriptSegment('s-1', { text: 'Stale pending edit' });
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-del')).toBe(true);

    transcriptAutoSaveRuntime.cancelPending(['hist-del']);
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-del')).toBe(false);

    await vi.advanceTimersByTimeAsync(2500);
    expect(historyService.commitTranscriptEdit).not.toHaveBeenCalled();

    // Verify cancelAllPending clears edits across open sessions too
    updateTranscriptSegment('s-1', { text: 'Another pending edit' });
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-del')).toBe(true);

    transcriptAutoSaveRuntime.cancelAllPending();
    expect(transcriptAutoSaveRuntime.hasPendingWork('hist-del')).toBe(false);

    await vi.advanceTimersByTimeAsync(2500);
    expect(historyService.commitTranscriptEdit).not.toHaveBeenCalled();
  });
});
