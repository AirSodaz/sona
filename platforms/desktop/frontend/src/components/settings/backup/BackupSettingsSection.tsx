import React from 'react';
import { Database, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  changeSyncMasterPassword,
  changeSyncPreset,
  createSyncVault,
  disconnectSyncVault,
  generateSyncRecoveryKey,
  joinSyncVault,
  lockSyncVault,
  previewSyncJoin,
  runSyncNow,
  setSyncPaused,
  testWebDavSyncProvider,
  unlockSyncVault,
  unlockSyncVaultWithRecovery,
} from '../../../services/tauri/sync';
import { syncRuntimeService } from '../../../services/syncRuntimeService';
import { saveDialog } from '../../../services/tauri/platform/dialog';
import { writeFile } from '../../../services/tauri/platform/fs';
import { useDialogStore } from '../../../stores/dialogStore';
import { useSyncStatusStore } from '../../../stores/syncStatusStore';
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
} from '../../../types/sync';
import { SettingsAccordion, SettingsSection } from '../SettingsLayout';
import { SyncConflictCenter } from '../sync/SyncConflictCenter';
import { SyncConnectedPanel } from '../sync/SyncConnectedPanel';
import { isPresetShrink } from '../sync/syncPreset';
import { SyncSetupPanel } from '../sync/SyncSetupPanel';
import { LegacyRemoteBackupPanel } from '../sync/LegacyRemoteBackupPanel';
import { BackupArchiveActions } from './BackupArchiveActions';
import { useBackupSettingsController } from './useBackupSettingsController';
import '../sync/SyncSettings.css';

interface BackupSettingsSectionProps {
  isVisible?: boolean;
  isPrewarming?: boolean;
}

