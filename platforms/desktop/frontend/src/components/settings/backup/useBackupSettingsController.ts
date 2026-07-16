import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  applyImportBackup,
  backupService,
  disposePreparedImport,
} from '../../../services/backupService';
import { useBatchQueueStore } from '../../../stores/batchQueueStore';
import { useDialogStore } from '../../../stores/dialogStore';
import { useTranscriptRuntimeStore } from '../../../stores/transcriptRuntimeStore';
import type { PreparedBackupImport } from '../../../types/backup';
import { buildBackupImportDetails, runPreparedBackupImportFlow } from './backupImportFlow';

export type BackupBusyAction = 'export' | 'import';

export const preparedBackupImportActions = {
  apply: applyImportBackup,
  dispose: disposePreparedImport,
};

interface UseBackupSettingsControllerResult {
  backupBlockerHint: string;
  busyAction: BackupBusyAction | null;
  isBackupBlocked: boolean;
  handleExportBackup: () => Promise<void>;
  handleImportBackup: () => Promise<void>;
}

export function useBackupSettingsController(): UseBackupSettingsControllerResult {
  const { t } = useTranslation();
  const alert = useDialogStore((state) => state.alert);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const hasBlockingQueueItems = useBatchQueueStore((state) => state.queueItems.some(
    (item) => item.status === 'pending' || item.status === 'processing',
  ));
  const [busyAction, setBusyAction] = React.useState<BackupBusyAction | null>(null);
  const blocker = isRecording ? 'recording' : hasBlockingQueueItems ? 'batch' : null;
  const backupBlockerHint = blocker === 'recording'
    ? t('settings.backup.blocked_recording', { defaultValue: 'Stop Live Record before exporting or importing backups.' })
    : blocker === 'batch'
      ? t('settings.backup.blocked_batch', { defaultValue: 'Wait for Batch Import to finish or clear pending items before exporting or importing backups.' })
      : t('settings.backup.advanced_hint', { defaultValue: 'Export or replace local data using a complete recovery archive.' });

  const runAction = React.useCallback(async (
    action: BackupBusyAction,
    task: () => Promise<void>,
  ) => {
    setBusyAction(action);
    try {
      await task();
    } finally {
      setBusyAction(null);
    }
  }, []);

  const reportError = React.useCallback((code: string, messageKey: string, cause: unknown) => showError({
    code,
    messageKey,
    cause,
    titleKey: 'settings.backup.error_title',
  }), [showError]);

  const confirmImport = React.useCallback(async (prepared: PreparedBackupImport) => confirm(
    t('settings.backup.import_confirm_message', { defaultValue: 'Import this backup and replace the current local data?' }),
    {
      title: t('settings.backup.import_confirm_title', { defaultValue: 'Replace current data' }),
      details: buildBackupImportDetails(t, prepared.manifest),
      confirmLabel: t('settings.backup.import_button', { defaultValue: 'Import Backup' }),
      cancelLabel: t('common.cancel'),
    },
  ), [confirm, t]);

  const handleExportBackup = React.useCallback(async () => runAction('export', async () => {
    try {
      const result = await backupService.exportBackup();
      if (result) {
        await alert(t('settings.backup.export_success', { defaultValue: 'Backup archive created successfully.' }), {
          variant: 'success',
          details: result.archivePath,
        });
      }
    } catch (error) {
      await reportError('backup.export_failed', 'errors.backup.export_failed', error);
    }
  }), [alert, reportError, runAction, t]);

  const handleImportBackup = React.useCallback(async () => runAction('import', async () => {
    await runPreparedBackupImportFlow({
      prepare: () => backupService.prepareImportBackup(),
      confirm: confirmImport,
      ...preparedBackupImportActions,
      alertSuccess: () => alert(t('settings.backup.import_success', { defaultValue: 'Backup archive imported successfully.' }), { variant: 'success' }),
      onError: (error) => reportError('backup.import_failed', 'errors.backup.import_failed', error),
    });
  }), [alert, confirmImport, reportError, runAction, t]);

  return {
    backupBlockerHint,
    busyAction,
    isBackupBlocked: blocker !== null,
    handleExportBackup,
    handleImportBackup,
  };
}
