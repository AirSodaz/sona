import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setTestConfig } from '../../test-utils/configTestUtils';
import { getLoggerLevel, setLoggerLevel } from '../../utils/logger';
import { useLogLevelSyncEffect } from '../useLogLevelSyncEffect';

const mocks = vi.hoisted(() => ({
  setLogLevel: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../services/tauri/app', async () => {
  const actual = await vi.importActual<typeof import('../../services/tauri/app')>('../../services/tauri/app');
  return {
    ...actual,
    setLogLevel: (...args: unknown[]) => mocks.setLogLevel(...args),
  };
});

describe('useLogLevelSyncEffect', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setTestConfig();
    setLoggerLevel('info');
  });

  it('updates the frontend logger level before native hydration', () => {
    setTestConfig({ logLevel: 'warn' });

    renderHook(() => useLogLevelSyncEffect(false));

    expect(getLoggerLevel()).toBe('warn');
    expect(mocks.setLogLevel).not.toHaveBeenCalled();
  });

  it('syncs the hydrated config log level to the native runtime', async () => {
    setTestConfig({ logLevel: 'debug' });

    renderHook(() => useLogLevelSyncEffect(true));

    expect(getLoggerLevel()).toBe('debug');
    await waitFor(() => {
      expect(mocks.setLogLevel).toHaveBeenCalledWith('debug');
    });
  });
});
