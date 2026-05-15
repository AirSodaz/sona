import React from 'react';
import { useTranslation } from 'react-i18next';
import type {
  BackupWebDavConfig,
  RemoteBackupEntry,
} from '../../../types/backup';
import { SettingsAccordion } from '../SettingsLayout';
import type { BackupBusyAction } from './useBackupSettingsController';

interface WebDavBackupPanelProps {
  busyAction: BackupBusyAction | null;
  hasLoadedRemoteBackups: boolean;
  isOpen: boolean;
  remoteBackups: RemoteBackupEntry[];
  webDavConfig: BackupWebDavConfig;
  webDavConfigActionDisabled: boolean;
  webDavConfigError: string | null;
  webDavConfigReady: boolean;
  webDavTransferDisabled: boolean;
  onConfigChange: (nextConfig: BackupWebDavConfig) => void;
  onRefreshRemoteBackups: () => Promise<void>;
  onRestoreRemoteBackup: (entry: RemoteBackupEntry) => Promise<void>;
  onTestConnection: () => Promise<void>;
  onToggle: () => void;
  onUploadBackup: () => Promise<void>;
}

function isNotHttpsUrl(value: string): boolean {
  const trimmed = value.trim();
  return trimmed.length > 0 && !trimmed.toLowerCase().startsWith('https://');
}

