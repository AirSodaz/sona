import { beforeEach, describe, expect, it, vi } from 'vitest';
import { startAppRuntimeServices } from '../runtime';

const mockLoadRecovery = vi.fn();
const mockLoadTasks = vi.fn();
const mockLoadAndStart = vi.fn();
const mockVoiceTypingInit = vi.fn();
const mockRunHealthCheck = vi.fn();
const mockLoggerError = vi.fn();

vi.mock('../../../stores/recoveryStore', () => ({
  useRecoveryStore: {
    getState: vi.fn(() => ({
      loadRecovery: (...args: unknown[]) => mockLoadRecovery(...args),
    })),
  },
}));

vi.mock('../../../stores/taskLedgerStore', () => ({
  useTaskLedgerStore: {
    getState: vi.fn(() => ({
      loadTasks: (...args: unknown[]) => mockLoadTasks(...args),
    })),
  },
}));

vi.mock('../../../stores/automationStore', () => ({
  useAutomationStore: {
    getState: vi.fn(() => ({
      loadAndStart: (...args: unknown[]) => mockLoadAndStart(...args),
    })),
  },
}));

vi.mock('../../voiceTypingService', () => ({
  voiceTypingService: {
    init: (...args: unknown[]) => mockVoiceTypingInit(...args),
  },
}));

vi.mock('../../healthCheckService', () => ({
  healthCheckService: {
    runHealthCheck: (...args: unknown[]) => mockRunHealthCheck(...args),
  },
}));

vi.mock('../../../utils/logger', () => ({
  logger: {
    error: (...args: unknown[]) => mockLoggerError(...args),
  },
}));

describe('startAppRuntimeServices', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockLoadTasks.mockResolvedValue(undefined);
    mockLoadRecovery.mockResolvedValue(undefined);
    mockLoadAndStart.mockResolvedValue(undefined);
    mockVoiceTypingInit.mockReturnValue(undefined);
    mockRunHealthCheck.mockResolvedValue(undefined);
  });

  it('continues later runtime startup tasks when an earlier step fails', async () => {
    mockLoadAndStart.mockRejectedValue(new Error('automation failed'));

    await startAppRuntimeServices();

    expect(mockLoadTasks).toHaveBeenCalledTimes(1);
    expect(mockLoadRecovery).toHaveBeenCalledTimes(1);
    expect(mockLoadAndStart).toHaveBeenCalledTimes(1);
    expect(mockVoiceTypingInit).toHaveBeenCalledTimes(1);
    expect(mockRunHealthCheck).toHaveBeenCalledTimes(1);
    expect(mockLoggerError).toHaveBeenCalledWith(
      '[Startup] Failed to load automation runtime:',
      expect.any(Error),
    );
  });

  it('loads the task ledger before recovery state and automation runtime', async () => {
    const callOrder: string[] = [];
    mockLoadTasks.mockImplementation(async () => {
      callOrder.push('task-ledger');
    });
    mockLoadRecovery.mockImplementation(async () => {
      callOrder.push('recovery');
    });
    mockLoadAndStart.mockImplementation(async () => {
      callOrder.push('automation');
    });

    await startAppRuntimeServices();

    expect(callOrder).toEqual(['task-ledger', 'recovery', 'automation']);
  });
});
