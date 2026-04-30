import { beforeEach, describe, expect, it, vi } from 'vitest';
import { transcriptSnapshotService } from '../transcriptSnapshotService';
import { historyService } from '../historyService';
import { useHistoryStore } from '../../stores/historyStore';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
import type { HistoryItem } from '../../types/history';

vi.mock('../historyService', () => ({
  historyService: {
    createTranscriptSnapshot: vi.fn(),
    listTranscriptSnapshots: vi.fn(),
    loadTranscriptSnapshot: vi.fn(),
  },
}));

function historyItem(id: string, status: HistoryItem['status'] = 'complete'): HistoryItem {
  return {
    id,
    timestamp: 1,
    duration: 1,
    audioPath: `${id}.wav`,
    transcriptPath: `${id}.json`,
    title: id,
    previewText: '',
    type: 'recording',
    searchContent: '',
    projectId: null,
    status,
  };
}

describe('transcriptSnapshotService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    vi.mocked(historyService.createTranscriptSnapshot).mockResolvedValue({
      id: 'snapshot-1',
      historyId: 'history-a',
      reason: 'polish',
      createdAt: 1,
      segmentCount: 1,
    });
  });

  it('creates a snapshot for saved current transcripts', async () => {
    useHistoryStore.setState({ items: [historyItem('history-a')] });
    useTranscriptStore.setState({
      sourceHistoryId: 'history-a',
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true }],
    });

    await transcriptSnapshotService.createSnapshotForCurrentTranscript('polish');

    expect(historyService.createTranscriptSnapshot).toHaveBeenCalledWith('history-a', 'polish', [
      expect.objectContaining({ id: 'seg-1', text: 'hello' }),
    ]);
  });

  it('skips unsaved current transcripts and live drafts', async () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'current',
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true }],
    });
    await expect(transcriptSnapshotService.createSnapshotForCurrentTranscript('polish')).resolves.toBeNull();

    useHistoryStore.setState({ items: [historyItem('history-a', 'draft')] });
    useTranscriptStore.setState({ sourceHistoryId: 'history-a' });
    await expect(transcriptSnapshotService.createSnapshotForCurrentTranscript('polish')).resolves.toBeNull();

    expect(historyService.createTranscriptSnapshot).not.toHaveBeenCalled();
  });
});
