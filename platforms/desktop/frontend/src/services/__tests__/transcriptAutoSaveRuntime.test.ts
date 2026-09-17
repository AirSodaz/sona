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
import { useTranscriptSidecarStore } from '../../stores/transcriptSidecarStore';
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
});
