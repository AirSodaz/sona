import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../historyService', () => ({
  historyService: {
    getAll: vi.fn(),
    saveRecording: vi.fn(),
    saveImportedFile: vi.fn(),
    deleteRecording: vi.fn(),
    deleteRecordings: vi.fn(),
    updateTranscript: vi.fn(),
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
    vi.mocked(historyService.updateTranscript).mockResolvedValue(undefined);
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

    expect(historyService.updateTranscript).toHaveBeenCalledWith(
      'history-1',
      [
        expect.objectContaining({
          id: 'seg-1',
          speaker: { id: 'speaker-1', label: 'Alice', kind: 'identified' },
        }),
      ],
    );
  });
});
