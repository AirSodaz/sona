import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  applyImportBackup,
  backupService,
  disposePreparedImport,
} from '../../../services/backupService';
import { backupWebDavService } from '../../../services/backupWebDavService';
import { useBatchQueueStore } from '../../../stores/batchQueueStore';
import { useDialogStore } from '../../../stores/dialogStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import type {
  BackupWebDavConfig,
  PreparedBackupImport,
  RemoteBackupEntry,
} from '../../../types/backup';
import { extractErrorMessage } from '../../../utils/errorUtils';
import {
  getSettingsPerfErrorDetail,
  markSettingsPerf,
} from '../../../utils/settingsPerf';
import {
  buildBackupImportDetails,
  runPreparedBackupImportFlow,
} from './backupImportFlow';

export type BackupBusyAction =
  | 'export'
  | 'import'
  | 'webdav_test'
  | 'webdav_upload'
  | 'webdav_refresh'
  | 'webdav_restore';

const EMPTY_WEBDAV_CONFIG: BackupWebDavConfig = {
  serverUrl: '',
  remoteDir: '',
  username: '',
  password: '',
};

export const preparedBackupImportActions = {
  apply: applyImportBackup,
  dispose: disposePreparedImport,
};

interface UseBackupSettingsControllerArgs {
  isPrewarming: boolean;
  isVisible: boolean;
}

interface UseBackupSettingsControllerResult {
  backupBlockerHint: string;
  busyAction: BackupBusyAction | null;
  hasLoadedRemoteBackups: boolean;
  isBackupBlocked: boolean;
  isWebDavOpen: boolean;
  remoteBackups: RemoteBackupEntry[];
  webDavConfig: BackupWebDavConfig;
  webDavConfigActionDisabled: boolean;
  webDavConfigError: string | null;
  webDavConfigReady: boolean;
  webDavTransferDisabled: boolean;
  handleExportBackup: () => Promise<void>;
  handleImportBackup: () => Promise<void>;
  handleRefreshRemoteBackups: () => Promise<void>;
  handleRestoreRemoteBackup: (entry: RemoteBackupEntry) => Promise<void>;
  handleTestWebDavConnection: () => Promise<void>;
  handleToggleWebDav: () => void;
  handleUploadWebDavBackup: () => Promise<void>;
  persistWebDavConfig: (nextConfig: BackupWebDavConfig) => void;
}

function getBackupBlockerHint(
  blocker: 'recording' | 'batch_queue' | null,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  if (blocker === 'recording') {
    return t('settings.backup.blocked_recording', {
      defaultValue: 'Stop Live Record before exporting or importing backups.',
    });
  }

  if (blocker === 'batch_queue') {
    return t('settings.backup.blocked_batch', {
      defaultValue:
        'Wait for Batch Import to finish or clear pending items before exporting or importing backups.',
    });
  }

  return t('settings.backup.description', {
    defaultValue:
      'Create or restore a single archive containing config, workspace, light history, automation state, and dashboard LLM usage.',
  });
}

/**
 * Owns backup settings state and action handlers while keeping the section
 * component itself focused on composition.
 */
