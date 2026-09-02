import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { SyncHeaderPill } from '../SyncHeaderPill';
import { useSyncStatusStore } from '../../stores/syncStatusStore';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { DISABLED_SYNC_STATUS, type SyncStatusSnapshot } from '../../types/sync';

const testContext = vi.hoisted(() => ({
  runSyncNow: vi.fn().mockResolvedValue({}),
  setSyncPaused: vi.fn(),
  refreshStatus: vi.fn().mockResolvedValue(null),
}));

vi.mock('../../services/tauri/sync', () => ({
  runSyncNow: (...args: unknown[]) => testContext.runSyncNow(...args),
  setSyncPaused: (...args: unknown[]) => testContext.setSyncPaused(...args),
}));

vi.mock('../../services/syncRuntimeService', () => ({
  syncRuntimeService: {
    refreshStatus: (...args: unknown[]) => testContext.refreshStatus(...args),
    requestSync: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const value = String(options?.defaultValue ?? key);
      return value.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? ''));
    },
    i18n: { language: 'en' },
  }),
}));

describe('SyncHeaderPill', () => {
  const onOpenSyncSettings = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        enableCloudSync: true,
      },
    });
    useSyncStatusStore.setState({
      snapshot: DISABLED_SYNC_STATUS,
      isLoaded: true,
      lastRunResult: null,
    });
  });

  function setStatus(snapshot: Partial<SyncStatusSnapshot>) {
    useSyncStatusStore.setState({
      snapshot: {
        ...DISABLED_SYNC_STATUS,
        ...snapshot,
      },
      isLoaded: true,
      lastRunResult: null,
    });
  }

  it('hides the pill completely when enableCloudSync is false (default)', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        enableCloudSync: false,
      },
    });

    const { container } = render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    expect(container.firstChild).toBeNull();
    expect(screen.queryByRole('button', { name: /Cloud Sync/i })).toBeNull();
  });

  it('renders disabled state and opens popover with enable CTA when switch is on', () => {
    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    expect(btn).toBeDefined();

    // Click to open popover
    fireEvent.click(btn);
    expect(screen.getByText(/Multi-device sync is not configured yet/i)).toBeDefined();

    const cta = screen.getByRole('button', { name: /Enable Cloud Sync/i });
    fireEvent.click(cta);
    expect(onOpenSyncSettings).toHaveBeenCalled();
  });

  it('renders synced state and allows manual sync', () => {
    setStatus({
      state: 'idle',
      providerId: 'webdav',
      vaultId: 'vault-123',
      lastSuccessAtMs: Date.now() - 5000,
      preset: 'standard',
    });

    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    fireEvent.click(btn);

    const syncNowBtn = screen.getByRole('button', { name: /Sync now/i });
    fireEvent.click(syncNowBtn);
    expect(testContext.runSyncNow).toHaveBeenCalled();
  });

  it('renders conflict badge when conflicts exist', () => {
    setStatus({
      state: 'idle',
      providerId: 'webdav',
      vaultId: 'vault-123',
      conflictCount: 2,
    });

    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    expect(screen.getByText(/2 conflicts/i)).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: /Cloud Sync/i }));
    const conflictBanner = screen.getByText(/2 content conflicts need review/i);
    fireEvent.click(conflictBanner);
    expect(onOpenSyncSettings).toHaveBeenCalled();
  });

  it('renders paused state and allows resume', () => {
    testContext.setSyncPaused.mockResolvedValue({
      ...DISABLED_SYNC_STATUS,
      state: 'idle',
    });

    setStatus({
      state: 'paused',
      providerId: 'webdav',
      vaultId: 'vault-123',
    });

    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    fireEvent.click(screen.getByRole('button', { name: /Cloud Sync/i }));

    const resumeBtn = screen.getByRole('button', { name: /Resume/i });
    fireEvent.click(resumeBtn);
    expect(testContext.setSyncPaused).toHaveBeenCalledWith(false);
  });
});
