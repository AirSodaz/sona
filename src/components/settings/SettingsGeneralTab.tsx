import React from 'react';
import { useTranslation } from 'react-i18next';
import { Database, Languages, Stethoscope } from 'lucide-react';
import { GeneralIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { backupService } from '../../services/backupService';
import { backupWebDavService } from '../../services/backupWebDavService';
import type { BackupManifestV1, BackupWebDavConfig, PreparedBackupImport, RemoteBackupEntry } from '../../types/backup';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useUIConfig, useSetConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import type { UIConfig } from '../../types/config';
import { extractErrorMessage } from '../../utils/errorUtils';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader, SettingsAccordion } from './SettingsLayout';

interface SettingsGeneralTabProps {
    onOpenDiagnostics?: () => void;
}

type FontValue = NonNullable<UIConfig['font']>;
type BackupBusyAction =
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

function getFontFamily(fontValue: string): string {
    switch (fontValue) {
        case 'mono': return 'monospace';
        case 'serif': return 'serif';
        default: return 'inherit';
    }
}

function isHttpUrl(value: string): boolean {
    return value.trim().toLowerCase().startsWith('http://');
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
        return t('settings.backup.cloud_unknown_time', { defaultValue: 'Unknown time' });
    }

    const date = new Date(modifiedAt);
    if (Number.isNaN(date.getTime())) {
        return modifiedAt;
    }

    return date.toLocaleString();
}

