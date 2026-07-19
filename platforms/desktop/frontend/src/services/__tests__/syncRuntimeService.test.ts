import { beforeEach, describe, expect, it, vi } from 'vitest';
import { notifySyncLocalChangeForCommand } from '../syncLocalChangeBus';
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
});
