import { describe, expect, it, vi } from 'vitest';
import { handleTaskRetryPreflightFailure } from '../taskRetryFailure';
import { patchTaskLedgerRecord } from '../taskLedgerBuilders';
import type { TaskLedgerRecord } from '../../types/taskLedger';

vi.mock('../taskLedgerBuilders', () => ({
  patchTaskLedgerRecord: vi.fn(),
}));

function makeTask(): TaskLedgerRecord {
  return {
    id: 'task-1',
    kind: 'automation',
    status: 'failed',
    title: 'Failed task',
    progress: 0,
    createdAt: 1,
    updatedAt: 2,
    retryable: true,
    cancelable: false,
    recoverable: false,
  };
}

describe('handleTaskRetryPreflightFailure', () => {
  it('patches the original task as retryable failed work', () => {
    const error = new Error('Retry metadata is missing.');

    try {
      handleTaskRetryPreflightFailure(makeTask(), error);
    } catch {
      // The throw is asserted separately so this test can inspect the patch call.
    }

    expect(patchTaskLedgerRecord).toHaveBeenCalledWith('task-1', {
      status: 'failed',
      progress: 0,
      cancelable: false,
      retryable: true,
      errorMessage: 'Retry metadata is missing.',
    });
  });

  it('rethrows a normalized error with the original cause', () => {
    const cause = { error: 'Tauri command failed.' };

    expect(() => handleTaskRetryPreflightFailure(makeTask(), cause)).toThrow('Tauri command failed.');

    try {
      handleTaskRetryPreflightFailure(makeTask(), cause);
    } catch (error) {
      expect(error).toBeInstanceOf(Error);
      expect((error as Error & { cause?: unknown }).cause).toBe(cause);
    }
  });
});
