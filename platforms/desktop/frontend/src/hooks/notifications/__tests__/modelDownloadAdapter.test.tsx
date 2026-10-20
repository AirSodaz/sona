import { describe, expect, it, vi } from 'vitest';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import { adaptModelDownloadsEntries, formatBytes } from '../modelDownloadAdapter';

const mockT = ((key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
  if (options?.defaultValue) return options.defaultValue;
  return key;
}) as any;

describe('formatBytes', () => {
  it('formats MB and GB correctly', () => {
    expect(formatBytes(undefined)).toBe('');
    expect(formatBytes(0)).toBe('');
    expect(formatBytes(-100)).toBe('');
    expect(formatBytes(50 * 1024 * 1024)).toBe('50 MB');
    expect(formatBytes(1.5 * 1024 * 1024 * 1024)).toBe('1.50 GB');
  });
});

describe('adaptModelDownloadsEntries', () => {
  const callbacks = {
    cancelDownload: vi.fn(),
    dismissDownload: vi.fn(),
  };

  it('adapts active download entry with progress and cancel action', () => {
    const entries = adaptModelDownloadsEntries(
      {
        dl1: {
          id: 'dl1',
          modelId: 'sensevoice',
          modelName: 'SenseVoice Small',
          modelType: 'asr',
          status: 'downloading',
          progress: 45,
          downloadedBytes: 100 * 1024 * 1024,
          totalBytes: 200 * 1024 * 1024,
          speed: '5.2 MB/s',
          timestamp: 1000,
          downloadId: 'dl1',
        },
      },
      mockT,
      callbacks
    );

    expect(entries).toHaveLength(1);
    const entry = entries[0];
    expect(entry.id).toBe('model-dl-sensevoice');
    expect(entry.priority).toBe('active');
    expect(entry.tone).toBe('accent');
    expect(entry.progress).toBe(45);
    expect(entry.body).toContain('100 MB / 200 MB');
    expect(entry.body).toContain('5.2 MB/s');
    expect(entry.actions).toHaveLength(1);
    expect(entry.actions[0].id).toBe('cancel-sensevoice');

    entry.actions[0].run();
    expect(callbacks.cancelDownload).toHaveBeenCalledWith('sensevoice');
  });

  it('adapts completed download entry with success tone and close action', () => {
    const entries = adaptModelDownloadsEntries(
      {
        dl2: {
          id: 'dl2',
          modelId: 'qwen3.5-4b',
          modelName: 'Qwen3.5 4B',
          modelType: 'llm',
          status: 'completed',
          progress: 100,
          timestamp: 2000,
          downloadId: 'dl2',
        },
      },
      mockT,
      callbacks
    );

    expect(entries).toHaveLength(1);
    const entry = entries[0];
    expect(entry.priority).toBe('info');
    expect(entry.tone).toBe('success');
    expect(entry.closeAction).toBeDefined();

    entry.closeAction?.run();
    expect(callbacks.dismissDownload).toHaveBeenCalledWith('qwen3.5-4b');
  });

  it('adapts failed download entry with action priority and retry action', () => {
    const onRetry = vi.fn();
    const entries = adaptModelDownloadsEntries(
      {
        dl3: {
          id: 'dl3',
          modelId: 'paraformer',
          modelName: 'Paraformer Large',
          modelType: 'asr',
          status: 'failed',
          progress: 60,
          error: 'Connection reset by peer',
          timestamp: 3000,
          downloadId: 'dl3',
          onRetry,
        },
      },
      mockT,
      callbacks
    );

    expect(entries).toHaveLength(1);
    const entry = entries[0];
    expect(entry.priority).toBe('action');
    expect(entry.tone).toBe('error');
    expect(entry.body).toBe('Connection reset by peer');
    expect(entry.actions).toHaveLength(1);
    expect(entry.actions[0].id).toBe('retry-paraformer');

    entry.actions[0].run();
    expect(onRetry).toHaveBeenCalled();
  });
});

describe('modelDownloadStore', () => {
  it('manages download state transitions properly', () => {
    const store = useModelDownloadStore.getState();

    // Start download
    store.startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'test-dl-1',
    });

    let current = useModelDownloadStore.getState().downloads['test-model'];
    expect(current).toBeDefined();
    expect(current.status).toBe('downloading');
    expect(current.progress).toBe(0);

    // Update progress
    store.updateProgress('test-dl-1', {
      progress: 55,
      downloadedBytes: 550,
      totalBytes: 1000,
      speed: '1.2 MB/s',
    });

    current = useModelDownloadStore.getState().downloads['test-model'];
    expect(current.progress).toBe(55);
    expect(current.downloadedBytes).toBe(550);
    expect(current.speed).toBe('1.2 MB/s');

    // Complete download
    store.completeDownload('test-model');
    current = useModelDownloadStore.getState().downloads['test-model'];
    expect(current.status).toBe('completed');
    expect(current.progress).toBe(100);

    // Clear completed
    store.clearCompleted();
    expect(useModelDownloadStore.getState().downloads['test-model']).toBeUndefined();
  });
});
