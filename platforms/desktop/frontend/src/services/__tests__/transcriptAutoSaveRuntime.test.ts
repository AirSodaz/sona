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

import { historyService } from '../historyService';
import { openTranscriptSession, updateTranscriptSegment } from '../../stores/transcriptCoordinator';
import { transcriptAutoSaveRuntime } from '../transcriptAutoSaveRuntime';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';

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
      ],
    );
  });

  it('serializes slow saves and commits the latest edit with the advanced baseline', async () => {
    let resolveFirst: (value: { status: 'unchanged' }) => void = () => {
      throw new Error('first save was not started');
    };
    vi.mocked(historyService.commitTranscriptEdit)
      .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; }))
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
});
