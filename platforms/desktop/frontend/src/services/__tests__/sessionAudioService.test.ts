import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { historyService } from '../historyService';
import { resolveCurrentSessionAudioPath } from '../sessionAudioService';

vi.mock('../historyService', () => ({
  historyService: {
    getAudioAbsolutePath: vi.fn(),
  },
}));

describe('sessionAudioService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTranscriptSessionStore.setState({ sourceHistoryId: null });
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null });
  });

  it('resolves audio path from history service when sourceHistoryId exists', async () => {
    useTranscriptSessionStore.setState({ sourceHistoryId: 'hist-123' });
    vi.mocked(historyService.getAudioAbsolutePath).mockResolvedValue('C:/recordings/hist-123.wav');

    const result = await resolveCurrentSessionAudioPath();

    expect(historyService.getAudioAbsolutePath).toHaveBeenCalledWith('hist-123');
    expect(result).toBe('C:/recordings/hist-123.wav');
  });

  it('falls back to active batch queue item file path when no history id is present', async () => {
    useTranscriptSessionStore.setState({ sourceHistoryId: null });
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filePath: 'D:/audio/lecture.mp3',
          filename: 'lecture.mp3',
          status: 'processing',
          progress: 50,
          segments: [],
          projectId: null,
        },
      ],
      activeItemId: 'item-1',
    });

    const result = await resolveCurrentSessionAudioPath();

    expect(result).toBe('D:/audio/lecture.mp3');
  });

  it('returns null when neither history audio nor active batch item exists', async () => {
    useTranscriptSessionStore.setState({ sourceHistoryId: null });
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null });

    const result = await resolveCurrentSessionAudioPath();

    expect(result).toBeNull();
  });
});
