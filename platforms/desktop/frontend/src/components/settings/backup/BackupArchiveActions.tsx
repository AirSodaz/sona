import React from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsItem } from '../SettingsLayout';
import type { BackupBusyAction } from './useBackupSettingsController';

interface BackupArchiveActionsProps {
  busyAction: BackupBusyAction | null;
  isBackupBlocked: boolean;
  onExport: () => Promise<void>;
  onImport: () => Promise<void>;
}

/**
 * Renders the local export/import backup actions without owning any state.
 */
export function BackupArchiveActions({
  busyAction,
  isBackupBlocked,
  onExport,
  onImport,
}: BackupArchiveActionsProps): React.JSX.Element {
  const { t } = useTranslation();
  const isActionDisabled = busyAction !== null || isBackupBlocked;

  return (
    <>
      <SettingsItem
        title={t('settings.backup.export_title', { defaultValue: 'Export Backup' })}
        hint={t('settings.backup.export_hint', {
          defaultValue:
            'Create one .tar.bz2 archive containing config, workspace, light history transcripts, summaries, automation state, and dashboard LLM usage. Audio files, onboarding, current project, and recovery state are excluded.',
        })}
      >
        <button
          type="button"
          className="btn btn-secondary"
          onClick={onExport}
          disabled={isActionDisabled}
        >
          {busyAction === 'export'
            ? t('settings.backup.export_busy', { defaultValue: 'Exporting...' })
            : t('settings.backup.export_button', { defaultValue: 'Export Backup' })}
        </button>
      </SettingsItem>

      <SettingsItem
        title={t('settings.backup.import_title', { defaultValue: 'Import Backup' })}
        hint={t('settings.backup.import_hint', {
          defaultValue:
            'Replace the current config, workspace, light history, automation state, and dashboard LLM usage from a backup archive. Imported light history does not restore audio playback files.',
        })}
      >
        <button
          type="button"
          className="btn btn-secondary"
          onClick={onImport}
          disabled={isActionDisabled}
        >
          {busyAction === 'import'
            ? t('settings.backup.import_busy', { defaultValue: 'Importing...' })
            : t('settings.backup.import_button', { defaultValue: 'Import Backup' })}
        </button>
      </SettingsItem>
    </>
  );
}