function buildImportDetails(t: (key: string, options?: Record<string, unknown>) => string, manifest: BackupManifestV1): string {
    return [
        t('settings.backup.import_details_scope', {
            defaultValue: 'This archive will replace config, workspace, light history, automation state, and dashboard LLM usage on this device.',
        }),
        t('settings.backup.import_details_audio_warning', {
            defaultValue: 'Light history backups do not include audio files, so restored items may reopen without playback.',
        }),
        '',
        t('settings.backup.summary_title', {
            defaultValue: 'Archive summary',
        }),
        t('settings.backup.summary_projects', {
            defaultValue: 'Projects: {{count}}',
            count: manifest.counts.projects,
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

export function SettingsGeneralTab({ onOpenDiagnostics }: SettingsGeneralTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const config = useUIConfig();
    const updateConfig = useSetConfig();
    const alert = useDialogStore((state) => state.alert);
    const confirm = useDialogStore((state) => state.confirm);
    const showError = useDialogStore((state) => state.showError);
    const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
    const hasBlockingQueueItems = useBatchQueueStore((state) => state.queueItems.some((item) => (
        item.status === 'pending' || item.status === 'processing'
    )));
    const [busyAction, setBusyAction] = React.useState<BackupBusyAction | null>(null);
    const [webDavConfig, setWebDavConfig] = React.useState<BackupWebDavConfig>(EMPTY_WEBDAV_CONFIG);
    const [webDavConfigReady, setWebDavConfigReady] = React.useState(false);
    const [webDavConfigError, setWebDavConfigError] = React.useState<string | null>(null);
    const [remoteBackups, setRemoteBackups] = React.useState<RemoteBackupEntry[]>([]);
    const [hasLoadedRemoteBackups, setHasLoadedRemoteBackups] = React.useState(false);

    const appLanguage = config.appLanguage || 'auto';
    const theme = config.theme || 'auto';
    const font = config.font || 'system';
    const minimizeToTrayOnExit = config.minimizeToTrayOnExit ?? true;
    const autoCheckUpdates = config.autoCheckUpdates ?? true;
    const backupBlocker = isRecording
        ? 'recording'
        : hasBlockingQueueItems
            ? 'batch_queue'
            : null;
    const isBackupBlocked = backupBlocker !== null;
    const backupBlockerHint = backupBlocker === 'recording'
        ? t('settings.backup.blocked_recording', {
            defaultValue: 'Stop Live Record before exporting or importing backups.',
        })
        : backupBlocker === 'batch_queue'
            ? t('settings.backup.blocked_batch', {
                defaultValue: 'Wait for Batch Import to finish or clear pending items before exporting or importing backups.',
            })
            : t('settings.backup.description', {
                defaultValue: 'Create or restore a single archive containing config, workspace, light history, automation state, and dashboard LLM usage.',
            });
    const webDavTransferDisabled = busyAction !== null || isBackupBlocked || !webDavConfigReady;
    const webDavConfigActionDisabled = busyAction !== null || !webDavConfigReady;
    const webDavAccordionTitle = (
        <div className="settings-accordion-copy">
            <div className="settings-accordion-copy-title">
                {t('settings.backup.cloud_title', { defaultValue: 'WebDAV Cloud Sync' })}
            </div>
            <div className="settings-accordion-copy-hint">
                {t('settings.backup.cloud_hint', {
                    defaultValue: 'Save WebDAV credentials locally on this device, upload backup archives manually, and restore from any remote snapshot without changing the backup scope.',
                })}
            </div>
        </div>
    );
    const showBackupError = React.useCallback((code: string, messageKey: string, cause: unknown) => (
        showError({
            code,
            messageKey,
            cause,
            titleKey: 'settings.backup.error_title',
        })
    ), [showError]);

    React.useEffect(() => {
        let cancelled = false;

        void backupWebDavService.loadConfig()
            .then((loadedConfig) => {
                if (cancelled) {
                    return;
                }

                setWebDavConfig(loadedConfig);
                setWebDavConfigReady(true);
            })
            .catch((error) => {
                if (cancelled) {
                    return;
                }

                setWebDavConfigReady(true);
                setWebDavConfigError(extractErrorMessage(error));
            });

        return () => {
            cancelled = true;
        };
    }, []);

    const persistWebDavConfig = React.useCallback((nextConfig: BackupWebDavConfig) => {
        setWebDavConfig(nextConfig);
        setWebDavConfigError(null);
        setRemoteBackups([]);
        setHasLoadedRemoteBackups(false);

        void backupWebDavService.saveConfig(nextConfig).catch((error) => {
            setWebDavConfigError(extractErrorMessage(error));
        });
    }, []);

    const handleExportBackup = async () => {
        setBusyAction('export');
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
            await showBackupError('backup.export_failed', 'errors.backup.export_failed', error);
        } finally {
            setBusyAction(null);
        }
    };

    const handleImportBackup = async () => {
        setBusyAction('import');
        try {
            const prepared = await backupService.prepareImportBackup();
            if (!prepared) {
                return;
            }

            const confirmed = await confirm(
                t('settings.backup.import_confirm_message', {
                    defaultValue: 'Import this backup and replace the current local data?',
                }),
                {
                    title: t('settings.backup.import_confirm_title', {
                        defaultValue: 'Replace current data',
                    }),
                    details: buildImportDetails(t, prepared.manifest),
                    confirmLabel: t('settings.backup.import_button', {
                        defaultValue: 'Import Backup',
                    }),
                    cancelLabel: t('common.cancel'),
                },
            );

            if (!confirmed) {
                await backupService.disposePreparedImport(prepared);
                return;
            }

            await backupService.applyImportBackup(prepared);
            await alert(
                t('settings.backup.import_success', {
                    defaultValue: 'Backup archive imported successfully.',
                }),
                {
                    variant: 'success',
                },
            );
        } catch (error) {
            await showBackupError('backup.import_failed', 'errors.backup.import_failed', error);
        } finally {
            setBusyAction(null);
        }
    };

    const handleTestWebDavConnection = async () => {
        setBusyAction('webdav_test');
        try {
            const result = await backupWebDavService.testConnection(webDavConfig);
            await alert(result.message, {
                title: t('settings.backup.cloud_test_title', { defaultValue: 'WebDAV connection' }),
                variant: result.status === 'warning' ? 'warning' : 'success',
            });
        } catch (error) {
            await showBackupError('backup.webdav_test_failed', 'errors.backup.webdav_test_failed', error);
        } finally {
            setBusyAction(null);
        }
    };

    const handleRefreshRemoteBackups = async () => {
        setBusyAction('webdav_refresh');
        try {
            const backups = await backupWebDavService.listBackups(webDavConfig);
            setRemoteBackups(backups);
            setHasLoadedRemoteBackups(true);
        } catch (error) {
            await showBackupError('backup.webdav_refresh_failed', 'errors.backup.webdav_refresh_failed', error);
        } finally {
            setBusyAction(null);
        }
    };

    const handleUploadWebDavBackup = async () => {
        setBusyAction('webdav_upload');
        try {
            const result = await backupWebDavService.uploadBackup(webDavConfig);
            const backups = await backupWebDavService.listBackups(webDavConfig);
            setRemoteBackups(backups);
            setHasLoadedRemoteBackups(true);
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
            await showBackupError('backup.webdav_upload_failed', 'errors.backup.webdav_upload_failed', error);
        } finally {
            setBusyAction(null);
        }
    };

    const handleRestoreRemoteBackup = async (entry: RemoteBackupEntry) => {
        setBusyAction('webdav_restore');
        let prepared: PreparedBackupImport | null = null;

        try {
            prepared = await backupWebDavService.prepareImportFromRemote(entry, webDavConfig);

            const confirmed = await confirm(
                t('settings.backup.import_confirm_message', {
                    defaultValue: 'Import this backup and replace the current local data?',
                }),
                {
                    title: t('settings.backup.import_confirm_title', {
                        defaultValue: 'Replace current data',
                    }),
                    details: buildImportDetails(t, prepared.manifest),
                    confirmLabel: t('settings.backup.cloud_restore_button', {
                        defaultValue: 'Restore',
                    }),
                    cancelLabel: t('common.cancel'),
                },
            );

            if (!confirmed) {
                await backupService.disposePreparedImport(prepared);
                prepared = null;
                return;
            }

            await backupService.applyImportBackup(prepared);
            prepared = null;
            await alert(
                t('settings.backup.import_success', {
                    defaultValue: 'Backup archive imported successfully.',
                }),
                {
                    variant: 'success',
                },
            );
        } catch (error) {
            if (prepared) {
                await backupService.disposePreparedImport(prepared).catch(() => undefined);
            }

            await showBackupError('backup.webdav_restore_failed', 'errors.backup.webdav_restore_failed', error);
        } finally {
            setBusyAction(null);
        }
    };

    return (
        <SettingsTabContainer id="settings-panel-general" ariaLabelledby="settings-tab-general">
            <SettingsPageHeader 
                icon={<GeneralIcon width={28} height={28} />}
                title={t('settings.general')} 
                description={t('settings.general_description')} 
            />
            <SettingsSection
                title={t('settings.general_title')}
                icon={<Languages size={20} />}
            >
                <SettingsItem
                    title={t('settings.language')}
                    hint={t('settings.language_hint')}
                >
                    <div style={{ width: '200px' }}>
                        <Dropdown
                            id="settings-language"
                            value={appLanguage}
                            onChange={(value) => updateConfig({ appLanguage: value as 'auto' | 'en' | 'zh' })}
                            options={[
                                { value: 'auto', label: t('common.auto') },
                                { value: 'en', label: t('settings.language_en') },
                                { value: 'zh', label: t('settings.language_zh') }
                            ]}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.theme')}
                    layout="vertical"
                >
                    <div className="theme-selector-container">
                        <button
                            className={`theme-card ${theme === 'light' ? 'active' : ''}`}
                            onClick={() => updateConfig({ theme: 'light' })}
                            aria-label={t('settings.theme_light')}
                            aria-pressed={theme === 'light'}
                        >
                            <div className="theme-preview light">
                                <div className="theme-preview-window">
                                    <div className="theme-preview-lines">
                                        <div className="line short"></div>
                                        <div className="line long"></div>
                                    </div>
                                    <div className="theme-preview-sidebar"></div>
                                </div>
                            </div>
                            <span className="theme-label">{t('settings.theme_light')}</span>
                        </button>

                        <button
                            className={`theme-card ${theme === 'dark' ? 'active' : ''}`}
                            onClick={() => updateConfig({ theme: 'dark' })}
                            aria-label={t('settings.theme_dark')}
                            aria-pressed={theme === 'dark'}
                        >
                            <div className="theme-preview dark">
                                <div className="theme-preview-window">
                                    <div className="theme-preview-lines">
                                        <div className="line short"></div>
                                        <div className="line long"></div>
                                    </div>
                                    <div className="theme-preview-sidebar"></div>
                                </div>
                            </div>
                            <span className="theme-label">{t('settings.theme_dark')}</span>
                        </button>

                        <button
                            className={`theme-card ${theme === 'auto' ? 'active' : ''}`}
                            onClick={() => updateConfig({ theme: 'auto' })}
                            aria-label={t('common.auto')}
                            aria-pressed={theme === 'auto'}
                        >
                            <div className="theme-preview auto">
                                <div className="theme-preview-window">
                                    <div className="theme-preview-lines">
                                        <div className="line short"></div>
                                        <div className="line long"></div>
                                    </div>
                                    <div className="theme-preview-sidebar"></div>
                                </div>
                            </div>
                            <span className="theme-label">{t('common.auto')}</span>
                        </button>
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.font')}
                >
                    <div style={{ width: '240px' }}>
                        <Dropdown
                            id="settings-font"
                            value={font}
                            onChange={(value) => updateConfig({ font: value as FontValue })}
                            options={[
                                { value: 'system', label: t('settings.font_system'), style: { fontFamily: 'inherit' } },
                                { value: 'serif', label: t('settings.font_serif'), style: { fontFamily: 'serif' } },
                                { value: 'sans', label: t('settings.font_sans'), style: { fontFamily: 'sans-serif' } },
                                { value: 'mono', label: t('settings.font_mono'), style: { fontFamily: 'monospace' } },
                                { value: 'arial', label: t('settings.font_arial'), style: { fontFamily: 'Arial' } },
                                { value: 'georgia', label: t('settings.font_georgia'), style: { fontFamily: 'Georgia' } }
                            ]}
                            style={{ fontFamily: getFontFamily(font) }}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <SettingsSection>
                <SettingsItem
                    title={t('settings.minimize_to_tray')}
                    hint={t('settings.minimize_to_tray_hint')}
                >
                    <Switch
                        checked={minimizeToTrayOnExit}
                        onChange={(enabled) => updateConfig({ minimizeToTrayOnExit: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.auto_check_updates')}
                >
                    <Switch
                        checked={autoCheckUpdates}
                        onChange={(enabled) => updateConfig({ autoCheckUpdates: enabled })}
                    />
                </SettingsItem>
            </SettingsSection>

            <SettingsSection
                title={t('settings.backup.title', { defaultValue: 'Backup & Restore' })}
                description={backupBlockerHint}
                icon={<Database size={20} />}
            >
                <SettingsItem
                    title={t('settings.backup.export_title', { defaultValue: 'Export Backup' })}
                    hint={t('settings.backup.export_hint', {
                        defaultValue: 'Create one .tar.bz2 archive containing config, workspace, light history transcripts, summaries, automation state, and dashboard LLM usage. Audio files, onboarding, current project, and recovery state are excluded.',
                    })}
                >
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={handleExportBackup}
                        disabled={busyAction !== null || isBackupBlocked}
                    >
                        {busyAction === 'export'
                            ? t('settings.backup.export_busy', { defaultValue: 'Exporting...' })
                            : t('settings.backup.export_button', { defaultValue: 'Export Backup' })}
                    </button>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.backup.import_title', { defaultValue: 'Import Backup' })}
                    hint={t('settings.backup.import_hint', {
                        defaultValue: 'Replace the current config, workspace, light history, automation state, and dashboard LLM usage from a backup archive. Imported light history does not restore audio playback files.',
                    })}
                >
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={handleImportBackup}
                        disabled={busyAction !== null || isBackupBlocked}
                    >
                        {busyAction === 'import'
                            ? t('settings.backup.import_busy', { defaultValue: 'Importing...' })
                            : t('settings.backup.import_button', { defaultValue: 'Import Backup' })}
                    </button>
                </SettingsItem>

                <SettingsAccordion title={webDavAccordionTitle}>
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
                                onChange={(event) => persistWebDavConfig({
                                    ...webDavConfig,
                                    serverUrl: event.target.value,
                                })}
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
                                onChange={(event) => persistWebDavConfig({
                                    ...webDavConfig,
                                    remoteDir: event.target.value,
                                })}
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
                                    onChange={(event) => persistWebDavConfig({
                                        ...webDavConfig,
                                        username: event.target.value,
                                    })}
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
                                    onChange={(event) => persistWebDavConfig({
                                        ...webDavConfig,
                                        password: event.target.value,
                                    })}
                                />
                            </div>
                        </div>

                        {!webDavConfigReady ? (
                            <div className="settings-hint">
                                {t('settings.backup.cloud_loading', { defaultValue: 'Loading saved WebDAV settings...' })}
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

                        {isHttpUrl(webDavConfig.serverUrl) ? (
                            <div
                                className="settings-hint"
                                style={{ color: 'var(--color-warning-text, #b7791f)' }}
                            >
                                {t('settings.backup.cloud_http_warning', {
                                    defaultValue: 'This WebDAV endpoint uses HTTP, so credentials and backup archives are not protected in transit.',
                                })}
                            </div>
                        ) : null}

                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
                            <button
                                type="button"
                                className="btn btn-secondary"
                                onClick={handleTestWebDavConnection}
                                disabled={webDavConfigActionDisabled}
                            >
                                {busyAction === 'webdav_test'
                                    ? t('settings.backup.cloud_test_busy', { defaultValue: 'Testing...' })
                                    : t('settings.backup.cloud_test_button', { defaultValue: 'Test Connection' })}
                            </button>

                            <button
                                type="button"
                                className="btn btn-secondary"
                                onClick={handleUploadWebDavBackup}
                                disabled={webDavTransferDisabled}
                            >
                                {busyAction === 'webdav_upload'
                                    ? t('settings.backup.cloud_upload_busy', { defaultValue: 'Uploading...' })
                                    : t('settings.backup.cloud_upload_button', { defaultValue: 'Upload Backup' })}
                            </button>

                            <button
                                type="button"
                                className="btn btn-secondary"
                                onClick={handleRefreshRemoteBackups}
                                disabled={webDavTransferDisabled}
                            >
                                {busyAction === 'webdav_refresh'
                                    ? t('settings.backup.cloud_refresh_busy', { defaultValue: 'Refreshing...' })
                                    : t('settings.backup.cloud_refresh_button', { defaultValue: 'Refresh Cloud Backups' })}
                            </button>
                        </div>

                        {hasLoadedRemoteBackups ? (
                            remoteBackups.length > 0 ? (
                                <div
                                    style={{
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '10px',
                                        marginTop: '4px',
                                    }}
                                >
                                    <div className="settings-label" style={{ marginBottom: 0 }}>
                                        {t('settings.backup.cloud_list_title', { defaultValue: 'Remote Snapshots' })}
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
                                                    {formatRemoteBackupModifiedAt(entry.modifiedAt, t)} · {formatRemoteBackupSize(entry.size)}
                                                </div>
                                            </div>
                                            <button
                                                type="button"
                                                className="btn btn-secondary"
                                                onClick={() => handleRestoreRemoteBackup(entry)}
                                                disabled={webDavTransferDisabled}
                                            >
                                                {busyAction === 'webdav_restore'
                                                    ? t('settings.backup.cloud_restore_busy', { defaultValue: 'Preparing...' })
                                                    : t('settings.backup.cloud_restore_button', { defaultValue: 'Restore' })}
                                            </button>
                                        </div>
                                    ))}
                                </div>
                            ) : (
                                <div className="settings-hint">
                                    {t('settings.backup.cloud_empty', {
                                        defaultValue: 'No WebDAV backup archives were found in the current remote directory.',
                                    })}
                                </div>
                            )
                        ) : null}
                    </div>
                </SettingsAccordion>
            </SettingsSection>

            <SettingsSection
                title={t('settings.diagnostics.title', { defaultValue: 'Model & Environment Diagnostics' })}
                description={t('settings.diagnostics.entry_description', {
                    defaultValue: 'Open a dedicated diagnostics page for the local transcription path, runtime readiness, and packaged environment checks.',
                })}
                icon={<Stethoscope size={20} />}
            >
                <SettingsItem
                    title={t('settings.diagnostics.entry_title', { defaultValue: 'Diagnostics' })}
                    hint={t('settings.diagnostics.entry_hint', {
                        defaultValue: 'Review the current local setup and jump straight to the clearest fix when something is off.',
                    })}
                >
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={onOpenDiagnostics}
                        disabled={!onOpenDiagnostics}
                    >
                        {t('settings.diagnostics.open_button', { defaultValue: 'Open Diagnostics' })}
                    </button>
                </SettingsItem>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
