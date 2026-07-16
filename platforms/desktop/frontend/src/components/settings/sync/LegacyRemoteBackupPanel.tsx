import React from 'react';
import { CloudDownload, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  listLegacyRemoteBackups,
  prepareLegacyRemoteBackupImport,
} from '../../../services/tauri/sync';
import {
  settingsStore,
  STORE_KEY_BACKUP_WEBDAV,
} from '../../../services/storageService';
import { useDialogStore } from '../../../stores/dialogStore';
import type { BackupWebDavConfig, PreparedBackupImport } from '../../../types/backup';
import type {
  LegacyRemoteBackupEntry,
  WebDavObjectStoreConfig,
} from '../../../types/sync';
import { buildBackupImportDetails, runPreparedBackupImportFlow } from '../backup/backupImportFlow';
import { preparedBackupImportActions } from '../backup/useBackupSettingsController';

interface LegacyRemoteBackupPanelProps {
  disabled: boolean;
}

const EMPTY_CONFIG: WebDavObjectStoreConfig = {
  serverUrl: '',
  remoteRoot: '',
  username: '',
  password: '',
};

function formatSize(size: number): string {
  if (size >= 1024 * 1024) {
    return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (size >= 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }
  return `${size} B`;
}

function isReady(config: WebDavObjectStoreConfig): boolean {
  try {
    return new URL(config.serverUrl).protocol === 'https:'
      && Boolean(config.username.trim());
  } catch {
    return false;
  }
}

export function LegacyRemoteBackupPanel({
  disabled,
}: LegacyRemoteBackupPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const alert = useDialogStore((state) => state.alert);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const [config, setConfig] = React.useState(EMPTY_CONFIG);
  const [entries, setEntries] = React.useState<LegacyRemoteBackupEntry[]>([]);
  const [loaded, setLoaded] = React.useState(false);
  const [busy, setBusy] = React.useState<'list' | 'import' | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    void settingsStore.get<BackupWebDavConfig | null>(STORE_KEY_BACKUP_WEBDAV)
      .then((legacy) => {
        if (!cancelled && legacy) {
          setConfig({
            serverUrl: legacy.serverUrl,
            remoteRoot: legacy.remoteDir,
            username: legacy.username,
            password: legacy.password,
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const reportError = React.useCallback((cause: unknown) => showError({
    code: 'sync.legacy_import_failed',
    messageKey: 'errors.sync.legacy_import_failed',
    cause,
    titleKey: 'settings.sync.error_title',
  }), [showError]);

  const clearMigratedPlaintext = async () => {
    const legacy = await settingsStore.get<BackupWebDavConfig | null>(STORE_KEY_BACKUP_WEBDAV);
    if (
      legacy?.password === config.password
      && legacy.serverUrl.trim() === config.serverUrl.trim()
      && legacy.remoteDir.trim() === config.remoteRoot.trim()
      && legacy.username.trim() === config.username.trim()
    ) {
      await settingsStore.set(STORE_KEY_BACKUP_WEBDAV, { ...legacy, password: '' });
      await settingsStore.save();
    }
  };

  const refresh = async () => {
    setBusy('list');
    try {
      const result = await listLegacyRemoteBackups(config);
      setEntries(result.entries);
      setLoaded(true);
      if (result.credentialsMigrated) {
        await clearMigratedPlaintext();
      }
    } catch (error) {
      await reportError(error);
    } finally {
      setBusy(null);
    }
  };

  const restore = async (entry: LegacyRemoteBackupEntry) => {
    setBusy('import');
    try {
      await runPreparedBackupImportFlow({
        prepare: () => prepareLegacyRemoteBackupImport(config, entry.key),
        confirm: (prepared: PreparedBackupImport) => confirm(
          t('settings.backup.import_confirm_message', { defaultValue: 'Import this backup and replace the current local data?' }),
          {
            title: t('settings.backup.import_confirm_title', { defaultValue: 'Replace current data' }),
            details: buildBackupImportDetails(t, prepared.manifest),
            confirmLabel: t('settings.backup.import_button', { defaultValue: 'Import Backup' }),
            cancelLabel: t('common.cancel'),
          },
        ),
        ...preparedBackupImportActions,
        alertSuccess: () => alert(t('settings.backup.import_success', { defaultValue: 'Backup archive imported successfully.' }), { variant: 'success' }),
        onError: reportError,
      });
    } finally {
      setBusy(null);
    }
  };

  const update = (patch: Partial<WebDavObjectStoreConfig>) => {
    setConfig((current) => ({ ...current, ...patch }));
    setEntries([]);
    setLoaded(false);
  };

  return (
    <div className="sync-legacy-panel">
      <div className="sync-form-heading"><CloudDownload size={17} /><span>{t('settings.sync.legacy_title', { defaultValue: 'Legacy WebDAV archives' })}</span></div>
      <div className="settings-accordion-copy-hint">{t('settings.sync.legacy_hint', { defaultValue: 'Import an existing remote archive once. New full-archive uploads are no longer created.' })}</div>
      <div className="sync-form-grid">
        <label className="sync-field sync-field-wide"><span>{t('settings.sync.server_url', { defaultValue: 'Server URL' })}</span><input className="settings-input" type="url" value={config.serverUrl} onChange={(event) => update({ serverUrl: event.target.value })} disabled={disabled || busy !== null} /></label>
        <label className="sync-field sync-field-wide"><span>{t('settings.sync.remote_root', { defaultValue: 'Remote root' })}</span><input className="settings-input" type="text" value={config.remoteRoot} onChange={(event) => update({ remoteRoot: event.target.value })} disabled={disabled || busy !== null} /></label>
        <label className="sync-field"><span>{t('settings.sync.username', { defaultValue: 'Username' })}</span><input className="settings-input" type="text" autoComplete="username" value={config.username} onChange={(event) => update({ username: event.target.value })} disabled={disabled || busy !== null} /></label>
        <label className="sync-field"><span>{t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}</span><input className="settings-input" type="password" autoComplete="current-password" value={config.password} onChange={(event) => update({ password: event.target.value })} disabled={disabled || busy !== null} /></label>
      </div>
      <button type="button" className="btn btn-secondary" onClick={() => void refresh()} disabled={disabled || busy !== null || !isReady(config)}>
        <RefreshCw size={16} className={busy === 'list' ? 'queue-icon-spin' : undefined} />
        {busy === 'list'
          ? t('settings.sync.legacy_loading', { defaultValue: 'Loading archives...' })
          : t('settings.sync.legacy_list', { defaultValue: 'List archives' })}
      </button>
      {loaded ? (
        entries.length === 0 ? (
          <div className="sync-empty-state">{t('settings.sync.legacy_empty', { defaultValue: 'No legacy backup archives found' })}</div>
        ) : (
          <div className="sync-legacy-list">
            {entries.map((entry) => (
              <div key={entry.key}>
                <span><strong>{entry.fileName}</strong><small>{entry.modifiedAt ? new Date(entry.modifiedAt).toLocaleString() : t('settings.sync.unknown_time', { defaultValue: 'Unknown time' })} · {formatSize(entry.size)}</small></span>
                <button type="button" className="btn btn-secondary" onClick={() => void restore(entry)} disabled={disabled || busy !== null}>
                  {busy === 'import'
                    ? t('settings.backup.cloud_restore_busy', { defaultValue: 'Preparing...' })
                    : t('settings.backup.cloud_restore_button', { defaultValue: 'Restore' })}
                </button>
              </div>
            ))}
          </div>
        )
      ) : null}
    </div>
  );
}
