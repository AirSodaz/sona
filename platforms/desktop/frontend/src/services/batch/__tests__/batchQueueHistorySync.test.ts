import { describe, expect, it, vi } from 'vitest';
import {
  applySavedBatchHistoryToQueue,
  resolveSavedBatchHistoryMeta,
} from '../batchQueueHistorySync';
import type { BatchQueueItem } from '../../../types/batchQueue';
import type { HistoryItem } from '../../../types/history';

function makeHistoryItem(overrides: Partial<HistoryItem> = {}): HistoryItem {
  return {
    id: 'history-1',
    timestamp: 1,
    duration: 1,
    audioPath: 'history-1.wav',
    transcriptPath: 'history-1.json',
    title: 'Batch meeting.wav',
    previewText: 'Hello',
    projectId: null,
    type: 'batch',
    ...overrides,
  };
}

function makeQueueItem(overrides: Partial<BatchQueueItem> = {}): BatchQueueItem {
  return {
    id: 'queue-1',
    filename: 'meeting.wav',
    filePath: '/audio/meeting.wav',
    status: 'complete',
    progress: 100,
    segments: [],
    audioUrl: 'asset:///audio/meeting.wav',
    projectId: null,
    ...overrides,
  };
}

describe('batchQueueHistorySync', () => {
  it('resolves saved history metadata with the persisted history audio URL', async () => {
    const getAudioUrl = vi.fn().mockResolvedValue('asset:///history/history-1.wav');

    await expect(resolveSavedBatchHistoryMeta({
      historyItem: makeHistoryItem({ icon: 'system:file-audio', projectId: 'project-1' }),
      getAudioUrl,
    })).resolves.toEqual({
      historyId: 'history-1',
      title: 'Batch meeting.wav',
      icon: 'system:file-audio',
      projectId: 'project-1',
      audioUrl: 'asset:///history/history-1.wav',
    });
    expect(getAudioUrl).toHaveBeenCalledWith('history-1');
  });

  it('does not fall back to the queue source audio URL when history audio is unavailable', async () => {
    const getAudioUrl = vi.fn().mockResolvedValue(null);

    await expect(resolveSavedBatchHistoryMeta({
      historyItem: makeHistoryItem(),
      getAudioUrl,
    })).resolves.toEqual(expect.objectContaining({
      audioUrl: null,
    }));
  });

  it('falls back to the queue project when the saved history item has no project id', async () => {
    const getAudioUrl = vi.fn().mockResolvedValue(null);

    await expect(resolveSavedBatchHistoryMeta({
      historyItem: makeHistoryItem({ projectId: null }),
      fallbackProjectId: 'queue-project',
      getAudioUrl,
    })).resolves.toEqual(expect.objectContaining({
      projectId: 'queue-project',
    }));
  });

  it('does not fall back to the queue source audio URL when history audio lookup fails', async () => {
    const getAudioUrl = vi.fn().mockRejectedValue(new Error('missing audio'));

    await expect(resolveSavedBatchHistoryMeta({
      historyItem: makeHistoryItem(),
      getAudioUrl,
    })).resolves.toEqual(expect.objectContaining({
      audioUrl: null,
    }));
  });

  it('applies saved history metadata to the matching queue item only', () => {
    const firstItem = makeQueueItem();
    const otherItem = makeQueueItem({ id: 'queue-2', filename: 'other.wav' });

    expect(applySavedBatchHistoryToQueue([firstItem, otherItem], 'queue-1', {
      historyId: 'history-1',
      title: 'Batch meeting.wav',
      icon: null,
      projectId: 'project-1',
      audioUrl: 'asset:///history/history-1.wav',
    })).toEqual([
      expect.objectContaining({
        id: 'queue-1',
        historyId: 'history-1',
        historyTitle: 'Batch meeting.wav',
        projectId: 'project-1',
        audioUrl: 'asset:///history/history-1.wav',
      }),
      otherItem,
    ]);
  });
});
