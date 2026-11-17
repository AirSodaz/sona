import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { settingsStore } from '../../services/storageService';
import { getLoggerLevel, setLoggerLevel } from '../../utils/logger';
import { useAuxWindowLogLevel } from '../useAuxWindowLogLevel';

vi.mock('../../services/storageService', () => ({
  STORE_KEY_CONFIG: 'sona-config',
  settingsStore: {
    get: vi.fn(),
    onKeyChange: vi.fn(),
  },
}));

describe('useAuxWindowLogLevel', () => {
  let onKeyChangeCallback: ((config: unknown) => void) | null = null;
  const mockUnlisten = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    setLoggerLevel('info');
    onKeyChangeCallback = null;
    vi.mocked(settingsStore.onKeyChange).mockImplementation((_key, callback) => {
      onKeyChangeCallback = callback;
      return Promise.resolve(mockUnlisten);
    });
  });

  it('synchronizes log level on mount from stored config', async () => {
    vi.mocked(settingsStore.get).mockResolvedValue({
      logLevel: 'warn',
    } as any);

    renderHook(() => useAuxWindowLogLevel());

    await waitFor(() => {
      expect(getLoggerLevel()).toBe('warn');
    });
  });

  it('updates log level when settings store changes', async () => {
    vi.mocked(settingsStore.get).mockResolvedValue({
      logLevel: 'info',
    } as any);

    renderHook(() => useAuxWindowLogLevel());

    await waitFor(() => {
      expect(onKeyChangeCallback).not.toBeNull();
    });

    onKeyChangeCallback?.({ logLevel: 'debug' });
    expect(getLoggerLevel()).toBe('debug');
  });

  it('unsubscribes on unmount', async () => {
    vi.mocked(settingsStore.get).mockResolvedValue({
      logLevel: 'info',
    } as any);

    const { unmount } = renderHook(() => useAuxWindowLogLevel());

    await waitFor(() => {
      expect(settingsStore.onKeyChange).toHaveBeenCalled();
    });

    unmount();
    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });
});
