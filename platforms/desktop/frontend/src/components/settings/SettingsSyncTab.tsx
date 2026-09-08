import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  Cloud,
  CloudOff,
} from 'lucide-react';
import {
  changeSyncMasterPassword,
  changeSyncPreset,
  createSyncVault,
  disconnectSyncVault,
  discoverWebDavSyncVaults,
  generateSyncRecoveryKey,
  joinSyncVault,
  lockSyncVault,
  previewSyncJoin,
  runSyncNow,
  setSyncPaused,
  testWebDavSyncProvider,
  unlockSyncVault,
  unlockSyncVaultWithRecovery,
} from '../../services/tauri/sync';
import { syncRuntimeService } from '../../services/syncRuntimeService';
import { saveDialog } from '../../services/tauri/platform/dialog';
import { writeFile } from '../../services/tauri/platform/fs';
import { useDialogStore } from '../../stores/dialogStore';
import { useSetConfig, useUIConfig } from '../../stores/configStore';
import { useSyncStatusStore } from '../../stores/syncStatusStore';
import type {
  SyncCreateRequest,
  SyncCreateResult,
  SyncJoinPreview,
  SyncPresetV1,
  SyncPreviewJoinRequest,
  SyncProviderDescriptor,
  SyncRunResult,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
  WebDavObjectStoreConfig,
} from '../../types/sync';
import {
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
  SettingsItem,
} from './SettingsLayout';
import { Switch } from '../Switch';
import { SyncConflictCenter } from './sync/SyncConflictCenter';
import { SyncConnectedPanel } from './sync/SyncConnectedPanel';
import { SyncSetupPanel } from './sync/SyncSetupPanel';
import './sync/SyncSettings.css';

interface SettingsSyncTabProps {
  isVisible?: boolean;
  isPrewarming?: boolean;
}

