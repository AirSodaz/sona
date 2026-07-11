import { beforeEach, describe, expect, it, vi } from 'vitest';
import { retryAutomationTaskFromLedger } from '../automationTaskRetryService';
import { useAutomationStore } from '../../stores/automationStore';
import { patchTaskLedgerRecord } from '../taskLedgerBuilders';
import type { TaskLedgerRecord } from '../../types/taskLedger';

vi.mock('../../stores/automationStore', () => ({
  useAutomationStore: {
    getState: vi.fn(),
  },
}));

vi.mock('../taskLedgerBuilders', () => ({
  patchTaskLedgerRecord: vi.fn(),
}));

function makeTask(overrides: Partial<TaskLedgerRecord> = {}): TaskLedgerRecord {
  return {
    id: 'automation-old-task',
    kind: 'automation',
    status: 'failed',
    title: 'failed.wav',
    progress: 0,
    createdAt: 100,
    updatedAt: 200,
    retryable: true,
    cancelable: false,
    recoverable: false,
    automationRuleId: 'rule-1',
    filePath: 'C:\\watch\\failed.wav',
    ...overrides,
  };
}

describe('retryAutomationTaskFromLedger', () => {
  const retryFailedFile = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    retryFailedFile.mockResolvedValue(undefined);
    vi.mocked(useAutomationStore.getState).mockReturnValue({
      retryFailedFile,
    } as unknown as ReturnType<typeof useAutomationStore.getState>);
  });

  it('routes automation ledger file retries to the automation runtime', async () => {
    await retryAutomationTaskFromLedger(makeTask());

    expect(retryFailedFile).toHaveBeenCalledWith('rule-1', 'C:\\watch\\failed.wav');
    expect(patchTaskLedgerRecord).not.toHaveBeenCalled();
  });

  it('records a preflight failure when retry metadata is missing', async () => {
    await expect(retryAutomationTaskFromLedger(makeTask({
      filePath: undefined,
    }))).rejects.toThrow('Automation task is missing retry metadata.');

    expect(retryFailedFile).not.toHaveBeenCalled();
    expect(patchTaskLedgerRecord).toHaveBeenCalledWith('automation-old-task', expect.objectContaining({
      status: 'failed',
      retryable: true,
      cancelable: false,
      errorMessage: 'Automation task is missing retry metadata.',
    }));
  });

  it('keeps the old ledger task when runtime preflight rejects the retry', async () => {
    retryFailedFile.mockRejectedValue(new Error('Automation rule not found.'));

    await expect(retryAutomationTaskFromLedger(makeTask())).rejects.toThrow('Automation rule not found.');

    expect(patchTaskLedgerRecord).toHaveBeenCalledWith('automation-old-task', expect.objectContaining({
      status: 'failed',
      retryable: true,
      cancelable: false,
      errorMessage: 'Automation rule not found.',
    }));
  });
});
