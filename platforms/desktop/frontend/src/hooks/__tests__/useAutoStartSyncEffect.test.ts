import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type * as TauriAppModule from '../../services/tauri/app';
import { setTestConfig } from '../../test-utils/configTestUtils';
import { useAutoStartSyncEffect } from '../useAutoStartSyncEffect';

const mocks = vi.hoisted(() => ({
  setAutoStart: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../services/tauri/app', async () => {
  const actual = await vi.importActual<typeof TauriAppModule>('../../services/tauri/app');
  return {
    ...actual,
    setAutoStart: (...args: unknown[]) => mocks.setAutoStart(...args),
  };
});

describe('useAutoStartSyncEffect', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setTestConfig();
  });

  it('does not sync before loaded', () => {
    setTestConfig({ autoStart: true });

    renderHook(() => useAutoStartSyncEffect(false));

    expect(mocks.setAutoStart).not.toHaveBeenCalled();
  });

  it('syncs autoStart value to native command when loaded', async () => {
    setTestConfig({ autoStart: true });

    renderHook(() => useAutoStartSyncEffect(true));

    await waitFor(() => {
      expect(mocks.setAutoStart).toHaveBeenCalledWith(true);
    });
  });

  it('syncs false when autoStart is disabled or undefined', async () => {
    setTestConfig({ autoStart: false });

    renderHook(() => useAutoStartSyncEffect(true));

    await waitFor(() => {
      expect(mocks.setAutoStart).toHaveBeenCalledWith(false);
    });
  });
});
