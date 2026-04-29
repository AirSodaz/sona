import { beforeEach, describe, expect, it, vi } from 'vitest';

const testContext = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn().mockImplementation((path) => `asset://${path}`),
  invoke: testContext.invokeMock,
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    error: vi.fn(),
    info: vi.fn(),
  },
}));

import { historyService } from '../historyService';

describe('historyService Performance', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('uses one Rust command for large delete batches instead of per-file IPC fan-out', async () => {
    const itemCount = 500;
    const idsToDelete = Array.from({ length: itemCount }, (_, index) => String(index));
    testContext.invokeMock.mockResolvedValue(undefined);

    const startTime = performance.now();
    await historyService.deleteRecordings(idsToDelete);
    const endTime = performance.now();

    console.log(`deleteRecordings for ${itemCount} items took: ${(endTime - startTime).toFixed(2)}ms`);

    expect(testContext.invokeMock).toHaveBeenCalledTimes(1);
    expect(testContext.invokeMock).toHaveBeenCalledWith('history_delete_items', {
      ids: idsToDelete,
    });
  });
});