function formatRemoteBackupSize(size: number): string {
  if (size >= 1024 * 1024) {
    return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (size >= 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }

  return `${size} B`;
}

function formatRemoteBackupModifiedAt(
  modifiedAt: string | null,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  if (!modifiedAt) {
    return t('settings.backup.cloud_unknown_time', {
      defaultValue: 'Unknown time',
    });
  }

  const date = new Date(modifiedAt);
  if (Number.isNaN(date.getTime())) {
    return modifiedAt;
  }

  return date.toLocaleString();
}

/**
 * Renders the WebDAV accordion, form fields, and remote backup list.
 */
export function WebDavBackupPanel({
  busyAction,
  hasLoadedRemoteBackups,
  isOpen,
  remoteBackups,
  webDavConfig,
  webDavConfigActionDisabled,
  webDavConfigError,
  webDavConfigReady,
  webDavTransferDisabled,
  onConfigChange,
  onRefreshRemoteBackups,
  onRestoreRemoteBackup,
  onTestConnection,
  onToggle,
  onUploadBackup,
}: WebDavBackupPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const title = (
    <div className="settings-accordion-copy">
      <div className="settings-accordion-copy-title">
        {t('settings.backup.cloud_title', { defaultValue: 'WebDAV Cloud Sync' })}
      </div>
      <div className="settings-accordion-copy-hint">
        {t('settings.backup.cloud_hint', {
          defaultValue:
            'Save WebDAV credentials locally on this device, upload backup archives manually, and restore from any remote snapshot without changing the backup scope.',
        })}
      </div>
    </div>
  );

  return (
    <SettingsAccordion title={title} isOpen={isOpen} onToggle={onToggle}>
      <div className="settings-group" style={{ width: '100%' }}>
        <div className="settings-item">
          <label className="settings-label" htmlFor="backup-webdav-server">
            {t('settings.backup.cloud_server_url', { defaultValue: 'Server URL' })}
          </label>
          <input
            id="backup-webdav-server"
            type="text"
            className="settings-input"
            value={webDavConfig.serverUrl}
            onChange={(event) =>
              onConfigChange({
                ...webDavConfig,
                serverUrl: event.target.value,
              })
            }
            placeholder={t('settings.backup.cloud_server_placeholder', {
              defaultValue: 'https://dav.example.com/remote.php/dav/files/you',
            })}
          />
        </div>

        <div className="settings-item">
          <label className="settings-label" htmlFor="backup-webdav-directory">
            {t('settings.backup.cloud_remote_dir', { defaultValue: 'Remote Directory' })}
          </label>
          <input
            id="backup-webdav-directory"
            type="text"
            className="settings-input"
            value={webDavConfig.remoteDir}
            onChange={(event) =>
              onConfigChange({
                ...webDavConfig,
                remoteDir: event.target.value,
              })
            }
            placeholder={t('settings.backup.cloud_remote_dir_placeholder', {
              defaultValue: 'backups/sona',
            })}
          />
        </div>

        <div
          style={{
            display: 'grid',
            gap: '12px',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          }}
        >
          <div className="settings-item">
            <label className="settings-label" htmlFor="backup-webdav-username">
              {t('settings.backup.cloud_username', { defaultValue: 'Username' })}
            </label>
            <input
              id="backup-webdav-username"
              type="text"
              className="settings-input"
              value={webDavConfig.username}
              onChange={(event) =>
                onConfigChange({
                  ...webDavConfig,
                  username: event.target.value,
                })
              }
            />
          </div>

          <div className="settings-item">
            <label className="settings-label" htmlFor="backup-webdav-password">
              {t('settings.backup.cloud_password', { defaultValue: 'Password' })}
            </label>
            <input
              id="backup-webdav-password"
              type="password"
              className="settings-input"
              value={webDavConfig.password}
              onChange={(event) =>
                onConfigChange({
                  ...webDavConfig,
                  password: event.target.value,
                })
              }
            />
          </div>
        </div>

        {!webDavConfigReady ? (
          <div className="settings-hint">
            {t('settings.backup.cloud_loading', {
              defaultValue: 'Loading saved WebDAV settings...',
            })}
          </div>
        ) : null}

        {webDavConfigError ? (
          <div
            className="settings-hint"
            style={{ color: 'var(--color-danger-text, #b91c1c)' }}
          >
            {t('settings.backup.cloud_local_config_error', {
              defaultValue: 'WebDAV settings could not be saved locally: {{message}}',
              message: webDavConfigError,
            })}
          </div>
        ) : null}

        {isNotHttpsUrl(webDavConfig.serverUrl) ? (
          <div
            className="settings-hint"
            style={{ color: 'var(--color-warning-text, #b7791f)' }}
          >
            {t('settings.backup.cloud_https_required', {
              defaultValue:
                'WebDAV cloud sync requires HTTPS to protect credentials and backup archives in transit.',
            })}
          </div>
        ) : null}

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onTestConnection}
            disabled={webDavConfigActionDisabled}
          >
            {busyAction === 'webdav_test'
              ? t('settings.backup.cloud_test_busy', { defaultValue: 'Testing...' })
              : t('settings.backup.cloud_test_button', {
                  defaultValue: 'Test Connection',
                })}
          </button>

          <button
            type="button"
            className="btn btn-secondary"
            onClick={onUploadBackup}
            disabled={webDavTransferDisabled}
          >
            {busyAction === 'webdav_upload'
              ? t('settings.backup.cloud_upload_busy', { defaultValue: 'Uploading...' })
              : t('settings.backup.cloud_upload_button', {
                  defaultValue: 'Upload Backup',
                })}
          </button>

          <button
            type="button"
            className="btn btn-secondary"
            onClick={onRefreshRemoteBackups}
            disabled={webDavTransferDisabled}
          >
            {busyAction === 'webdav_refresh'
              ? t('settings.backup.cloud_refresh_busy', { defaultValue: 'Refreshing...' })
              : t('settings.backup.cloud_refresh_button', {
                  defaultValue: 'Refresh Cloud Backups',
                })}
          </button>
        </div>

        {!hasLoadedRemoteBackups ? null : remoteBackups.length > 0 ? (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '10px',
              marginTop: '4px',
            }}
          >
            <div className="settings-label" style={{ marginBottom: 0 }}>
              {t('settings.backup.cloud_list_title', {
                defaultValue: 'Remote Snapshots',
              })}
            </div>
            {remoteBackups.map((entry) => (
              <div
                key={entry.href}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: '12px',
                  padding: '12px',
                  borderRadius: 'var(--radius-md)',
                  border: '1px solid var(--color-border)',
                  background: 'var(--color-bg-elevated)',
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div
                    style={{
                      fontWeight: 500,
                      color: 'var(--color-text-primary)',
                      overflowWrap: 'anywhere',
                    }}
                  >
                    {entry.fileName}
                  </div>
                  <div className="settings-hint">
                    {formatRemoteBackupModifiedAt(entry.modifiedAt, t)} ·{' '}
                    {formatRemoteBackupSize(entry.size)}
                  </div>
                </div>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => onRestoreRemoteBackup(entry)}
                  disabled={webDavTransferDisabled}
                >
                  {busyAction === 'webdav_restore'
                    ? t('settings.backup.cloud_restore_busy', {
                        defaultValue: 'Preparing...',
                      })
                    : t('settings.backup.cloud_restore_button', {
                        defaultValue: 'Restore',
                      })}
                </button>
              </div>
            ))}
          </div>
        ) : (
          <div className="settings-hint">
            {t('settings.backup.cloud_empty', {
              defaultValue:
                'No WebDAV backup archives were found in the current remote directory.',
            })}
          </div>
        )}
      </div>
    </SettingsAccordion>
  );
}
