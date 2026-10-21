import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TauriEvent } from '../../services/tauri/events';
import {
  cleanupModelDownloadListener,
  ensureModelDownloadListener,
  useModelDownloadStore,
} from '../modelDownloadStore';

vi.mock('../../services/tauri/app', () => ({
  cancelDownload: vi.fn().mockResolvedValue(true),
}));

let progressListener: ((event: { payload: unknown }) => void) | null = null;
const mockUnlisten = vi.fn();
const mockListen = vi
  .fn()
  .mockImplementation((event: string, cb: (e: { payload: unknown }) => void) => {
    if (event === TauriEvent.app.downloadProgress) {
      progressListener = cb;
    }
    return Promise.resolve(mockUnlisten);
  });

vi.mock('../../services/tauri/platform/events', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

describe('modelDownloadStore', () => {
  beforeEach(() => {
    cleanupModelDownloadListener();
    useModelDownloadStore.setState({ downloads: {} });
    vi.clearAllMocks();
    progressListener = null;
    mockListen.mockImplementation((event: string, cb: (e: { payload: unknown }) => void) => {
      if (event === TauriEvent.app.downloadProgress) {
        progressListener = cb;
      }
      return Promise.resolve(mockUnlisten);
    });
  });

  afterEach(() => {
    cleanupModelDownloadListener();
    vi.unstubAllGlobals();
    vi.useRealTimers();
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

  it('still flips to cancelled even if backend cancellation fails', async () => {
    const { cancelDownload: tauriCancelDownload } = await import('../../services/tauri/app');
    vi.mocked(tauriCancelDownload).mockRejectedValueOnce(new Error('Backend error'));

    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-active',
    });

    const result = await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-active');
    expect(result).toBe(true);
    // Should reliably flip to cancelled when user requests cancellation
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('cancelled');
  });

  it('does not overwrite cancelled status with failDownload', async () => {
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-active',
    });

    await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-active');
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('cancelled');

    // Downstream rejection handler catches abort error and calls failDownload
    useModelDownloadStore.getState().failDownload('test-model', 'AbortError', 'dl-active');
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('cancelled');
  });

  it('maintains cancelled status even when onCancel synchronously calls failDownload', async () => {
    const onCancel = vi.fn(() => {
      useModelDownloadStore.getState().failDownload('test-model', 'AbortError', 'dl-active');
    });

    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-active',
      onCancel,
    });

    const result = await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-active');
    expect(result).toBe(true);
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('cancelled');
  });

  it('does not overwrite completed status with cancelDownload', async () => {
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-active',
    });

    useModelDownloadStore.getState().completeDownload('test-model', 'dl-active');
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('completed');

    const result = await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-active');
    expect(result).toBe(false);
    expect(useModelDownloadStore.getState().downloads['test-model'].status).toBe('completed');
  });

  it('handles dismissDownload, clearCompleted, and clearAllNonActive', () => {
    useModelDownloadStore.getState().startDownload({
      modelId: 'active-1',
      modelName: 'Active 1',
      downloadId: 'dl-active-1',
    });
    useModelDownloadStore.getState().startDownload({
      modelId: 'completed-1',
      modelName: 'Completed 1',
      downloadId: 'dl-completed-1',
    });
    useModelDownloadStore.getState().completeDownload('completed-1', 'dl-completed-1');

    useModelDownloadStore.getState().startDownload({
      modelId: 'dismiss-me',
      modelName: 'Dismiss Me',
      downloadId: 'dl-dismiss-me',
    });
    useModelDownloadStore.getState().dismissDownload('dismiss-me');
    expect(useModelDownloadStore.getState().downloads['dismiss-me']).toBeUndefined();

    useModelDownloadStore.getState().clearCompleted();
    expect(useModelDownloadStore.getState().downloads['completed-1']).toBeUndefined();
    expect(useModelDownloadStore.getState().downloads['active-1']).toBeDefined();

    useModelDownloadStore.getState().clearAllNonActive();
    expect(useModelDownloadStore.getState().downloads['active-1']).toBeDefined();
  });

  it('preserves onRetry when cancelling download so it can be resumed', async () => {
    const onRetry = vi.fn().mockResolvedValue(undefined);
    const onCancel = vi.fn();
    useModelDownloadStore.getState().startDownload({
      modelId: 'test-model',
      modelName: 'Test Model',
      downloadId: 'dl-123',
      onRetry,
      onCancel,
    });

    const cancelled = await useModelDownloadStore.getState().cancelDownload('test-model', 'dl-123');
    expect(cancelled).toBe(true);

    const item = useModelDownloadStore.getState().downloads['test-model'];
    expect(item).toBeDefined();
    expect(item.status).toBe('cancelled');
    expect(item.onRetry).toBe(onRetry);
    expect(item.onCancel).toBeUndefined();
    expect(item.speed).toBeUndefined();

    await item.onRetry?.();
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('handles generation deduplication and cleanup during listener reconnect', async () => {
    vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
    let resolveListen1!: (unlisten: () => void) => void;
    const listenPromise1 = new Promise<() => void>((resolve) => {
      resolveListen1 = resolve;
    });
    const unlisten1 = vi.fn();

    mockListen.mockImplementationOnce(() => listenPromise1);

    const p1 = ensureModelDownloadListener();
    void ensureModelDownloadListener();
    expect(mockListen).toHaveBeenCalledTimes(1);

    cleanupModelDownloadListener();

    resolveListen1(unlisten1);
    await p1;

    expect(unlisten1).toHaveBeenCalledTimes(1);

    const unlisten2 = vi.fn();
    mockListen.mockResolvedValueOnce(unlisten2);
    await ensureModelDownloadListener();
    expect(mockListen).toHaveBeenCalledTimes(2);

    cleanupModelDownloadListener();
    expect(unlisten2).toHaveBeenCalledTimes(1);
  });

  it('calculates speed on progress events and cleans up sample cache', async () => {
    vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
    vi.useFakeTimers();
    await ensureModelDownloadListener();
    expect(progressListener).toBeTypeOf('function');

    useModelDownloadStore.getState().startDownload({
      modelId: 'model-speed',
      modelName: 'Speed Test Model',
      downloadId: 'dl-speed-1',
    });

    vi.setSystemTime(1000);
    progressListener!({
      payload: { id: 'dl-speed-1', downloaded: 100 * 1024, total: 10 * 1024 * 1024 },
    });

    let item = useModelDownloadStore.getState().downloads['model-speed'];
    expect(item.progress).toBe(1);
    expect(item.downloadedBytes).toBe(100 * 1024);
    expect(item.totalBytes).toBe(10 * 1024 * 1024);
    expect(item.speed).toBeUndefined();

    vi.setSystemTime(1200);
    progressListener!({
      payload: { id: 'dl-speed-1', downloaded: 200 * 1024, total: 10 * 1024 * 1024 },
    });
    item = useModelDownloadStore.getState().downloads['model-speed'];
    expect(item.speed).toBeUndefined();

    vi.setSystemTime(1500);
    progressListener!({
      payload: {
        id: 'dl-speed-1',
        downloaded: 100 * 1024 + 1024 * 1024,
        total: 10 * 1024 * 1024,
      },
    });
    item = useModelDownloadStore.getState().downloads['model-speed'];
    expect(item.speed).toBe('2.0 MB/s');

    await useModelDownloadStore.getState().cancelDownload('model-speed', 'dl-speed-1');
    item = useModelDownloadStore.getState().downloads['model-speed'];
    expect(item.status).toBe('cancelled');
    expect(item.speed).toBeUndefined();

    progressListener!({
      payload: { id: 'dl-speed-1', downloaded: 2 * 1024 * 1024, total: 10 * 1024 * 1024 },
    });
    item = useModelDownloadStore.getState().downloads['model-speed'];
    expect(item.status).toBe('cancelled');
    expect(item.speed).toBeUndefined();

    useModelDownloadStore.getState().startDownload({
      modelId: 'model-speed',
      modelName: 'Speed Test Model',
      downloadId: 'dl-speed-2',
    });
    item = useModelDownloadStore.getState().downloads['model-speed'];
    expect(item.status).toBe('downloading');
    expect(item.speed).toBeUndefined();
  });

  it('does not clear active unlistenProgress when a stale listener resolution completes', async () => {
    vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
    let resolveFirstListen: (val: typeof mockUnlisten) => void = () => {};
    const unlisten1 = vi.fn();
    const unlisten2 = vi.fn();

    mockListen.mockImplementationOnce(() => {
      return new Promise((resolve) => {
        resolveFirstListen = resolve;
      });
    });

    const p1 = ensureModelDownloadListener();

    cleanupModelDownloadListener();
    mockListen.mockResolvedValueOnce(unlisten2);
    await ensureModelDownloadListener();

    resolveFirstListen(unlisten1);
    await p1;

    expect(unlisten1).toHaveBeenCalled();

    cleanupModelDownloadListener();
    expect(unlisten2).toHaveBeenCalled();
  });
});
