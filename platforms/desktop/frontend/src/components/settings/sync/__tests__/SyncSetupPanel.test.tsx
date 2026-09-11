import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import type { DiscoveredVaultSummary } from '../../../../types/sync';
import { SyncSetupPanel } from '../SyncSetupPanel';
import { encodeSyncPairingToken } from '../syncPairing';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      let value = String(options?.defaultValue ?? key);
      if (options) {
        for (const [k, v] of Object.entries(options)) {
          value = value.replace(new RegExp(`{{${k}}}`, 'g'), String(v));
        }
      }
      return value;
    },
  }),
}));

describe('SyncSetupPanel Minimal Flow (Scheme A)', () => {
  let onCreate: Mock;
  let onJoin: Mock;
  let onPreviewJoin: Mock;
  let onTestProvider: Mock;

  beforeEach(() => {
    onCreate = vi.fn().mockResolvedValue({
      vaultId: 'default',
      deviceId: 'dev-1',
      recoveryKey: null,
      status: { state: 'idle' },
    });
    onJoin = vi.fn().mockResolvedValue({ appliedOperationCount: 0 });
    onPreviewJoin = vi.fn().mockResolvedValue({
      localOperationCount: 0,
      remoteOperationCount: 5,
      projectedConflictCount: 0,
    });
    onTestProvider = vi.fn().mockResolvedValue({
      id: 'webdav',
      displayName: 'WebDAV Server',
    });
  });

  function fillFields(password = 'master-password-123') {
    const urlInput = screen.getByLabelText(/Server URL/i);
    const userInput = screen.getByLabelText(/Username/i);
    const passInput = screen.getByLabelText(/^Password$/i);
    const masterInput = screen.getByLabelText(/^Master password$/i);
    const confirmInput = screen.getByLabelText(/^Confirm password$/i);

    fireEvent.change(urlInput, { target: { value: 'https://dav.example.com/' } });
    fireEvent.change(userInput, { target: { value: 'alice' } });
    fireEvent.change(passInput, { target: { value: 'secret' } });
    fireEvent.change(masterInput, { target: { value: password } });
    fireEvent.change(confirmInput, { target: { value: password } });
  }

  it('tests provider connection when clicking Test connection button', async () => {
    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
      />
    );

    fillFields();

    const testBtn = screen.getByRole('button', { name: /Test connection|测试连接/i });
    fireEvent.click(testBtn);

    await waitFor(() => {
      expect(onTestProvider).toHaveBeenCalledWith(
        expect.objectContaining({
          serverUrl: 'https://dav.example.com/',
          username: 'alice',
          password: 'secret',
        })
      );
    });
  });

  it('initializes default vault automatically when 0 vaults exist on server', async () => {
    const onDiscoverVaults = vi.fn().mockResolvedValue([]);

    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
        onDiscoverVaults={onDiscoverVaults}
      />
    );

    fillFields();

    const saveBtn = screen.getByRole('button', { name: /Save & Enable Sync|保存并开启同步/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(onDiscoverVaults).toHaveBeenCalledTimes(1);
    });

    await waitFor(() => {
      expect(onCreate).toHaveBeenCalledWith(
        expect.objectContaining({
          vaultId: 'default',
          masterPassword: 'master-password-123',
        })
      );
    });
  });

  it('automatically joins existing vault when exactly 1 vault exists on server', async () => {
    const existingVaults: DiscoveredVaultSummary[] = [
      { vaultId: 'existing-vault-abc', preset: 'standard' },
    ];
    const onDiscoverVaults = vi.fn().mockResolvedValue(existingVaults);

    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
        onDiscoverVaults={onDiscoverVaults}
      />
    );

    fillFields();

    const saveBtn = screen.getByRole('button', { name: /Save & Enable Sync|保存并开启同步/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(onDiscoverVaults).toHaveBeenCalledTimes(1);
    });

    await waitFor(() => {
      expect(onJoin).toHaveBeenCalledWith(
        expect.objectContaining({
          vaultId: 'existing-vault-abc',
          masterPassword: 'master-password-123',
        })
      );
    });
  });

  it('presents selection modal when multiple vaults are discovered and joins chosen vault', async () => {
    const multipleVaults: DiscoveredVaultSummary[] = [
      { vaultId: 'personal-vault', preset: 'standard' },
      { vaultId: 'work-vault', preset: 'full' },
    ];
    const onDiscoverVaults = vi.fn().mockResolvedValue(multipleVaults);

    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
        onDiscoverVaults={onDiscoverVaults}
      />
    );

    fillFields();

    const saveBtn = screen.getByRole('button', { name: /Save & Enable Sync|保存并开启同步/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(onDiscoverVaults).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/Multiple Sync Vaults Detected|检测到多个同步库/i)).toBeDefined();
      expect(screen.getByText('personal-vault')).toBeDefined();
      expect(screen.getByText('work-vault')).toBeDefined();
    });

    // Select work-vault
    fireEvent.click(screen.getByText('work-vault'));

    // Click Confirm
    const confirmBtn = screen.getByRole('button', { name: /Confirm|确认/i });
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(onJoin).toHaveBeenCalledWith(
        expect.objectContaining({
          vaultId: 'work-vault',
          masterPassword: 'master-password-123',
        })
      );
    });
  });

  it('imports parameters via pairing code modal', async () => {
    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
      />
    );

    // Click quick import button
    const importBtn = screen.getByRole('button', { name: /Import pairing code|从配对口令导入/i });
    fireEvent.click(importBtn);
    expect(
      screen.getByRole('heading', { name: /Import Pairing Code|导入设备配对口令/i })
    ).toBeDefined();
    const token = encodeSyncPairingToken(
      {
        serverUrl: 'https://dav.paired.com/remote/',
        remoteRoot: 'MySync',
        username: 'paired-user',
        password: 'paired-password',
      },
      'paired-vault-123',
      true
    );

    const textarea = screen.getByPlaceholderText(/sonasync:\/\/v1\?data=/i);
    fireEvent.change(textarea, { target: { value: token } });

    // Click Import in modal
    const doImportBtn = screen.getByRole('button', { name: /^Import$|^导入$/i });
    fireEvent.click(doImportBtn);

    // Modal closes, fields populated
    const urlInput = screen.getByLabelText(/Server URL/i) as HTMLInputElement;
    const userInput = screen.getByLabelText(/Username/i) as HTMLInputElement;

    expect(urlInput.value).toBe('https://dav.paired.com/remote/');
    expect(userInput.value).toBe('paired-user');
  });
  it('displays inline error inside pairing modal when invalid token is entered', async () => {
    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
      />
    );

    const importBtn = screen.getByRole('button', { name: /Import pairing code|从配对口令导入/i });
    fireEvent.click(importBtn);

    const textarea = screen.getByPlaceholderText(/sonasync:\/\/v1\?data=/i);
    fireEvent.change(textarea, { target: { value: 'invalid-token-string' } });

    const doImportBtn = screen.getByRole('button', { name: /^Import$|^导入$/i });
    fireEvent.click(doImportBtn);

    // Modal stays open and displays inline error
    expect(screen.getByRole('alert')).toBeDefined();
    expect(screen.getByText(/Invalid pairing token format|配对口令格式不正确/i)).toBeDefined();

    // Closing modal dismisses error
    const cancelBtn = screen.getByRole('button', { name: /Cancel|取消/i });
    fireEvent.click(cancelBtn);
  });
  it('allows changing sync preset via horizontal cards and toggling recovery key creation', async () => {
    const onDiscoverVaults = vi.fn().mockResolvedValue([]);
    render(
      <SyncSetupPanel
        busyAction={null}
        onCreate={onCreate}
        onJoin={onJoin}
        onPreviewJoin={onPreviewJoin}
        onTestProvider={onTestProvider}
        onDiscoverVaults={onDiscoverVaults}
      />
    );

    fillFields();

    // Open advanced options accordion
    const advancedTitle = screen.getByText(/Advanced Settings|高级设置/i);
    fireEvent.click(advancedTitle);

    // Select 'Full workspace' preset
    const fullCard = screen.getByRole('button', { name: /Full workspace/i });
    fireEvent.click(fullCard);

    // Toggle recovery key switch
    const recoverySwitch = screen.getByLabelText(/Emergency Recovery Key/i);
    fireEvent.click(recoverySwitch);

    const saveBtn = screen.getByRole('button', { name: /Save & Enable Sync|保存并开启同步/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(onCreate).toHaveBeenCalledWith(
        expect.objectContaining({
          preset: 'full',
          createRecoveryKey: false,
        })
      );
    });
  });
});
