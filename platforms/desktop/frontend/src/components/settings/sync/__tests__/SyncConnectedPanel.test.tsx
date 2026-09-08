import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SyncConnectedPanel } from '../SyncConnectedPanel';
import type { SyncStatusSnapshot } from '../../../../types/sync';
import { useDialogStore } from '../../../../stores/dialogStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
      let val = options?.defaultValue ?? key;
      if (options) {
        for (const [k, v] of Object.entries(options)) {
          val = val.replace(new RegExp(`{{${k}}}`, 'g'), String(v));
        }
      }
      return val;
    },
    i18n: { language: 'en' },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../../../services/tauri/sync', () => ({
  getSyncPairingInfo: vi.fn().mockResolvedValue({
    providerId: 'webdav',
    vaultId: 'vault-xyz',
    serverUrl: 'https://dav.example.com/',
  }),
}));

const mockStatus: SyncStatusSnapshot = {
  state: 'idle',
  providerId: 'webdav',
  vaultId: 'vault-xyz',
  preset: 'standard',
  lastSuccessAtMs: Date.now() - 10000,
  pendingOperationCount: 0,
  conflictCount: 0,
  nextRetryAtMs: null,
  lastError: null,
};

describe('SyncConnectedPanel', () => {
  const onChangeMasterPassword = vi.fn().mockResolvedValue(undefined);
  const onChangePreset = vi.fn().mockResolvedValue(undefined);
  const onCopyRecoveryKey = vi.fn().mockResolvedValue(undefined);
  const onDisconnect = vi.fn().mockResolvedValue(undefined);
  const onExportRecoveryKey = vi.fn().mockResolvedValue(undefined);
  const onGenerateRecoveryKey = vi.fn().mockResolvedValue(undefined);
  const onLock = vi.fn().mockResolvedValue(undefined);
  const onRunNow = vi.fn().mockResolvedValue(undefined);
  const onSetPaused = vi.fn().mockResolvedValue(undefined);
  const onUnlock = vi.fn().mockResolvedValue(undefined);
  const onUnlockWithRecovery = vi.fn().mockResolvedValue(undefined);
  const onDeleteRecoveryKey = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders horizontal scenario cards for preset selection and triggers apply', () => {
    render(
      <SyncConnectedPanel
        busyAction={null}
        recoveryKey={null}
        status={mockStatus}
        onChangeMasterPassword={onChangeMasterPassword}
        onChangePreset={onChangePreset}
        onCopyRecoveryKey={onCopyRecoveryKey}
        onDisconnect={onDisconnect}
        onExportRecoveryKey={onExportRecoveryKey}
        onGenerateRecoveryKey={onGenerateRecoveryKey}
        onLock={onLock}
        onRunNow={onRunNow}
        onSetPaused={onSetPaused}
        onUnlock={onUnlock}
        onUnlockWithRecovery={onUnlockWithRecovery}
        onDeleteRecoveryKey={onDeleteRecoveryKey}
      />,
    );

    // Should find the 3 scenario card buttons
    expect(screen.getByRole('button', { name: /Content/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Standard/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Full/i })).toBeDefined();

    // Standard should be active initially
    const standardBtn = screen.getByRole('button', { name: /Standard/i });
    expect(standardBtn.className).toContain('active');

    // Click 'Content'
    const contentBtn = screen.getByRole('button', { name: /Content/i });
    fireEvent.click(contentBtn);

    // Apply button should appear
    const applyBtn = screen.getByRole('button', { name: /Apply preset change/i });
    expect(applyBtn).toBeDefined();

    fireEvent.click(applyBtn);
    expect(onChangePreset).toHaveBeenCalledWith('content');
  });

  it('handles recovery key masking, partial view, copy, and delete with confirmation', async () => {
    const rawKey = 'secret-long-recovery-key-token-abcd-1234';
    useDialogStore.setState({
      confirm: vi.fn().mockResolvedValue(true) as never,
    });

    render(
      <SyncConnectedPanel
        busyAction={null}
        recoveryKey={rawKey}
        status={mockStatus}
        onChangeMasterPassword={onChangeMasterPassword}
        onChangePreset={onChangePreset}
        onCopyRecoveryKey={onCopyRecoveryKey}
        onDisconnect={onDisconnect}
        onExportRecoveryKey={onExportRecoveryKey}
        onGenerateRecoveryKey={onGenerateRecoveryKey}
        onLock={onLock}
        onRunNow={onRunNow}
        onSetPaused={onSetPaused}
        onUnlock={onUnlock}
        onUnlockWithRecovery={onUnlockWithRecovery}
        onDeleteRecoveryKey={onDeleteRecoveryKey}
      />,
    );

    // Recovery card should be rendered
    expect(screen.getByText(/Active Recovery Key/i)).toBeDefined();

    // Initially masked with dots, not showing plaintext
    expect(screen.queryByText(rawKey)).toBeNull();
    expect(screen.getByText(/••••••••••••••••••••••••/)).toBeDefined();

    // Click View key to partially reveal
    const viewBtn = screen.getByRole('button', { name: /View key/i });
    fireEvent.click(viewBtn);

    // Should still NOT show full plaintext key
    expect(screen.queryByText(rawKey)).toBeNull();
    // But shows partial key with prefix and suffix
    expect(screen.getByText(/secret••••••••••••••••1234/)).toBeDefined();

    // Toggle hide
    const hideBtn = screen.getByRole('button', { name: /Hide key/i });
    fireEvent.click(hideBtn);
    expect(screen.getByText(/••••••••••••••••••••••••/)).toBeDefined();

    // Copy button calls onCopyRecoveryKey
    const copyBtn = screen.getByRole('button', { name: /Copy/i });
    fireEvent.click(copyBtn);
    expect(onCopyRecoveryKey).toHaveBeenCalled();

    // Click Delete button
    const deleteBtn = screen.getByRole('button', { name: /Delete key/i });
    fireEvent.click(deleteBtn);

    expect(useDialogStore.getState().confirm).toHaveBeenCalled();
    await waitFor(() => {
      expect(onDeleteRecoveryKey).toHaveBeenCalled();
    });
  });

  it('allows disconnecting/unbinding when vault is locked', async () => {
    const lockedStatus: SyncStatusSnapshot = {
      ...mockStatus,
      state: 'locked',
    };
    render(
      <SyncConnectedPanel
        busyAction={null}
        recoveryKey={null}
        status={lockedStatus}
        onChangeMasterPassword={onChangeMasterPassword}
        onChangePreset={onChangePreset}
        onCopyRecoveryKey={onCopyRecoveryKey}
        onDisconnect={onDisconnect}
        onExportRecoveryKey={onExportRecoveryKey}
        onGenerateRecoveryKey={onGenerateRecoveryKey}
        onLock={onLock}
        onRunNow={onRunNow}
        onSetPaused={onSetPaused}
        onUnlock={onUnlock}
        onUnlockWithRecovery={onUnlockWithRecovery}
        onDeleteRecoveryKey={onDeleteRecoveryKey}
      />,
    );

    // Verify Disconnect button is present even in locked state
    const disconnectBtn = screen.getByRole('button', { name: /Disconnect/i });
    expect(disconnectBtn).toBeDefined();

    fireEvent.click(disconnectBtn);
    expect(onDisconnect).toHaveBeenCalled();
  });
});
