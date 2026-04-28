import { beforeEach, describe, expect, it, vi } from 'vitest';
import { startAppRuntimeServices } from '../runtime';

const mockLoadAndStart = vi.fn();
const mockLlmUsageInit = vi.fn();
const mockVoiceTypingInit = vi.fn();
const mockRunHealthCheck = vi.fn();
const mockLoggerError = vi.fn();

vi.mock('../../../stores/automationStore', () => ({
  useAutomationStore: {
    getState: vi.fn(() => ({
      loadAndStart: (...args: unknown[]) => mockLoadAndStart(...args),
    })),
  },
}));

vi.mock('../../llmUsageService', () => ({
  llmUsageService: {
    init: (...args: unknown[]) => mockLlmUsageInit(...args),
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
    mockLoadAndStart.mockResolvedValue(undefined);
    mockLlmUsageInit.mockResolvedValue(undefined);
    mockVoiceTypingInit.mockReturnValue(undefined);
    mockRunHealthCheck.mockResolvedValue(undefined);
  });

  it('continues later runtime startup tasks when an earlier step fails', async () => {
    mockLoadAndStart.mockRejectedValue(new Error('automation failed'));

    await startAppRuntimeServices();

    expect(mockLoadAndStart).toHaveBeenCalledTimes(1);
    expect(mockLlmUsageInit).toHaveBeenCalledTimes(1);
    expect(mockVoiceTypingInit).toHaveBeenCalledTimes(1);
    expect(mockRunHealthCheck).toHaveBeenCalledTimes(1);
    expect(mockLoggerError).toHaveBeenCalledWith(
      '[Startup] Failed to load automation runtime:',
      expect.any(Error),
    );
  });
});
