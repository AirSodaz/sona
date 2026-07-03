import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  resetHistoryAudioCleanupServiceForTests,
  runHistoryAudioCleanupForCurrentConfig,
} from '../historyAudioCleanupService';

const mocks = vi.hoisted(() => ({
  cleanupAudio: vi.fn(),
  config: {
    historyAudioRetentionDays: null as number | null | undefined,
  },
  refreshHistory: vi.fn(),
  sourceHistoryId: 'active-history' as string | null,
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: {
    getState: () => ({
      config: mocks.config,
    }),
  },
}));

vi.mock('../../stores/historyStore', () => ({
  useHistoryStore: {
    getState: () => ({
      refresh: (...args: unknown[]) => mocks.refreshHistory(...args),
    }),
  },
}));

vi.mock('../../stores/transcriptSessionStore', () => ({
  useTranscriptSessionStore: {
    getState: () => ({
      sourceHistoryId: mocks.sourceHistoryId,
    }),
  },
}));

vi.mock('../historyService', () => ({
  historyService: {
    cleanupAudio: (...args: unknown[]) => mocks.cleanupAudio(...args),
  },
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    error: vi.fn(),
    info: vi.fn(),
  },
}));

function cleanupReport(overrides: Partial<{
  eligibleCount: number;
  removedCount: number;
  removedBytes: number;
  missingMarkedCount: number;
  failedCount: number;
  skippedActiveCount: number;
}> = {}) {
  return {
    eligibleCount: 0,
    removedCount: 0,
    removedBytes: 0,
    missingMarkedCount: 0,
    failedCount: 0,
    skippedActiveCount: 0,
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe('historyAudioCleanupService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetHistoryAudioCleanupServiceForTests();
    mocks.config.historyAudioRetentionDays = null;
    mocks.sourceHistoryId = 'active-history';
    mocks.cleanupAudio.mockResolvedValue(cleanupReport());
    mocks.refreshHistory.mockResolvedValue(undefined);
  });

  it('does nothing when retention is keep forever', async () => {
    mocks.config.historyAudioRetentionDays = null;

    const result = await runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 4));

    expect(result).toBeNull();
    expect(mocks.cleanupAudio).not.toHaveBeenCalled();
  });

  it('runs cleanup for finite retention and refreshes history when statuses changed', async () => {
    mocks.config.historyAudioRetentionDays = 30;
    mocks.cleanupAudio.mockResolvedValue(cleanupReport({
      removedCount: 1,
      missingMarkedCount: 1,
    }));

    const result = await runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 4));

    expect(mocks.cleanupAudio).toHaveBeenCalledWith(30, 'active-history');
    expect(mocks.refreshHistory).toHaveBeenCalledTimes(1);
    expect(result?.removedCount).toBe(1);
  });

  it('runs at most once per app day', async () => {
    mocks.config.historyAudioRetentionDays = 7;

    await runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 4, 9));
    await runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 4, 18));
    await runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 5, 9));

    expect(mocks.cleanupAudio).toHaveBeenCalledTimes(2);
  });

  it('shares an in-flight cleanup run instead of starting another one', async () => {
    mocks.config.historyAudioRetentionDays = 90;
    const pending = deferred<ReturnType<typeof cleanupReport>>();
    mocks.cleanupAudio.mockReturnValue(pending.promise);

    const first = runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 4, 9));
    const second = runHistoryAudioCleanupForCurrentConfig(new Date(2026, 6, 4, 9));

    expect(mocks.cleanupAudio).toHaveBeenCalledTimes(1);

    pending.resolve(cleanupReport({ removedCount: 1 }));

    await expect(first).resolves.toEqual(expect.objectContaining({ removedCount: 1 }));
    await expect(second).resolves.toEqual(expect.objectContaining({ removedCount: 1 }));
  });
});
