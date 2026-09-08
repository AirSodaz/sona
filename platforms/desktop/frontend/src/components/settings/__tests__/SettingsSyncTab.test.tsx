import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { SettingsSyncTab } from '../SettingsSyncTab';
import { useSyncStatusStore } from '../../../stores/syncStatusStore';
import { DEFAULT_CONFIG, useConfigStore } from '../../../stores/configStore';
import { DISABLED_SYNC_STATUS, type SyncStatusSnapshot } from '../../../types/sync';
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) =>
      (options?.defaultValue as string | undefined) ?? key,
    i18n: { language: 'en' },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));
const testContext = vi.hoisted(() => ({
  createSyncVault: vi.fn(),
  joinSyncVault: vi.fn(),
  previewSyncJoin: vi.fn(),
  testWebDavSyncProvider: vi.fn(),
  runSyncNow: vi.fn().mockResolvedValue({}),
  setSyncPaused: vi.fn(),
}));

vi.mock('../../../services/tauri/sync', () => ({
  createSyncVault: (...args: unknown[]) => testContext.createSyncVault(...args),
  joinSyncVault: (...args: unknown[]) => testContext.joinSyncVault(...args),
  previewSyncJoin: (...args: unknown[]) => testContext.previewSyncJoin(...args),
  testWebDavSyncProvider: (...args: unknown[]) => testContext.testWebDavSyncProvider(...args),
  runSyncNow: (...args: unknown[]) => testContext.runSyncNow(...args),
  setSyncPaused: (...args: unknown[]) => testContext.setSyncPaused(...args),
  changeSyncPreset: vi.fn(),
  changeSyncMasterPassword: vi.fn(),
  discoverWebDavSyncVaults: vi.fn().mockResolvedValue([]),
  generateSyncRecoveryKey: vi.fn(),
  getSyncPairingInfo: vi.fn().mockResolvedValue(null),
  disconnectSyncVault: vi.fn(),
  lockSyncVault: vi.fn(),
  unlockSyncVault: vi.fn(),
  unlockSyncVaultWithRecovery: vi.fn(),
  listSyncConflicts: vi.fn().mockResolvedValue([]),
}));
vi.mock('../../../services/syncRuntimeService', () => ({
  syncRuntimeService: {
    refreshStatus: vi.fn().mockResolvedValue(null),
    requestSync: vi.fn(),
  },
}));

describe('SettingsSyncTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        enableCloudSync: false,
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

  it('renders switch off by default and shows disabled notice card', () => {
    render(<SettingsSyncTab isVisible={true} />);
    expect(screen.getAllByText(/Cloud Sync|云同步/i).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Cloud sync is currently turned off|云同步功能当前已关闭/i)).toBeDefined();

    // Toggle switch
    const switchEl = screen.getByRole('switch');
    fireEvent.click(switchEl);

    expect(useConfigStore.getState().config.enableCloudSync).toBe(true);
  });

  it('renders setup form when enabled but not configured', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        enableCloudSync: true,
      },
    });

    render(<SettingsSyncTab isVisible={true} />);

    expect(screen.getByLabelText(/Server URL|服务器地址/i)).toBeDefined();
    expect(screen.getByLabelText(/^Master password$|^主密码$/i)).toBeDefined();
    expect(screen.getByRole('button', { name: /Save & Enable Sync|保存并开启同步/i })).toBeDefined();
    expect(screen.getAllByText(/Nutstore|坚果云/i).length).toBeGreaterThanOrEqual(1);
  });
  it('renders status overview and security sections when connected', () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        enableCloudSync: true,
      },
    });
    setStatus({
      state: 'idle',
      providerId: 'webdav',
      vaultId: 'vault-active-123',
      lastSuccessAtMs: Date.now() - 5000,
      preset: 'standard',
    });

    render(<SettingsSyncTab isVisible={true} />);

    expect(screen.getByText(/vault-active-123/i)).toBeDefined();
    expect(screen.getByRole('button', { name: /Sync now|立即同步/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Pause|暂停/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Pair new device|配对新设备/i })).toBeDefined();
  });
});
