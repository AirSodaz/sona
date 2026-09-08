import { beforeEach, describe, expect, it, vi } from 'vitest';
import { notifySyncLocalChangeForCommand } from '../tauri/syncLocalChangeBus';
import { syncRuntimeService } from '../syncRuntimeService';
import { useSyncStatusStore } from '../../stores/syncStatusStore';
import type { SyncStatusSnapshot } from '../../types/sync';

const testContext = vi.hoisted(() => {
  const transcriptListeners = new Set<(state: any, previous: any) => void>();
  const batchListeners = new Set<(state: any, previous: any) => void>();
  const transcriptState = { isRecording: false };
  const batchState = { isQueueProcessing: false, queueItems: [] as Array<{ status: string }> };
  return {
    getStatus: vi.fn(),
    runNow: vi.fn(),
    transcriptState,
    batchState,
    setRecording(next: boolean) {
      const previous = { ...transcriptState };
      transcriptState.isRecording = next;
      transcriptListeners.forEach((listener) => listener(transcriptState, previous));
    },
    setBatchState(next: typeof batchState) {
      const previous = { ...batchState, queueItems: [...batchState.queueItems] };
      batchState.isQueueProcessing = next.isQueueProcessing;
      batchState.queueItems = next.queueItems;
      batchListeners.forEach((listener) => listener(batchState, previous));
    },
    transcriptListeners,
    batchListeners,
  };
});

vi.mock('../../stores/transcriptRuntimeStore', () => {
  const useTranscriptRuntimeStore = Object.assign(
    (selector: any) => selector(testContext.transcriptState),
    {
      getState: () => testContext.transcriptState,
      subscribe: (listener: any) => {
        testContext.transcriptListeners.add(listener);
        return () => testContext.transcriptListeners.delete(listener);
      },
    },
  );
  return { useTranscriptRuntimeStore };
});

vi.mock('../../stores/batchQueueStore', () => {
  const useBatchQueueStore = Object.assign(
    (selector: any) => selector(testContext.batchState),
    {
      getState: () => testContext.batchState,
      subscribe: (listener: any) => {
        testContext.batchListeners.add(listener);
        return () => testContext.batchListeners.delete(listener);
      },
    },
  );
  return { useBatchQueueStore };
});

vi.mock('../tauri/sync', () => ({
  getSyncStatus: (...args: unknown[]) => testContext.getStatus(...args),
  runSyncNow: (...args: unknown[]) => testContext.runNow(...args),
}));

vi.mock('../../utils/logger', () => ({
  logger: { warn: vi.fn(), error: vi.fn() },
}));

const IDLE_STATUS: SyncStatusSnapshot = {
  state: 'idle',
  providerId: 'webdav',
  vaultId: 'vault-1',
  preset: 'standard',
  lastSuccessAtMs: null,
  pendingOperationCount: 0,
  conflictCount: 0,
  nextRetryAtMs: null,
  lastError: null,
};

const RUN_RESULT = {
  pulledSegmentCount: 0,
  pulledCheckpointCount: 0,
  pushedSegmentCount: 0,
  appliedOperationCount: 0,
  publishedOperationCount: 0,
  conflictCount: 0,
  checkpointPublished: false,
};

async function flushStartup(): Promise<void> {
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(0);
  await Promise.resolve();
}

