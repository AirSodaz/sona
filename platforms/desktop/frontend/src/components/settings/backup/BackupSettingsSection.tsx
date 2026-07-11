import React from 'react';
import { Database } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { SettingsSection } from '../SettingsLayout';
import { BackupArchiveActions } from './BackupArchiveActions';
import { useBackupSettingsController } from './useBackupSettingsController';
import { WebDavBackupPanel } from './WebDavBackupPanel';

interface BackupSettingsSectionProps {
  isVisible?: boolean;
  isPrewarming?: boolean;
}

/**
 * Stable facade for the backup settings area. Internal state and WebDAV UI live
 * in focused backup submodules so this entrypoint stays easy to scan.
 */
export function BackupSettingsSection({
  isVisible = true,
  isPrewarming = false,
}: BackupSettingsSectionProps): React.JSX.Element {
  const { t } = useTranslation();
  const controller = useBackupSettingsController({
    isVisible,
    isPrewarming,
  });

  return (
    <SettingsSection
      title={t('settings.backup.title', { defaultValue: 'Backup & Restore' })}
      description={controller.backupBlockerHint}
      icon={<Database size={20} />}
    >
      <BackupArchiveActions
        busyAction={controller.busyAction}
        isBackupBlocked={controller.isBackupBlocked}
        onExport={controller.handleExportBackup}
        onImport={controller.handleImportBackup}
      />

      <WebDavBackupPanel
        busyAction={controller.busyAction}
        hasLoadedRemoteBackups={controller.hasLoadedRemoteBackups}
        isOpen={controller.isWebDavOpen}
        remoteBackups={controller.remoteBackups}
        webDavConfig={controller.webDavConfig}
        webDavConfigActionDisabled={controller.webDavConfigActionDisabled}
        webDavConfigError={controller.webDavConfigError}
        webDavConfigReady={controller.webDavConfigReady}
        webDavTransferDisabled={controller.webDavTransferDisabled}
        onConfigChange={controller.persistWebDavConfig}
        onRefreshRemoteBackups={controller.handleRefreshRemoteBackups}
        onRestoreRemoteBackup={controller.handleRestoreRemoteBackup}
        onTestConnection={controller.handleTestWebDavConnection}
        onToggle={controller.handleToggleWebDav}
        onUploadBackup={controller.handleUploadWebDavBackup}
      />
    </SettingsSection>
  );
}