export function BackupSettingsSection({
  isVisible = true,
  isPrewarming = false,
}: BackupSettingsSectionProps): React.JSX.Element {
  const { t } = useTranslation();
  const alert = useDialogStore((state) => state.alert);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const status = useSyncStatusStore((state) => state.snapshot);
  const isStatusLoaded = useSyncStatusStore((state) => state.isLoaded);
  const setStatus = useSyncStatusStore((state) => state.setSnapshot);
  const setLastRunResult = useSyncStatusStore((state) => state.setLastRunResult);
  const [busyAction, setBusyAction] = React.useState<string | null>(null);
  const [recoveryKey, setRecoveryKey] = React.useState<string | null>(null);
  const localBackup = useBackupSettingsController();

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
      // runReturningAction already reports the structured error.
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
      setStatus(result.status);
      setRecoveryKey(result.recoveryKey);
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
    setStatus(await unlockSyncVault(request));
    syncRuntimeService.requestSync(0);
  });

  const handleUnlockWithRecovery = (request: SyncUnlockRecoveryRequest): Promise<void> => runAction('unlock', async () => {
    setStatus(await unlockSyncVaultWithRecovery(request));
    syncRuntimeService.requestSync(0);
  });

  const handleRunNow = (): Promise<void> => runAction('run', async () => {
    setLastRunResult(await runSyncNow());
    await syncRuntimeService.refreshStatus();
  });

  const handleSetPaused = (paused: boolean): Promise<void> => runAction(paused ? 'pause' : 'resume', async () => {
    setStatus(await setSyncPaused(paused));
    if (!paused) {
      syncRuntimeService.requestSync(0);
    }
  });

  const handleLock = (): Promise<void> => runAction('lock', async () => {
    setStatus(await lockSyncVault());
  });

  const handleDisconnect = async (): Promise<void> => {
    const accepted = await confirm(
      t('settings.sync.disconnect_confirm', {
        defaultValue: 'Disconnect this device from the sync vault?',
      }),
      {
        title: t('settings.sync.disconnect_title', { defaultValue: 'Disconnect this device' }),
        confirmLabel: t('settings.sync.disconnect_action', { defaultValue: 'Disconnect' }),
        cancelLabel: t('common.cancel'),
      },
    );
    if (!accepted) {
      return;
    }
    await runAction('disconnect', async () => {
      setStatus(await disconnectSyncVault());
      setRecoveryKey(null);
    });
  };

  const handleChangePreset = async (preset: SyncPresetV1): Promise<void> => {
    const current = status.preset;
    let confirmShrink = false;
    if (current && isPresetShrink(current, preset)) {
      confirmShrink = await confirm(
        t('settings.sync.shrink_confirm', {
          defaultValue: 'Shrinking the preset publishes tombstones for data that leaves sync. Continue?',
        }),
        {
          title: t('settings.sync.shrink_title', { defaultValue: 'Shrink sync preset' }),
          confirmLabel: t('common.apply', { defaultValue: 'Apply' }),
          cancelLabel: t('common.cancel'),
        },
      );
      if (!confirmShrink) {
        return;
      }
    }
    await runAction('change_preset', async () => {
      setStatus(await changeSyncPreset(preset, confirmShrink));
      syncRuntimeService.requestSync(0);
    });
  };

  const handleChangeMasterPassword = (
    currentMasterPassword: string,
    nextMasterPassword: string,
  ): Promise<void> => runAction('change_password', async () => {
    await changeSyncMasterPassword({ currentMasterPassword, nextMasterPassword });
    await alert(t('settings.sync.password_changed', { defaultValue: 'Master password changed.' }), { variant: 'success' });
  });

  const handleGenerateRecoveryKey = (): Promise<void> => runAction('generate_recovery_key', async () => {
    setRecoveryKey(await generateSyncRecoveryKey());
  });

  const handleCopyRecoveryKey = async (): Promise<void> => {
    if (recoveryKey && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(recoveryKey);
    }
  };

  const handleExportRecoveryKey = (): Promise<void> => runAction('export_recovery_key', async () => {
    if (!recoveryKey) {
      return;
    }
    const outputPath = await saveDialog({
      defaultPath: `sona-recovery-${status.vaultId ?? 'vault'}.txt`,
      filters: [{ name: 'Text', extensions: ['txt'] }],
    });
    if (!outputPath) {
      return;
    }
    await writeFile(outputPath, new TextEncoder().encode(`${recoveryKey}\n`));
  });

  const sectionDescription = status.state === 'disabled'
    ? t('settings.sync.description_disabled', { defaultValue: 'Encrypted multi-device sync is off for this data directory.' })
    : t('settings.sync.description_enabled', { defaultValue: 'End-to-end encrypted incremental sync. Audio and device-local settings stay local.' });

  return (
    <SettingsSection
      title={t('settings.sync.title', { defaultValue: 'Sync & Recovery' })}
      description={sectionDescription}
      icon={<ShieldCheck size={20} />}
    >
      {!isStatusLoaded ? (
        <div className="sync-empty-state">{t('common.loading', { defaultValue: 'Loading...' })}</div>
      ) : status.state === 'disabled' ? (
        <SyncSetupPanel
          busyAction={busyAction}
          onCreate={handleCreate}
          onJoin={handleJoin}
          onPreviewJoin={handlePreviewJoin}
          onTestProvider={handleTestProvider}
        />
      ) : (
        <>
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
          />
          <SyncConflictCenter
            conflictCount={status.conflictCount}
            disabled={status.state === 'locked'}
          />
        </>
      )}

      <SettingsAccordion
        title={(
          <div className="settings-accordion-copy">
            <div className="settings-accordion-copy-title sync-advanced-title"><Database size={16} />{t('settings.backup.advanced_title', { defaultValue: 'Advanced recovery' })}</div>
            <div className="settings-accordion-copy-hint">{localBackup.backupBlockerHint}</div>
          </div>
        )}
      >
        <BackupArchiveActions
          busyAction={localBackup.busyAction}
          isBackupBlocked={localBackup.isBackupBlocked}
          onExport={localBackup.handleExportBackup}
          onImport={localBackup.handleImportBackup}
        />
        <LegacyRemoteBackupPanel disabled={localBackup.isBackupBlocked} />
      </SettingsAccordion>
    </SettingsSection>
  );
}