describe('syncRuntimeService', () => {
  beforeEach(() => {
    syncRuntimeService.dispose();
    vi.useFakeTimers();
    vi.clearAllMocks();
    testContext.transcriptState.isRecording = false;
    testContext.batchState.isQueueProcessing = false;
    testContext.batchState.queueItems = [];
    testContext.getStatus.mockResolvedValue(IDLE_STATUS);
    testContext.runNow.mockResolvedValue(RUN_RESULT);
    useSyncStatusStore.setState({
      snapshot: { ...IDLE_STATUS, state: 'disabled' },
      isLoaded: false,
      lastRunResult: null,
    });
  });

  it('loads the real status and runs once at startup', async () => {
    syncRuntimeService.init();
    await flushStartup();

    expect(testContext.getStatus).toHaveBeenCalled();
    expect(testContext.runNow).toHaveBeenCalledTimes(1);
  });

  it('debounces local mutations for five seconds', async () => {
    syncRuntimeService.init();
    await flushStartup();
    testContext.runNow.mockClear();

    notifySyncLocalChangeForCommand('history_update_transcript');
    await vi.advanceTimersByTimeAsync(4_999);
    expect(testContext.runNow).not.toHaveBeenCalled();

    notifySyncLocalChangeForCommand('history_update_transcript');
    await vi.advanceTimersByTimeAsync(4_999);
    expect(testContext.runNow).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(testContext.runNow).toHaveBeenCalledTimes(1);
  });

  it('queues a requested run while recording and releases it when recording stops', async () => {
    testContext.transcriptState.isRecording = true;
    syncRuntimeService.init();
    await flushStartup();
    expect(testContext.runNow).not.toHaveBeenCalled();

    notifySyncLocalChangeForCommand('tag_update');
    await vi.advanceTimersByTimeAsync(5_000);
    expect(testContext.runNow).not.toHaveBeenCalled();

    testContext.setRecording(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(testContext.runNow).toHaveBeenCalledTimes(1);
  });

  it('triggers periodic sync every 5 minutes in foreground', async () => {
    syncRuntimeService.init();
    await flushStartup();
    testContext.runNow.mockClear();
    // Advance timer by 4 minutes 50 seconds (less than 5 minutes)
    await vi.advanceTimersByTimeAsync(4 * 60 * 1_000 + 50_000);
    expect(testContext.runNow).not.toHaveBeenCalled();

    // Advance timer past 5 minutes (next heartbeat tick at 5m 10s)
    await vi.advanceTimersByTimeAsync(20_000);
    await flushStartup();
    expect(testContext.runNow).toHaveBeenCalledTimes(1);
  });

  it('throttles focus sync if last sync was recent, but syncs if interval exceeded', async () => {
    syncRuntimeService.init();
    await flushStartup();
    testContext.runNow.mockClear();

    // Focus immediately (0ms since last sync) -> throttled
    window.dispatchEvent(new Event('focus'));
    await vi.advanceTimersByTimeAsync(0);
    expect(testContext.runNow).not.toHaveBeenCalled();

    // Focus after 15 seconds (< 30s) -> still throttled
    await vi.advanceTimersByTimeAsync(15_000);
    window.dispatchEvent(new Event('focus'));
    await vi.advanceTimersByTimeAsync(0);
    expect(testContext.runNow).not.toHaveBeenCalled();

    // Focus after 31 seconds (> 30s) -> triggers immediate sync
    await vi.advanceTimersByTimeAsync(16_000);
    window.dispatchEvent(new Event('focus'));
    await vi.advanceTimersByTimeAsync(0);
    expect(testContext.runNow).toHaveBeenCalledTimes(1);
  });

  it('does not trigger periodic sync when vault is disabled or locked', async () => {
    testContext.getStatus.mockResolvedValue({ ...IDLE_STATUS, state: 'locked' });
    syncRuntimeService.init();
    await flushStartup();
    testContext.runNow.mockClear();

    await vi.advanceTimersByTimeAsync(10 * 60 * 1_000);
    expect(testContext.runNow).not.toHaveBeenCalled();
  });

  it('retries scheduled sync when error backoff expires', async () => {
    const errorStatus: SyncStatusSnapshot = {
      ...IDLE_STATUS,
      state: 'error',
      lastError: { code: 'network', message: 'failed', retryable: true },
      nextRetryAtMs: Date.now() + 30_000,
    };
    testContext.getStatus.mockResolvedValue(errorStatus);
    syncRuntimeService.init();
    await flushStartup();
    testContext.runNow.mockClear();

    // Advance 25 seconds (< 30s) -> not yet
    await vi.advanceTimersByTimeAsync(25_000);
    expect(testContext.runNow).not.toHaveBeenCalled();

    // Advance past 30 seconds -> triggers retry run
    testContext.getStatus.mockResolvedValue(IDLE_STATUS);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(testContext.runNow).toHaveBeenCalledTimes(1);
  });

  it('uses relaxed 15-minute periodic sync when document is hidden', async () => {
    Object.defineProperty(document, 'visibilityState', {
      value: 'hidden',
      configurable: true,
    });
    try {
      syncRuntimeService.init();
      await flushStartup();
      testContext.runNow.mockClear();

      // Advance timer by 10 minutes (> 5 minutes, but < 15 minutes)
      await vi.advanceTimersByTimeAsync(10 * 60 * 1_000);
      expect(testContext.runNow).not.toHaveBeenCalled();

      // Advance timer past 15 minutes
      await vi.advanceTimersByTimeAsync(5 * 60 * 1_000 + 20_000);
      await flushStartup();
      expect(testContext.runNow).toHaveBeenCalledTimes(1);
    } finally {
      Object.defineProperty(document, 'visibilityState', {
        value: 'visible',
        configurable: true,
      });
    }
  });
});
