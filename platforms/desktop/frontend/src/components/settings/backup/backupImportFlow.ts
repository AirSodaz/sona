import type {
  BackupManifestV1,
  PreparedBackupImport,
} from '../../../types/backup';

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;

interface RunPreparedBackupImportFlowOptions {
  prepare: () => Promise<PreparedBackupImport | null>;
  confirm: (prepared: PreparedBackupImport) => Promise<boolean>;
  apply: (prepared: PreparedBackupImport) => Promise<void>;
  dispose: (prepared: PreparedBackupImport) => Promise<void>;
  alertSuccess: () => Promise<void>;
  onError: (error: unknown) => Promise<void>;
}

/**
 * Builds the destructive import summary copy shared by local and remote backup
 * restore flows.
 */
export function buildBackupImportDetails(
  t: TranslateFn,
  manifest: BackupManifestV1,
): string {
  return [
    t('settings.backup.import_details_scope', {
      defaultValue:
        'This archive will replace config, workspace, light history, automation state, and dashboard LLM usage on this device.',
    }),
    t('settings.backup.import_details_audio_warning', {
      defaultValue:
        'Light history backups do not include audio files, so restored items may reopen without playback.',
    }),
    '',
    t('settings.backup.summary_title', {
      defaultValue: 'Archive summary',
    }),
    t('settings.backup.summary_projects', {
      defaultValue: 'Projects: {{count}}',
      count: manifest.counts.tags,
    }),
    t('settings.backup.summary_history_items', {
      defaultValue: 'History items: {{count}}',
      count: manifest.counts.historyItems,
    }),
    t('settings.backup.summary_transcripts', {
      defaultValue: 'Transcript files: {{count}}',
      count: manifest.counts.transcriptFiles,
    }),
    t('settings.backup.summary_summaries', {
      defaultValue: 'Summary files: {{count}}',
      count: manifest.counts.summaryFiles,
    }),
    t('settings.backup.summary_automation_rules', {
      defaultValue: 'Automation rules: {{count}}',
      count: manifest.counts.automationRules,
    }),
    t('settings.backup.summary_automation_processed', {
      defaultValue: 'Processed automation files: {{count}}',
      count: manifest.counts.automationProcessedEntries,
    }),
    t('settings.backup.summary_analytics', {
      defaultValue: 'Analytics files: {{count}}',
      count: manifest.counts.analyticsFiles,
    }),
  ].join('\n');
}

/**
 * Runs the common prepared-backup import flow used by local archive import and
 * remote WebDAV restore without changing their surrounding UI copy.
 */
export async function runPreparedBackupImportFlow(
  options: RunPreparedBackupImportFlowOptions,
): Promise<void> {
  let prepared: PreparedBackupImport | null = null;

  try {
    prepared = await options.prepare();
    if (!prepared) {
      return;
    }

    const confirmed = await options.confirm(prepared);
    if (!confirmed) {
      await options.dispose(prepared);
      return;
    }

    await options.apply(prepared);
    prepared = null;
    await options.alertSuccess();
  } catch (error) {
    if (prepared) {
      await options.dispose(prepared).catch(() => undefined);
    }

    await options.onError(error);
  }
}
