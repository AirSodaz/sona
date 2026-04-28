import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppInitialization } from '../useAppInitialization';

const mockHydrateAppStartupState = vi.fn();
const mockStartAppRuntimeServices = vi.fn();
const mockLoggerError = vi.fn();

vi.mock('../../services/startup/hydration', () => ({
  hydrateAppStartupState: (...args: unknown[]) => mockHydrateAppStartupState(...args),
}));

vi.mock('../../services/startup/runtime', () => ({
  startAppRuntimeServices: (...args: unknown[]) => mockStartAppRuntimeServices(...args),
}));

vi.mock('../useThemeEffect', () => ({
  useThemeEffect: vi.fn(),
}));

vi.mock('../useFontEffect', () => ({
  useFontEffect: vi.fn(),
}));

vi.mock('../useConfigPersistence', () => ({
  useConfigPersistence: vi.fn(),
}));

vi.mock('../useTraySyncEffect', () => ({
  useTraySyncEffect: vi.fn(),
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    error: (...args: unknown[]) => mockLoggerError(...args),
  },
}));

describe('useAppInitialization', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('marks the app as loaded right after hydration without waiting for background runtime startup', async () => {
    let resolveHydration: (() => void) | null = null;
    mockHydrateAppStartupState.mockImplementation(() => new Promise<void>((resolve) => {
      resolveHydration = resolve;
    }));
    mockStartAppRuntimeServices.mockImplementation(() => new Promise<void>(() => undefined));

    const { result } = renderHook(() => useAppInitialization());

    expect(result.current.isLoaded).toBe(false);

    await act(async () => {
      resolveHydration?.();
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    expect(mockStartAppRuntimeServices).toHaveBeenCalledTimes(1);
  });

  it('keeps the app loaded even if background runtime startup rejects', async () => {
    mockHydrateAppStartupState.mockResolvedValue(undefined);
    mockStartAppRuntimeServices.mockRejectedValue(new Error('runtime failed'));

    const { result } = renderHook(() => useAppInitialization());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    await waitFor(() => {
      expect(mockLoggerError).toHaveBeenCalledWith(
        '[Startup] Failed to start background runtime services:',
        expect.any(Error),
      );
    });
  });
});
