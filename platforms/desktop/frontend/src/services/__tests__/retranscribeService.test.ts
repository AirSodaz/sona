import { beforeEach, describe, expect, it, vi } from 'vitest';
import { type HistoryState, useHistoryStore } from '../../stores/historyStore';
import {
  DEFAULT_SESSION_DATA,
  useTranscriptStore as useCanonicalTranscriptStore,
} from '../../stores/transcriptStore';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../types/transcript';
import { normalizeTranscriptSegments } from '../../utils/transcriptTiming';
import { historyService } from '../historyService';
import { retranscribeService } from '../retranscribeService';
import { transcriptionService } from '../transcriptionService';
import { transcriptSnapshotService } from '../transcriptSnapshotService';

vi.mock('../../stores/effectiveConfigStore', () => ({
  getEffectiveConfigSnapshot: () => ({
    batchModelPath: 'C:/models/asr.onnx',
    enableITN: true,
    language: 'zh',
    batchLanguage: 'ja',
  }),
  useEffectiveConfigStore: {
    getState: () => ({
      config: {
        batchModelPath: 'C:/models/asr.onnx',
        enableITN: true,
        language: 'zh',
        batchLanguage: 'ja',
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
    useHistoryStore.setState({ updateTranscript } as Partial<HistoryState>);
    vi.mocked(transcriptionService.transcribeFile).mockResolvedValue(nextSegments);

    await retranscribeService.retranscribeCurrentRecord();

    expect(transcriptSnapshotService.createSnapshot).toHaveBeenCalledWith(
      'history-a',
      'retranscribe',
      originalSegments
    );
    expect(
      vi.mocked(transcriptSnapshotService.createSnapshot).mock.invocationCallOrder[0]
    ).toBeLessThan(vi.mocked(transcriptionService.transcribeFile).mock.invocationCallOrder[0]);
    expect(updateTranscript).toHaveBeenCalledWith('history-a', nextSegments);
    expect(useTranscriptStore.getState().segments).toEqual(
      normalizeTranscriptSegments(nextSegments)
    );
  });

  it('uses batchLanguage when retranscribing', async () => {
    vi.mocked(transcriptionService.transcribeFile).mockResolvedValue([]);
    useTranscriptStore.setState({
      sourceHistoryId: 'history-a',
      segments: [],
    });
    useHistoryStore.setState({
      updateTranscript: vi.fn().mockResolvedValue(undefined),
    } as Partial<HistoryState>);

    await retranscribeService.retranscribeCurrentRecord();

    expect(transcriptionService.transcribeFile).toHaveBeenCalledWith(
      'C:/audio/history-a.wav',
      undefined,
      undefined,
      'ja'
    );
  });

  it('writes retranscribed segments only to the originating history session when switching tabs mid-job', async () => {
    const originalSegments: TranscriptSegment[] = [
      { id: 'old-1', start: 0, end: 1, text: 'old text', isFinal: true },
    ];
    const nextSegments: TranscriptSegment[] = [
      { id: 'new-1', start: 0, end: 1, text: 'new text', isFinal: true },
    ];
    const otherSegments: TranscriptSegment[] = [
      { id: 'rec-1', start: 0, end: 1, text: 'live recording', isFinal: false },
    ];
    const updateTranscript = vi.fn().mockResolvedValue(undefined);

    useCanonicalTranscriptStore.setState({
      activeSessionId: 'history-a',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'history-a': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'history-a',
          segments: originalSegments,
        },
        'live-rec': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'live-rec',
          segments: otherSegments,
        },
      },
    });
    useHistoryStore.setState({ updateTranscript } as Partial<HistoryState>);
    vi.mocked(transcriptionService.transcribeFile).mockImplementation(async () => {
      useCanonicalTranscriptStore.getState().activateSession('live-rec');
      return nextSegments;
    });

    await retranscribeService.retranscribeCurrentRecord();

    expect(useCanonicalTranscriptStore.getState().sessions['history-a'].segments).toEqual(
      normalizeTranscriptSegments(nextSegments)
    );
    expect(useCanonicalTranscriptStore.getState().sessions['live-rec'].segments).toEqual(
      otherSegments
    );
  });
});
