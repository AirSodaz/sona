import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useSyncStatusStore } from '../../stores/syncStatusStore';
import { DISABLED_SYNC_STATUS, type SyncStatusSnapshot } from '../../types/sync';
import { SyncHeaderPill } from '../SyncHeaderPill';

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
    const btn = screen.getByRole('button', { name: /Cloud Sync: Not configured/i });
    expect(btn.classList.contains('btn')).toBe(true);
    expect(btn.classList.contains('btn-icon')).toBe(true);
    expect(btn.classList.contains('sync-header-trigger')).toBe(true);
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: Not configured');
    expect(btn.getAttribute('data-tooltip-pos')).toBe('bottom-left');
    expect(btn.querySelector('.sync-status-dot.is-disabled')).not.toBeNull();

    // Click to open popover
    fireEvent.click(btn);
    expect(btn.hasAttribute('data-tooltip')).toBe(false);
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
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: Synced (Just now)');
    expect(btn.querySelector('.sync-status-dot.is-idle')).not.toBeNull();

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
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: 2 conflicts found');
    expect(btn.querySelector('.sync-status-dot.is-conflict')).not.toBeNull();

    fireEvent.click(btn);
    const conflictBanner = screen.getByText(/2 content conflicts need review/i);
    fireEvent.click(conflictBanner);
    expect(onOpenSyncSettings).toHaveBeenCalled();
  });

  it('renders syncing state with pulsing dot and tooltip', () => {
    setStatus({
      state: 'syncing',
      providerId: 'webdav',
      vaultId: 'vault-123',
    });

    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: Syncing...');
    expect(btn.querySelector('.sync-status-dot.is-syncing')).not.toBeNull();
  });

  it('renders error state with error dot and tooltip', () => {
    setStatus({
      state: 'error',
      providerId: 'webdav',
      vaultId: 'vault-123',
    });

    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: Sync error');
    expect(btn.querySelector('.sync-status-dot.is-error')).not.toBeNull();
  });

  it('renders locked state with warning dot and tooltip', () => {
    setStatus({
      state: 'locked',
      providerId: 'webdav',
      vaultId: 'vault-123',
    });

    render(<SyncHeaderPill onOpenSyncSettings={onOpenSyncSettings} />);
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: Locked');
    expect(btn.querySelector('.sync-status-dot.is-locked')).not.toBeNull();
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
    const btn = screen.getByRole('button', { name: /Cloud Sync/i });
    expect(btn.getAttribute('data-tooltip')).toBe('Cloud Sync: Paused');
    expect(btn.querySelector('.sync-status-dot.is-paused')).not.toBeNull();
    fireEvent.click(btn);

    const resumeBtn = screen.getByRole('button', { name: /Resume/i });
    fireEvent.click(resumeBtn);
    expect(testContext.setSyncPaused).toHaveBeenCalledWith(false);
  });
});
