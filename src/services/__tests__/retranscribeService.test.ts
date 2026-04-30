import { beforeEach, describe, expect, it, vi } from 'vitest';
import { retranscribeService } from '../retranscribeService';
import { historyService } from '../historyService';
import { transcriptionService } from '../transcriptionService';
import { transcriptSnapshotService } from '../transcriptSnapshotService';
import { useHistoryStore } from '../../stores/historyStore';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../types/transcript';
import { normalizeTranscriptSegments } from '../../utils/transcriptTiming';

vi.mock('../../stores/effectiveConfigStore', () => ({
  getEffectiveConfigSnapshot: () => ({
    offlineModelPath: 'C:/models/asr.onnx',
    enableITN: true,
    language: 'auto',
  }),
  useEffectiveConfigStore: {
    getState: () => ({
      config: {
        offlineModelPath: 'C:/models/asr.onnx',
        enableITN: true,
        language: 'auto',
      },
      syncConfig: vi.fn(),
    }),
    setState: vi.fn(),
    subscribe: vi.fn(() => () => undefined),
  },
}));

vi.mock('../historyService', () => ({
  historyService: {
    getAll: vi.fn(),
    getAudioAbsolutePath: vi.fn(),
  },
}));

vi.mock('../transcriptionService', () => ({
  transcriptionService: {
    setModelPath: vi.fn(),
    setEnableITN: vi.fn(),
    transcribeFile: vi.fn(),
  },
}));

vi.mock('../transcriptSnapshotService', () => ({
  transcriptSnapshotService: {
    createSnapshot: vi.fn(),
  },
}));

describe('RetranscribeService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    vi.mocked(historyService.getAll).mockResolvedValue([
      {
        id: 'history-a',
        timestamp: 1,
        duration: 2,
        audioPath: 'history-a.wav',
        transcriptPath: 'history-a.json',
        title: 'History A',
        previewText: '',
        type: 'recording',
        searchContent: '',
        projectId: null,
        status: 'complete',
      },
    ]);
    vi.mocked(historyService.getAudioAbsolutePath).mockResolvedValue('C:/audio/history-a.wav');
    vi.mocked(transcriptSnapshotService.createSnapshot).mockResolvedValue(null);
  });

  it('creates a snapshot before re-transcription overwrites the saved transcript', async () => {
    const originalSegments: TranscriptSegment[] = [
      { id: 'old-1', start: 0, end: 1, text: 'old text', isFinal: true },
    ];
    const nextSegments: TranscriptSegment[] = [
      { id: 'new-1', start: 0, end: 1, text: 'new text', isFinal: true },
    ];
    const updateTranscript = vi.fn().mockResolvedValue(undefined);

    useTranscriptStore.setState({
      sourceHistoryId: 'history-a',
      segments: originalSegments,
    });
    useHistoryStore.setState({ updateTranscript } as Partial<ReturnType<typeof useHistoryStore.getState>>);
    vi.mocked(transcriptionService.transcribeFile).mockResolvedValue(nextSegments);

    await retranscribeService.retranscribeCurrentRecord();

    expect(transcriptSnapshotService.createSnapshot).toHaveBeenCalledWith(
      'history-a',
      'retranscribe',
      originalSegments,
    );
    expect(
      vi.mocked(transcriptSnapshotService.createSnapshot).mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(transcriptionService.transcribeFile).mock.invocationCallOrder[0],
    );
    expect(updateTranscript).toHaveBeenCalledWith('history-a', nextSegments);
    expect(useTranscriptStore.getState().segments).toEqual(normalizeTranscriptSegments(nextSegments));
  });
});