export function SettingsSyncTab({
  isVisible = true,
  isPrewarming = false,
}: SettingsSyncTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const alert = useDialogStore((state) => state.alert);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const setConfig = useSetConfig();
  const enableCloudSync = useUIConfig().enableCloudSync ?? false;

  const status = useSyncStatusStore((state) => state.snapshot);
  const isStatusLoaded = useSyncStatusStore((state) => state.isLoaded);
  const setSnapshot = useSyncStatusStore((state) => state.setSnapshot);
  const setLastRunResult = useSyncStatusStore((state) => state.setLastRunResult);

  const [busyAction, setBusyAction] = React.useState<string | null>(null);
  const [recoveryKey, setRecoveryKey] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (isVisible || isPrewarming) {
      void syncRuntimeService.refreshStatus();
    }
  }, [isPrewarming, isVisible]);

  const reportError = React.useCallback((action: string, cause: unknown) => showError({
    code: `sync.${action}_failed`,
    messageKey: 'errors.sync.operation_failed',
    cause,
    titleKey: 'settings.sync.error_title',
  }), [showError]);

  const runReturningAction = React.useCallback(async <T,>(
    action: string,
    task: () => Promise<T>,
  ): Promise<T> => {
    setBusyAction(action);
    try {
      return await task();
    } catch (error) {
      await reportError(action, error);
      throw error;
    } finally {
      setBusyAction(null);
    }
  }, [reportError]);

  const runAction = React.useCallback(async (
    action: string,
    task: () => Promise<void>,
  ): Promise<void> => {
    try {
      await runReturningAction(action, task);
    } catch {
      // Handled by runReturningAction
    }
  }, [runReturningAction]);

  const handleTestProvider = (provider: WebDavObjectStoreConfig): Promise<SyncProviderDescriptor> => runReturningAction(
    'test_provider',
    async () => {
      const descriptor = await testWebDavSyncProvider(provider);
      await alert(t('settings.sync.provider_ready', {
        defaultValue: '{{provider}} is ready for sync.',
        provider: descriptor.displayName,
      }), { variant: 'success' });
      return descriptor;
    },
  );

  const handleCreate = (request: SyncCreateRequest): Promise<SyncCreateResult> => runReturningAction(
    'create',
    async () => {
      const result = await createSyncVault(request);
      setSnapshot(result.status);
      setRecoveryKey(result.recoveryKey);
      await syncRuntimeService.refreshStatus();
      return result;
    },
  );

  const handlePreviewJoin = (request: SyncPreviewJoinRequest): Promise<SyncJoinPreview> => runReturningAction(
    'preview_join',
    () => previewSyncJoin(request),
  );

  const handleJoin = (request: SyncPreviewJoinRequest): Promise<SyncRunResult> => runReturningAction(
    'join',
    async () => {
      const result = await joinSyncVault(request);
      setLastRunResult(result);
      await syncRuntimeService.refreshStatus();
      return result;
    },
  );

  const handleUnlock = (request: SyncUnlockRequest): Promise<void> => runAction('unlock', async () => {
    setSnapshot(await unlockSyncVault(request));
    syncRuntimeService.requestSync(0);
  });

  const handleUnlockWithRecovery = (request: SyncUnlockRecoveryRequest): Promise<void> => runAction('unlock', async () => {
    setSnapshot(await unlockSyncVaultWithRecovery(request));
    syncRuntimeService.requestSync(0);
  });

  const handleRunNow = (): Promise<void> => runAction('run', async () => {
    setLastRunResult(await runSyncNow());
    await syncRuntimeService.refreshStatus();
  });

  const handleSetPaused = (paused: boolean): Promise<void> => runAction(paused ? 'pause' : 'resume', async () => {
    setSnapshot(await setSyncPaused(paused));
    if (!paused) {
      syncRuntimeService.requestSync(0);
    }
  });

  const handleLock = (): Promise<void> => runAction('lock', async () => {
    setSnapshot(await lockSyncVault());
  });

  const handleChangePreset = (preset: SyncPresetV1): Promise<void> => runAction('change_preset', async () => {
    setSnapshot(await changeSyncPreset(preset, true));
  });

  const handleChangeMasterPassword = async (currentPassword: string, nextPassword: string): Promise<void> => {
    await runAction('change_master_password', async () => {
      await changeSyncMasterPassword({
        currentMasterPassword: currentPassword,
        nextMasterPassword: nextPassword,
      });
      await alert(t('settings.sync.password_changed_success', { defaultValue: 'Master password updated successfully.' }), { variant: 'success' });
    });
  };

  const handleGenerateRecoveryKey = (): Promise<void> => runAction('generate_recovery_key', async () => {
    const key = await generateSyncRecoveryKey();
    setRecoveryKey(key);
  });

  const handleCopyRecoveryKey = async (): Promise<void> => {
    if (!recoveryKey) return;
    await navigator.clipboard.writeText(recoveryKey);
  };

  const handleExportRecoveryKey = (): Promise<void> => runAction('export_recovery_key', async () => {
    if (!recoveryKey) return;
    const outputPath = await saveDialog({
      defaultPath: 'sona-recovery-key.txt',
      filters: [{ name: 'Text file', extensions: ['txt'] }],
    });
    if (outputPath) {
      await writeFile(outputPath, new TextEncoder().encode(`${recoveryKey}\n`));
      await alert(t('settings.sync.recovery_key_exported', { defaultValue: 'Recovery key saved to {{path}}', path: outputPath }), { variant: 'success' });
    }
  });

  const handleDisconnect = async (): Promise<void> => {
    const approved = await confirm(
      t('settings.sync.disconnect_confirm_message', {
        defaultValue: 'Disconnect this device from the sync vault? Local data stays intact.',
      }),
      {
        title: t('settings.sync.disconnect_confirm_title', { defaultValue: 'Disconnect Sync Vault' }),
        confirmLabel: t('settings.sync.disconnect', { defaultValue: 'Disconnect' }),
        variant: 'error',
      },
    );
    if (!approved) return;
    await runAction('disconnect', async () => {
      setSnapshot(await disconnectSyncVault());
      setRecoveryKey(null);
    });
  };

  return (
    <SettingsTabContainer id="settings-sync-panel" ariaLabelledby="settings-tab-sync">
      <SettingsPageHeader
        icon={<Cloud width={28} height={28} />}
        title={t('settings.sync.title', { defaultValue: 'Cloud Sync' })}
        description={t('settings.sync.page_description', {
          defaultValue: 'End-to-end encrypted synchronization across devices with WebDAV.',
        })}
      />

      {/* Feature Switch Section */}
      <SettingsSection>
        <SettingsItem
          title={t('settings.sync.enable_cloud_sync', { defaultValue: 'Enable Cloud Sync' })}
          hint={t('settings.sync.enable_cloud_sync_hint', {
            defaultValue: 'Enable end-to-end encrypted WebDAV sync across devices and show status capsule in the header.',
          })}
        >
          <Switch
            id="enable-cloud-sync-switch"
            checked={enableCloudSync}
            onChange={(checked) => setConfig({ enableCloudSync: checked })}
          />
        </SettingsItem>
      </SettingsSection>

      {!enableCloudSync ? (
        <div className="sync-disabled-feature-card">
          <div className="sync-disabled-feature-icon">
            <CloudOff size={28} />
          </div>
          <div className="sync-disabled-feature-content">
            <strong>
              {t('settings.sync.disabled_feature_notice', {
                defaultValue: 'Cloud sync is currently turned off. Turn it on to configure WebDAV sync and seamlessly sync transcripts and settings across devices.',
              })}
            </strong>
          </div>
        </div>
      ) : !isStatusLoaded ? (
        <SettingsSection>
          <div className="sync-banner-row">
            <div className="sync-empty-state">{t('common.loading', { defaultValue: 'Loading...' })}</div>
          </div>
        </SettingsSection>
      ) : status.state === 'disabled' ? (
        <SyncSetupPanel
          busyAction={busyAction}
          onCreate={handleCreate}
          onJoin={handleJoin}
          onPreviewJoin={handlePreviewJoin}
          onTestProvider={handleTestProvider}
          onDiscoverVaults={discoverWebDavSyncVaults}
        />
      ) : (
        <SyncConnectedPanel
          busyAction={busyAction}
          recoveryKey={recoveryKey}
          status={status}
          onChangeMasterPassword={handleChangeMasterPassword}
          onChangePreset={handleChangePreset}
          onCopyRecoveryKey={handleCopyRecoveryKey}
          onDisconnect={handleDisconnect}
          onExportRecoveryKey={handleExportRecoveryKey}
          onGenerateRecoveryKey={handleGenerateRecoveryKey}
          onLock={handleLock}
          onRunNow={handleRunNow}
          onSetPaused={handleSetPaused}
          onUnlock={handleUnlock}
          onUnlockWithRecovery={handleUnlockWithRecovery}
          conflictCenterSlot={
            <SyncConflictCenter
              conflictCount={status.conflictCount}
              disabled={status.state === 'locked'}
            />
          }
        />
      )}
    </SettingsTabContainer>
  );
}

export default SettingsSyncTab;