export function useBackupSettingsController({
  isPrewarming,
  isVisible,
}: UseBackupSettingsControllerArgs): UseBackupSettingsControllerResult {
  const { t } = useTranslation();
  const alert = useDialogStore((state) => state.alert);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const hasBlockingQueueItems = useBatchQueueStore((state) =>
    state.queueItems.some(
      (item) => item.status === 'pending' || item.status === 'processing',
    ),
  );
  const [busyAction, setBusyAction] = React.useState<BackupBusyAction | null>(
    null,
  );
  const [webDavConfig, setWebDavConfig] =
    React.useState<BackupWebDavConfig>(EMPTY_WEBDAV_CONFIG);
  const [webDavConfigReady, setWebDavConfigReady] = React.useState(false);
  const [webDavConfigError, setWebDavConfigError] = React.useState<string | null>(
    null,
  );
  const [isWebDavOpen, setIsWebDavOpen] = React.useState(false);
  const [hasRequestedWebDavConfig, setHasRequestedWebDavConfig] =
    React.useState(false);
  const [remoteBackups, setRemoteBackups] = React.useState<RemoteBackupEntry[]>(
    [],
  );
  const [hasLoadedRemoteBackups, setHasLoadedRemoteBackups] =
    React.useState(false);

  const backupBlocker: 'recording' | 'batch_queue' | null = isRecording
    ? 'recording'
    : hasBlockingQueueItems
      ? 'batch_queue'
      : null;
  const isBackupBlocked = backupBlocker !== null;
  const backupBlockerHint = getBackupBlockerHint(backupBlocker, t);
  const webDavTransferDisabled =
    busyAction !== null || isBackupBlocked || !webDavConfigReady;
  const webDavConfigActionDisabled = busyAction !== null || !webDavConfigReady;

  const showBackupError = React.useCallback(
    (code: string, messageKey: string, cause: unknown) =>
      showError({
        code,
        messageKey,
        cause,
        titleKey: 'settings.backup.error_title',
      }),
    [showError],
  );

  React.useEffect(() => {
    if (!isVisible && !isPrewarming) {
      return;
    }

    const markerPrefix = isPrewarming
      ? 'settings.prewarm.backup'
      : 'settings.backup';
    markSettingsPerf(`${markerPrefix}.commit`);
  }, [isPrewarming, isVisible]);

  React.useEffect(() => {
    if (!hasRequestedWebDavConfig || webDavConfigReady) {
      return;
    }

    let cancelled = false;
    markSettingsPerf('settings.webdav.loadConfig.start');

    void backupWebDavService
      .loadConfig()
      .then((loadedConfig) => {
        if (cancelled) {
          return;
        }

        setWebDavConfig(loadedConfig);
        setWebDavConfigReady(true);
        markSettingsPerf('settings.webdav.loadConfig.end');
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }

        setWebDavConfigReady(true);
        setWebDavConfigError(extractErrorMessage(error));
        markSettingsPerf(
          'settings.webdav.loadConfig.fail',
          getSettingsPerfErrorDetail(error),
        );
      });

    return () => {
      cancelled = true;
    };
  }, [hasRequestedWebDavConfig, webDavConfigReady]);

  const runBusyAction = React.useCallback(
    async (action: BackupBusyAction, task: () => Promise<void>) => {
      setBusyAction(action);
      try {
        await task();
      } finally {
        setBusyAction(null);
      }
    },
    [],
  );

  const showImportSuccessAlert = React.useCallback(
    () =>
      alert(
        t('settings.backup.import_success', {
          defaultValue: 'Backup archive imported successfully.',
        }),
        {
          variant: 'success',
        },
      ),
    [alert, t],
  );

  const confirmPreparedImport = React.useCallback(
    (confirmLabel: string) => async (prepared: PreparedBackupImport) =>
      confirm(
        t('settings.backup.import_confirm_message', {
          defaultValue: 'Import this backup and replace the current local data?',
        }),
        {
          title: t('settings.backup.import_confirm_title', {
            defaultValue: 'Replace current data',
          }),
          details: buildBackupImportDetails(t, prepared.manifest),
          confirmLabel,
          cancelLabel: t('common.cancel'),
        },
      ),
    [confirm, t],
  );

  const refreshRemoteBackups = React.useCallback(async () => {
    const backups = await backupWebDavService.listBackups(webDavConfig);
    setRemoteBackups(backups);
    setHasLoadedRemoteBackups(true);
  }, [webDavConfig]);

  const handleToggleWebDav = React.useCallback(() => {
    const willOpen = !isWebDavOpen;
    if (willOpen && !hasRequestedWebDavConfig) {
      markSettingsPerf('settings.webdav.open');
      setHasRequestedWebDavConfig(true);
    }

    setIsWebDavOpen(willOpen);
  }, [hasRequestedWebDavConfig, isWebDavOpen]);

  const persistWebDavConfig = React.useCallback((nextConfig: BackupWebDavConfig) => {
    setWebDavConfig(nextConfig);
    setWebDavConfigError(null);
    setRemoteBackups([]);
    setHasLoadedRemoteBackups(false);

    void backupWebDavService.saveConfig(nextConfig).catch((error) => {
      setWebDavConfigError(extractErrorMessage(error));
    });
  }, []);

  const handleExportBackup = React.useCallback(
    async () =>
      runBusyAction('export', async () => {
        try {
          const result = await backupService.exportBackup();
          if (!result) {
            return;
          }

          await alert(
            t('settings.backup.export_success', {
              defaultValue: 'Backup archive created successfully.',
            }),
            {
              variant: 'success',
              details: result.archivePath,
            },
          );
        } catch (error) {
          await showBackupError(
            'backup.export_failed',
            'errors.backup.export_failed',
            error,
          );
        }
      }),
    [alert, runBusyAction, showBackupError, t],
  );

  const handleImportBackup = React.useCallback(
    async () =>
      runBusyAction('import', async () => {
        await runPreparedBackupImportFlow({
          prepare: () => backupService.prepareImportBackup(),
          confirm: confirmPreparedImport(
            t('settings.backup.import_button', {
              defaultValue: 'Import Backup',
            }),
          ),
          ...preparedBackupImportActions,
          alertSuccess: showImportSuccessAlert,
          onError: (error) =>
            showBackupError(
              'backup.import_failed',
              'errors.backup.import_failed',
              error,
            ),
        });
      }),
    [
      confirmPreparedImport,
      runBusyAction,
      showBackupError,
      showImportSuccessAlert,
      t,
    ],
  );

  const handleTestWebDavConnection = React.useCallback(
    async () =>
      runBusyAction('webdav_test', async () => {
        try {
          const result = await backupWebDavService.testConnection(webDavConfig);
          await alert(result.message, {
            title: t('settings.backup.cloud_test_title', {
              defaultValue: 'WebDAV connection',
            }),
            variant: result.status === 'warning' ? 'warning' : 'success',
          });
        } catch (error) {
          await showBackupError(
            'backup.webdav_test_failed',
            'errors.backup.webdav_test_failed',
            error,
          );
        }
      }),
    [alert, runBusyAction, showBackupError, t, webDavConfig],
  );

  const handleRefreshRemoteBackups = React.useCallback(
    async () =>
      runBusyAction('webdav_refresh', async () => {
        try {
          await refreshRemoteBackups();
        } catch (error) {
          await showBackupError(
            'backup.webdav_refresh_failed',
            'errors.backup.webdav_refresh_failed',
            error,
          );
        }
      }),
    [refreshRemoteBackups, runBusyAction, showBackupError],
  );

  const handleUploadWebDavBackup = React.useCallback(
    async () =>
      runBusyAction('webdav_upload', async () => {
        try {
          const result = await backupWebDavService.uploadBackup(webDavConfig);
          await refreshRemoteBackups();
          await alert(
            t('settings.backup.cloud_upload_success', {
              defaultValue: 'Backup archive uploaded to WebDAV successfully.',
            }),
            {
              variant: 'success',
              details: result.fileName,
            },
          );
        } catch (error) {
          await showBackupError(
            'backup.webdav_upload_failed',
            'errors.backup.webdav_upload_failed',
            error,
          );
        }
      }),
    [
      alert,
      refreshRemoteBackups,
      runBusyAction,
      showBackupError,
      t,
      webDavConfig,
    ],
  );

  const handleRestoreRemoteBackup = React.useCallback(
    async (entry: RemoteBackupEntry) =>
      runBusyAction('webdav_restore', async () => {
        await runPreparedBackupImportFlow({
          prepare: () =>
            backupWebDavService.prepareImportFromRemote(entry, webDavConfig),
          confirm: confirmPreparedImport(
            t('settings.backup.cloud_restore_button', {
              defaultValue: 'Restore',
            }),
          ),
          ...preparedBackupImportActions,
          alertSuccess: showImportSuccessAlert,
          onError: (error) =>
            showBackupError(
              'backup.webdav_restore_failed',
              'errors.backup.webdav_restore_failed',
              error,
            ),
        });
      }),
    [
      confirmPreparedImport,
      runBusyAction,
      showBackupError,
      showImportSuccessAlert,
      t,
      webDavConfig,
    ],
  );

  return {
    backupBlockerHint,
    busyAction,
    hasLoadedRemoteBackups,
    isBackupBlocked,
    isWebDavOpen,
    remoteBackups,
    webDavConfig,
    webDavConfigActionDisabled,
    webDavConfigError,
    webDavConfigReady,
    webDavTransferDisabled,
    handleExportBackup,
    handleImportBackup,
    handleRefreshRemoteBackups,
    handleRestoreRemoteBackup,
    handleTestWebDavConnection,
    handleToggleWebDav,
    handleUploadWebDavBackup,
    persistWebDavConfig,
  };
}
