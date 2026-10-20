import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useModelDownloadStore } from '../modelDownloadStore';

vi.mock('../../services/tauri/app', () => ({
  cancelDownload: vi.fn().mockResolvedValue(true),
}));

describe('modelDownloadStore', () => {
  beforeEach(() => {
    useModelDownloadStore.setState({ downloads: {} });
    vi.clearAllMocks();
  });

  it('starts download with onCancel and downloadId', () => {
    const onCancel = vi.fn();
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-123',
      onCancel,
    });

    const item = useModelDownloadStore.getState().downloads['test-model'];
    expect(item).toBeDefined();
    expect(item.downloadId).toBe('dl-123');
    expect(item.status).toBe('downloading');
    expect(item.onCancel).toBe(onCancel);
  });

  it('prevents old attempt from clobbering new attempt on complete/fail/cancel (H5)', async () => {
    // Start attempt 1
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-attempt-1',
    });

    // Start attempt 2 (clobbers attempt 1 in active map)
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-attempt-2',
    });

    // Delayed completion from attempt 1 arrives
    useModelDownloadStore.getState().completeDownload('test-model', 'dl-attempt-1');
    // Status must remain 'downloading' from attempt 2
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('downloading');

    // Delayed failure from attempt 1 arrives
    useModelDownloadStore.getState().failDownload('test-model', 'old error', 'dl-attempt-1');
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('downloading');

    // Delayed cancel from attempt 1 arrives
    const cancelled = await useModelDownloadStore
      .getState()
      .cancelDownload('test-model', 'dl-attempt-1');
    expect(cancelled).toBe(false);
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('downloading');

    // Matching cancel for attempt 2 succeeds
    const cancelled2 = await useModelDownloadStore
      .getState()
      .cancelDownload('test-model', 'dl-attempt-2');
    expect(cancelled2).toBe(true);
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('cancelled');
  });

  it('triggers onCancel when cancelling download (H1)', async () => {
    const onCancel = vi.fn();
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-active',
      onCancel,
    });

    await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-active');
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('cancelled');
  });

  it('does not flip to cancelled if backend cancellation fails', async () => {
    const { cancelDownload: tauriCancelDownload } = await import('../../services/tauri/app');
    vi.mocked(tauriCancelDownload).mockRejectedValueOnce(new Error('Backend error'));

    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-active',
    });

    const result = await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-active');
    expect(result).toBe(false);
    // Should NOT flip to cancelled
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('downloading');
  });
});
