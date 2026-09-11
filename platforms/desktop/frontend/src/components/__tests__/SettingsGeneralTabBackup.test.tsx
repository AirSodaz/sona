import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSyncStatusStore } from '../../stores/syncStatusStore';
import { DISABLED_SYNC_STATUS, type SyncStatusSnapshot } from '../../types/sync';
import { BackupSettingsSection } from '../settings/backup/BackupSettingsSection';

const testContext = vi.hoisted(() => ({
  alert: vi.fn().mockResolvedValue(undefined),
  confirm: vi.fn().mockResolvedValue(true),
  showError: vi.fn().mockResolvedValue(undefined),
  createVault: vi.fn(),
  previewJoin: vi.fn(),
  joinVault: vi.fn(),
  listLegacy: vi.fn(),
  unlock: vi.fn(),
  unlockRecovery: vi.fn(),
  getSetting: vi.fn().mockResolvedValue(null),
  setSetting: vi.fn().mockResolvedValue(undefined),
  refreshStatus: vi.fn().mockResolvedValue(null),
  transcriptState: { isRecording: false },
  batchState: { queueItems: [], isQueueProcessing: false },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const value = String(options?.defaultValue ?? key);
      return value.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? ''));
    },
  }),
}));

vi.mock('../Dropdown', () => ({
  Dropdown: ({ id, value, onChange, options, disabled }: any) => (
    <select
      id={id}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      disabled={disabled}
    >
      {options.map((option: any) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock('../../stores/dialogStore', () => ({
  useDialogStore: (selector: any) =>
    selector({
      alert: testContext.alert,
      confirm: testContext.confirm,
      showError: testContext.showError,
    }),
}));

vi.mock('../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: (selector: any) => selector(testContext.transcriptState),
}));

vi.mock('../../stores/batchQueueStore', () => ({
  useBatchQueueStore: (selector: any) => selector(testContext.batchState),
}));

vi.mock('../../services/storageService', () => ({
  STORE_KEY_BACKUP_WEBDAV: 'sona-backup-webdav',
  settingsStore: {
    get: (...args: unknown[]) => testContext.getSetting(...args),
    set: (...args: unknown[]) => testContext.setSetting(...args),
    save: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../services/syncRuntimeService', () => ({
  syncRuntimeService: {
    refreshStatus: (...args: unknown[]) => testContext.refreshStatus(...args),
    requestSync: vi.fn(),
  },
}));

vi.mock('../../services/tauri/platform/dialog', () => ({
  saveDialog: vi.fn().mockResolvedValue(null),
}));

vi.mock('../../services/tauri/platform/fs', () => ({
  writeFile: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../services/backupService', () => ({
  applyImportBackup: vi.fn().mockResolvedValue(undefined),
  disposePreparedImport: vi.fn().mockResolvedValue(undefined),
  backupService: {
    exportBackup: vi.fn().mockResolvedValue(null),
    prepareImportBackup: vi.fn().mockResolvedValue(null),
  },
}));

vi.mock('../../services/transcriptSnapshotService', () => ({
  transcriptSnapshotService: {
    buildDiff: vi.fn().mockResolvedValue({ rows: [], changedCount: 0 }),
  },
}));

vi.mock('../../services/tauri/sync', () => ({
  changeSyncMasterPassword: vi.fn().mockResolvedValue(undefined),
  changeSyncPreset: vi.fn(),
  createSyncVault: (...args: unknown[]) => testContext.createVault(...args),
  disconnectSyncVault: vi.fn(),
  discoverWebDavSyncVaults: vi.fn().mockResolvedValue([]),
  getSyncPairingInfo: vi.fn().mockResolvedValue(null),
  generateSyncRecoveryKey: vi.fn().mockResolvedValue('recovery-key'),
  getSyncConflict: vi.fn(),
  joinSyncVault: (...args: unknown[]) => testContext.joinVault(...args),
  listLegacyRemoteBackups: (...args: unknown[]) => testContext.listLegacy(...args),
  listSyncConflicts: vi.fn().mockResolvedValue([]),
  lockSyncVault: vi.fn(),
  prepareLegacyRemoteBackupImport: vi.fn(),
  previewSyncJoin: (...args: unknown[]) => testContext.previewJoin(...args),
  resolveSyncConflict: vi.fn(),
  runSyncNow: vi.fn(),
  setSyncPaused: vi.fn(),
  testWebDavSyncProvider: vi.fn().mockResolvedValue({ id: 'webdav', displayName: 'WebDAV' }),
  unlockSyncVault: (...args: unknown[]) => testContext.unlock(...args),
  unlockSyncVaultWithRecovery: (...args: unknown[]) => testContext.unlockRecovery(...args),
}));

function status(patch: Partial<SyncStatusSnapshot>): SyncStatusSnapshot {
  return {
    ...DISABLED_SYNC_STATUS,
    state: 'idle',
    providerId: 'webdav',
    vaultId: 'vault-1',
    preset: 'standard',
    ...patch,
  };
}

function setStatus(snapshot: SyncStatusSnapshot): void {
  useSyncStatusStore.setState({
    snapshot,
    isLoaded: true,
    lastRunResult: null,
  });
}

describe('Sync & Recovery settings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testContext.confirm.mockResolvedValue(true);
    testContext.getSetting.mockResolvedValue(null);
    testContext.listLegacy.mockResolvedValue({ entries: [], credentialsMigrated: false });
    testContext.transcriptState.isRecording = false;
    testContext.batchState.queueItems = [];
    setStatus(DISABLED_SYNC_STATUS);
  });

  it('replaces the archive upload UI with unified sync setup', () => {
    render(<BackupSettingsSection />);

    screen.getByText('Sync & Recovery');
    screen.getByLabelText('Server URL');
    screen.getByLabelText('Username');
    screen.getByLabelText('Password');
    screen.getByLabelText('Master password');
    screen.getByRole('button', { name: 'Save & Enable Sync' });
    expect(screen.queryByRole('button', { name: 'Upload Backup' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Refresh Cloud Backups' })).toBeNull();
  });

  it('allows joining an explicit remote vault via advanced options', async () => {
    testContext.joinVault.mockResolvedValue({
      pulledSegmentCount: 1,
      pulledCheckpointCount: 0,
      pushedSegmentCount: 1,
      appliedOperationCount: 12,
      publishedOperationCount: 7,
      conflictCount: 2,
      checkpointPublished: false,
    });
    render(<BackupSettingsSection />);

    fireEvent.change(screen.getByLabelText('Server URL'), {
      target: { value: 'https://dav.example.com' },
    });
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'sona' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'provider-secret' } });
    fireEvent.change(screen.getByLabelText('Master password'), { target: { value: 'x' } });

    // Open advanced options and enter Vault ID
    fireEvent.click(screen.getByRole('button', { name: /Advanced Settings|高级设置/i }));
    fireEvent.change(screen.getByLabelText('Vault ID'), { target: { value: 'vault-remote' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save & Enable Sync' }));

    await waitFor(() =>
      expect(testContext.joinVault).toHaveBeenCalledWith(
        expect.objectContaining({
          vaultId: 'vault-remote',
          masterPassword: 'x',
        })
      )
    );
  });

  it('shows only unlock controls while the vault is locked', async () => {
    testContext.unlock.mockResolvedValue(status({ state: 'idle' }));
    setStatus(status({ state: 'locked' }));
    render(<BackupSettingsSection />);

    screen.getByText('Sync vault locked');
    expect(screen.queryByRole('button', { name: 'Sync now' })).toBeNull();
    fireEvent.change(screen.getByLabelText('WebDAV password'), {
      target: { value: 'provider-secret' },
    });
    fireEvent.change(screen.getByLabelText('Master password'), { target: { value: 'x' } });
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }));

    await waitFor(() =>
      expect(testContext.unlock).toHaveBeenCalledWith({
        providerPassword: 'provider-secret',
        masterPassword: 'x',
      })
    );
  });

  it('renders sync status and quick actions cleanly', () => {
    setStatus(
      status({
        pendingOperationCount: 4,
        conflictCount: 2,
        lastSuccessAtMs: 1_750_000_000_000,
      })
    );
    render(<BackupSettingsSection />);

    screen.getByRole('button', { name: 'Sync now' });
    screen.getByRole('button', { name: 'Pause' });
    screen.getByText('Pending upload');
    screen.getByText('4');
    expect(screen.queryByText('Advanced recovery')).toBeNull();
  });
});
