import React from 'react';
import { useTranslation } from 'react-i18next';
import {
    Box,
    Clock,
    Database,
    ExternalLink,
    Folder,
    FolderOpen,
    Globe2,
    HardDrive,
    Music,
    RefreshCw,
    RotateCcw,
    Trash2,
} from 'lucide-react';
import { Dropdown } from '../Dropdown';
import type { DropdownOption } from '../Dropdown';
import { historyService } from '../../services/historyService';
import { storageLocationService } from '../../services/storageLocationService';
import { storageUsageService } from '../../services/storageUsageService';
import { modelService } from '../../services/modelService';
import { logger } from '../../utils/logger';
import { useHistoryStorageConfig, useSetConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import type { HistoryAudioCleanupReport } from '../../types/history';
import type {
    StorageDirectoriesInfo,
    StorageUsageSnapshot,
    WebviewBrowsingDataClearResult,
} from '../../types/storage';
import { SettingsItem, SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import './SettingsShared.css';

const RETENTION_PRESETS = [
    { value: 'forever', days: null, labelKey: 'settings.storage.retention_forever', defaultLabel: 'Keep forever' },
    { value: '0', days: 0, labelKey: 'settings.storage.retention_immediate', defaultLabel: 'Remove immediately' },
    { value: '7', days: 7, labelKey: 'settings.storage.retention_7_days', defaultLabel: '7 days' },
    { value: '30', days: 30, labelKey: 'settings.storage.retention_30_days', defaultLabel: '30 days' },
    { value: '90', days: 90, labelKey: 'settings.storage.retention_90_days', defaultLabel: '90 days' },
    { value: '180', days: 180, labelKey: 'settings.storage.retention_180_days', defaultLabel: '180 days' },
    { value: '365', days: 365, labelKey: 'settings.storage.retention_365_days', defaultLabel: '365 days' },
] as const;

function retentionDaysToValue(retentionDays: number | null | undefined): string {
    return retentionDays === null || retentionDays === undefined ? 'forever' : String(retentionDays);
}

function retentionValueToDays(value: string): number | null {
    if (value === 'forever') {
        return null;
    }

    const parsed = Number(value);
    return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function reportHasCleanupWork(report: HistoryAudioCleanupReport): boolean {
    return report.eligibleCount > 0 || report.removedCount > 0 || report.missingMarkedCount > 0;
}

function formatBytes(bytes: number, locale: string | undefined): string {
    if (bytes <= 0) {
        return '0 B';
    }

    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let value = bytes;
    let unitIndex = 0;

    while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024;
        unitIndex += 1;
    }

    return `${new Intl.NumberFormat(locale || undefined, {
        maximumFractionDigits: value >= 10 || unitIndex === 0 ? 0 : 1,
    }).format(value)} ${units[unitIndex]}`;
}

function formatOptionalBytes(
    bytes: number | null | undefined,
    t: ReturnType<typeof useTranslation>['t'],
    locale: string | undefined,
): string {
    return typeof bytes === 'number'
        ? formatBytes(bytes, locale)
        : t('settings.storage.size_unknown', { defaultValue: 'Unknown' });
}

function formatGeneratedAt(value: string, locale: string | undefined): string {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
        return value;
    }

    return new Intl.DateTimeFormat(locale || undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
    }).format(date);
}

function formatFileCount(
    count: number,
    t: ReturnType<typeof useTranslation>['t'],
): string {
    return t('settings.storage.file_count', {
        defaultValue: '{{count}} files',
        count,
    });
}

function formatWebviewClearResult(
    result: WebviewBrowsingDataClearResult,
    t: ReturnType<typeof useTranslation>['t'],
    locale: string | undefined,
): string {
    return t('settings.storage.webview_clear_result', {
        defaultValue: 'WebView cleanup requested. Before: {{before}}, after: {{after}}.',
        before: formatOptionalBytes(result.beforeBytes, t, locale),
        after: formatOptionalBytes(result.afterBytes, t, locale),
    });
}

function UsageMetric({
    icon,
    title,
    value,
    detail,
}: {
    icon: React.ReactNode;
    title: string;
    value: string;
    detail: string;
}): React.JSX.Element {
    return (
        <div className="settings-storage-usage-metric">
            <div className="settings-storage-usage-icon" aria-hidden="true">
                {icon}
            </div>
            <div className="settings-storage-usage-copy">
                <div className="settings-storage-usage-title">{title}</div>
                <div className="settings-storage-usage-value">{value}</div>
                <div className="settings-storage-usage-detail">{detail}</div>
            </div>
        </div>
    );
}

function StorageDetailRow({ label, value }: { label: string; value: string }): React.JSX.Element {
    return (
        <div className="settings-storage-detail-row">
            <span>{label}</span>
            <strong>{value}</strong>
        </div>
    );
}

function formatReportSummary(
    report: HistoryAudioCleanupReport,
    t: ReturnType<typeof useTranslation>['t'],
    locale: string | undefined,
): string {
    const parts: string[] = [];

    if (report.removedCount > 0) {
        parts.push(t('settings.storage.result_removed', {
            defaultValue: '{{count}} files removed',
            count: report.removedCount,
        }));
    }

    if (report.removedBytes > 0) {
        parts.push(t('settings.storage.result_bytes', {
            defaultValue: '{{bytes}} freed',
            bytes: formatBytes(report.removedBytes, locale),
        }));
    }

    if (report.missingMarkedCount > 0) {
        parts.push(t('settings.storage.result_missing', {
            defaultValue: '{{count}} missing',
            count: report.missingMarkedCount,
        }));
    }

    if (report.failedCount > 0) {
        parts.push(t('settings.storage.result_failed', {
            defaultValue: '{{count}} failed',
            count: report.failedCount,
        }));
    }

    if (report.skippedActiveCount > 0) {
        parts.push(t('settings.storage.result_skipped_active', {
            defaultValue: '{{count}} active skipped',
            count: report.skippedActiveCount,
        }));
    }

    return parts.length > 0
        ? parts.join(' · ')
        : t('settings.storage.cleanup_noop', { defaultValue: 'No audio files need cleanup.' });
}

export function SettingsStorageTab(): React.JSX.Element {
    const { t, i18n } = useTranslation();
    const { historyAudioRetentionDays } = useHistoryStorageConfig();
    const updateConfig = useSetConfig();
    const confirm = useDialogStore((state) => state.confirm);
    const showError = useDialogStore((state) => state.showError);
    const refreshHistory = useHistoryStore((state) => state.refresh);
    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
    const [isCleaning, setIsCleaning] = React.useState(false);
    const [resultMessage, setResultMessage] = React.useState<string | null>(null);
    const [usageSnapshot, setUsageSnapshot] = React.useState<StorageUsageSnapshot | null>(null);
    const [isUsageLoading, setIsUsageLoading] = React.useState(true);
    const [usageError, setUsageError] = React.useState<string | null>(null);
    const [isClearingWebview, setIsClearingWebview] = React.useState(false);
    const [webviewResultMessage, setWebviewResultMessage] = React.useState<string | null>(null);
    const [directoriesInfo, setDirectoriesInfo] = React.useState<StorageDirectoriesInfo | null>(null);
    const [isLocationBusy, setIsLocationBusy] = React.useState(false);

    const loadDirectories = React.useCallback(async () => {
        try {
            const info = await storageLocationService.getDirectories();
            setDirectoriesInfo(info);
        } catch (error) {
            logger.error('Failed to load storage directories:', error);
        }
    }, []);

    React.useEffect(() => {
        const timer = window.setTimeout(() => {
            void loadDirectories();
        }, 0);
        return () => window.clearTimeout(timer);
    }, [loadDirectories]);

    const handleChangeDataDir = async () => {
        try {
            const selected = await storageLocationService.selectDirectory(directoriesInfo?.dataDir);
            if (!selected || selected === directoriesInfo?.dataDir) {
                return;
            }

            const confirmed = await confirm(
                t('settings.storage.data_dir_change_confirm_message', {
                    defaultValue: 'Switch data directory to:\n{{path}}\n\nSona will restart to safely reopen the database.',
                    path: selected,
                }),
                {
                    title: t('settings.storage.data_dir_change_confirm_title', { defaultValue: 'Change Data Directory?' }),
                    confirmLabel: t('settings.storage.data_dir_confirm_btn', { defaultValue: 'Confirm & Restart' }),
                    cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
                },
            );

            if (!confirmed) {
                return;
            }

            setIsLocationBusy(true);
            await storageLocationService.migrateDataDirectory(selected, true);
            await storageLocationService.relaunchApp();
        } catch (error) {
            await showError({
                code: 'storage.dir_update_failed',
                messageKey: 'settings.storage.dir_update_failed',
                cause: error,
            });
        } finally {
            setIsLocationBusy(false);
        }
    };

    const handleResetDataDir = async () => {
        if (!directoriesInfo?.defaultDataDir) return;
        try {
            const confirmed = await confirm(
                t('settings.storage.data_dir_reset_confirm_message', {
                    defaultValue: 'Restore system default data directory:\n{{path}}\n\nSona will restart to apply the change.',
                    path: directoriesInfo.defaultDataDir,
                }),
                {
                    title: t('settings.storage.data_dir_reset_confirm_title', { defaultValue: 'Restore Default Data Directory?' }),
                    confirmLabel: t('settings.storage.data_dir_confirm_btn', { defaultValue: 'Confirm & Restart' }),
                    cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
                },
            );

            if (!confirmed) {
                return;
            }

            setIsLocationBusy(true);
            await storageLocationService.resetDataDirectory();
            await storageLocationService.relaunchApp();
        } catch (error) {
            await showError({
                code: 'storage.dir_update_failed',
                messageKey: 'settings.storage.dir_update_failed',
                cause: error,
            });
        } finally {
            setIsLocationBusy(false);
        }
    };

    const handleOpenDataDir = async () => {
        if (!directoriesInfo?.dataDir) return;
        try {
            await storageLocationService.openPath(directoriesInfo.dataDir);
        } catch (error) {
            await showError({
                code: 'storage.open_folder_failed',
                messageKey: 'errors.storage.open_folder_failed',
                cause: error,
            });
        }
    };

    const handleChangeModelsDir = async () => {
        try {
            const selected = await storageLocationService.selectDirectory(directoriesInfo?.modelsDir);
            if (!selected || selected === directoriesInfo?.modelsDir) {
                return;
            }

            const confirmed = await confirm(
                t('settings.storage.models_dir_change_confirm_message', {
                    defaultValue: 'Switch model storage directory to:\n{{path}}\n\nFuture model downloads and recognition will use the new directory.',
                    path: selected,
                }),
                {
                    title: t('settings.storage.models_dir_change_confirm_title', { defaultValue: 'Change Models Directory?' }),
                    confirmLabel: t('settings.storage.models_dir_confirm_btn', { defaultValue: 'Confirm' }),
                    cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
                },
            );

            if (!confirmed) {
                return;
            }

            setIsLocationBusy(true);
            const updated = await storageLocationService.setModelsDirectory(selected, true);
            setDirectoriesInfo(updated);
            try {
                await modelService.getModelCatalogSnapshot();
            } catch (err) {
                logger.warn('Failed to refresh model catalog snapshot:', err);
            }
            await loadUsageSnapshot(true);
        } catch (error) {
            await showError({
                code: 'storage.dir_update_failed',
                messageKey: 'settings.storage.dir_update_failed',
                cause: error,
            });
        } finally {
            setIsLocationBusy(false);
        }
    };

    const handleResetModelsDir = async () => {
        if (!directoriesInfo?.defaultModelsDir) return;
        try {
            const confirmed = await confirm(
                t('settings.storage.models_dir_reset_confirm_message', {
                    defaultValue: 'Restore default models directory:\n{{path}}\n\nRestore default location?',
                    path: directoriesInfo.defaultModelsDir,
                }),
                {
                    title: t('settings.storage.models_dir_reset_confirm_title', { defaultValue: 'Restore Default Models Directory?' }),
                    confirmLabel: t('settings.storage.restore_default', { defaultValue: 'Restore Default' }),
                    cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
                },
            );

            if (!confirmed) {
                return;
            }

            setIsLocationBusy(true);
            const updated = await storageLocationService.resetModelsDirectory();
            setDirectoriesInfo(updated);
            try {
                await modelService.getModelCatalogSnapshot();
            } catch (err) {
                logger.warn('Failed to refresh model catalog snapshot:', err);
            }
            await loadUsageSnapshot(true);
        } catch (error) {
            await showError({
                code: 'storage.dir_update_failed',
                messageKey: 'settings.storage.dir_update_failed',
                cause: error,
            });
        } finally {
            setIsLocationBusy(false);
        }
    };

    const handleOpenModelsDir = async () => {
        if (!directoriesInfo?.modelsDir) return;
        try {
            await storageLocationService.openPath(directoriesInfo.modelsDir);
        } catch (error) {
            await showError({
                code: 'storage.open_folder_failed',
                messageKey: 'errors.storage.open_folder_failed',
                cause: error,
            });
        }
    };


    const loadUsageSnapshot = React.useCallback(async (quiet = false) => {
        if (!quiet) {
            setIsUsageLoading(true);
        }
        setUsageError(null);

        try {
            const snapshot = await storageUsageService.getUsageSnapshot();
            setUsageSnapshot(snapshot);
        } catch (error) {
            setUsageError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsUsageLoading(false);
        }
    }, []);

    React.useEffect(() => {
        const timer = window.setTimeout(() => {
            void loadUsageSnapshot();
        }, 0);
        return () => window.clearTimeout(timer);
    }, [loadUsageSnapshot]);

    const retentionValue = retentionDaysToValue(historyAudioRetentionDays);
    const retentionOptions = React.useMemo(() => {
        const options: DropdownOption[] = RETENTION_PRESETS.map((preset) => ({
            value: preset.value,
            label: t(preset.labelKey, { defaultValue: preset.defaultLabel }),
        }));

        if (!RETENTION_PRESETS.some((preset) => preset.value === retentionValue)) {
            options.push({
                value: retentionValue,
                label: t('settings.storage.retention_custom_days', {
                    defaultValue: '{{count}} days',
                    count: historyAudioRetentionDays ?? 0,
                }),
            });
        }

        return options;
    }, [historyAudioRetentionDays, retentionValue, t]);

    const handleRetentionChange = (value: string) => {
        updateConfig({ historyAudioRetentionDays: retentionValueToDays(value) });
        setResultMessage(null);
    };

    const handleCleanNow = async () => {
        const retentionDays = historyAudioRetentionDays ?? null;
        if (retentionDays === null) {
            setResultMessage(t('settings.storage.cleanup_disabled', {
                defaultValue: 'Choose a retention period before running manual cleanup.',
            }));
            return;
        }

        setIsCleaning(true);
        setResultMessage(null);

        try {
            const excludeHistoryId = sourceHistoryId || null;
            const preview = await historyService.previewAudioCleanup(retentionDays, excludeHistoryId);

            if (!reportHasCleanupWork(preview)) {
                setResultMessage(formatReportSummary(preview, t, i18n.language));
                return;
            }

            const confirmed = await confirm(
                t('settings.storage.cleanup_confirm_message', {
                    defaultValue: 'This removes managed history audio attachments only. Transcripts, summaries, and history records stay intact.',
                }),
                {
                    title: t('settings.storage.cleanup_confirm_title', { defaultValue: 'Clean history audio?' }),
                    details: formatReportSummary(preview, t, i18n.language),
                    confirmLabel: t('settings.storage.clean_now', { defaultValue: 'Clean Now' }),
                    cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
                },
            );

            if (!confirmed) {
                return;
            }

            const result = await historyService.cleanupAudio(retentionDays, excludeHistoryId);
            await refreshHistory();
            await loadUsageSnapshot(true);
            setResultMessage(formatReportSummary(result, t, i18n.language));
        } catch (error) {
            await showError({
                code: 'history.audio_cleanup_failed',
                messageKey: 'errors.history.audio_cleanup_failed',
                cause: error,
            });
        } finally {
            setIsCleaning(false);
        }
    };

    const handleClearWebviewData = async () => {
        const confirmed = await confirm(
            t('settings.storage.webview_clear_confirm_message', {
                defaultValue: 'This clears WebView cache and browsing data through the system WebView API. Lightweight UI state stored by the WebView may also reset.',
            }),
            {
                title: t('settings.storage.webview_clear_confirm_title', { defaultValue: 'Clear WebView browsing data?' }),
                details: t('settings.storage.webview_clear_confirm_details', {
                    defaultValue: 'Audio, database files, models, and app configuration are not deleted. Some space may only be released after restarting Sona.',
                }),
                confirmLabel: t('settings.storage.clear_webview', { defaultValue: 'Clear WebView Data' }),
                cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
            },
        );

        if (!confirmed) {
            return;
        }

        setIsClearingWebview(true);
        setWebviewResultMessage(null);

        try {
            const result = await storageUsageService.clearWebviewBrowsingData();
            setWebviewResultMessage(formatWebviewClearResult(result, t, i18n.language));
            await loadUsageSnapshot(true);
        } catch (error) {
            await showError({
                code: 'storage.webview_cleanup_failed',
                messageKey: 'errors.storage.webview_cleanup_failed',
                cause: error,
            });
        } finally {
            setIsClearingWebview(false);
        }
    };

    const manualCleanupDisabled = isCleaning || historyAudioRetentionDays === null || historyAudioRetentionDays === undefined;
    const webviewClearDisabled = isClearingWebview || !usageSnapshot?.categories.webviewCache.clearSupported;
    const sqlite = usageSnapshot?.categories.database.sqlite;
    const indexedStructureCount = sqlite?.indexEntries.length ?? 0;

    return (
        <SettingsTabContainer id="settings-panel-storage" ariaLabelledby="settings-tab-storage">
            <SettingsPageHeader
                icon={<HardDrive size={28} />}
                title={t('settings.storage.title', { defaultValue: 'Data & Storage' })}
                description={t('settings.storage.description', {
                    defaultValue: 'Review local data usage and clean managed storage without deleting transcript text.',
                })}
            />

            <SettingsSection
                title={t('settings.storage.locations_title', { defaultValue: 'Storage Locations' })}
                description={t('settings.storage.locations_description', {
                    defaultValue: 'Manage the storage directories for application data and AI speech recognition models.',
                })}
                icon={<Folder size={20} />}
            >
                <div className="settings-storage-locations-grid">
                    <div className="settings-storage-location-card" data-testid="settings-storage-data-dir-card">
                        <div className="settings-storage-location-header">
                            <div className="settings-storage-location-title-row">
                                <span className="settings-storage-location-title">
                                    {t('settings.storage.data_dir_title', { defaultValue: 'Data Directory' })}
                                </span>
                                <span className={`settings-storage-location-badge ${directoriesInfo?.isCustomDataDir ? 'custom' : 'default'}`}>
                                    {directoriesInfo?.isCustomDataDir
                                        ? t('settings.storage.badge_custom', { defaultValue: 'Custom' })
                                        : t('settings.storage.badge_default', { defaultValue: 'Default' })}
                                </span>
                            </div>
                            <p className="settings-storage-location-hint">
                                {t('settings.storage.data_dir_hint', {
                                    defaultValue: 'Stores SQLite databases, history recordings, speaker voice profiles, and recovery snapshots. Changing this requires restarting Sona.',
                                })}
                            </p>
                        </div>
                        <div
                            className="settings-storage-path-box"
                            data-tooltip={directoriesInfo?.dataDir || undefined}
                            data-tooltip-pos="top"
                            data-tooltip-multiline
                        >
                            <code>{directoriesInfo?.dataDir || t('common.loading', { defaultValue: 'Loading...' })}</code>
                        </div>
                        <div className="settings-storage-location-actions">
                            <button
                                type="button"
                                className="btn btn-secondary btn-sm"
                                onClick={() => { void handleChangeDataDir(); }}
                                disabled={isLocationBusy}
                            >
                                <FolderOpen size={14} aria-hidden="true" />
                                {t('settings.storage.browse', { defaultValue: 'Change Directory...' })}
                            </button>
                            <button
                                type="button"
                                className="btn btn-ghost btn-sm"
                                onClick={() => { void handleOpenDataDir(); }}
                                disabled={!directoriesInfo?.dataDir}
                            >
                                <ExternalLink size={14} aria-hidden="true" />
                                {t('settings.storage.open_folder', { defaultValue: 'Open Folder' })}
                            </button>
                            {directoriesInfo?.isCustomDataDir && (
                                <button
                                    type="button"
                                    className="btn btn-ghost btn-sm"
                                    onClick={() => { void handleResetDataDir(); }}
                                    disabled={isLocationBusy}
                                >
                                    <RotateCcw size={14} aria-hidden="true" />
                                    {t('settings.storage.restore_default', { defaultValue: 'Restore Default' })}
                                </button>
                            )}
                        </div>
                    </div>

                    <div className="settings-storage-location-card" data-testid="settings-storage-models-dir-card">
                        <div className="settings-storage-location-header">
                            <div className="settings-storage-location-title-row">
                                <span className="settings-storage-location-title">
                                    {t('settings.storage.models_dir_title', { defaultValue: 'Models Directory' })}
                                </span>
                                <span className={`settings-storage-location-badge ${directoriesInfo?.isCustomModelsDir ? 'custom' : 'default'}`}>
                                    {directoriesInfo?.isCustomModelsDir
                                        ? t('settings.storage.badge_custom', { defaultValue: 'Custom' })
                                        : t('settings.storage.badge_default', { defaultValue: 'Default' })}
                                </span>
                            </div>
                            <p className="settings-storage-location-hint">
                                {t('settings.storage.models_dir_hint', {
                                    defaultValue: 'Stores offline ASR speech recognition, VAD, and punctuation models. Newly downloaded models will be saved here.',
                                })}
                            </p>
                        </div>
                        <div
                            className="settings-storage-path-box"
                            data-tooltip={directoriesInfo?.modelsDir || undefined}
                            data-tooltip-pos="top"
                            data-tooltip-multiline
                        >
                            <code>{directoriesInfo?.modelsDir || t('common.loading', { defaultValue: 'Loading...' })}</code>
                        </div>
                        <div className="settings-storage-location-actions">
                            <button
                                type="button"
                                className="btn btn-secondary btn-sm"
                                onClick={() => { void handleChangeModelsDir(); }}
                                disabled={isLocationBusy}
                            >
                                <FolderOpen size={14} aria-hidden="true" />
                                {t('settings.storage.browse', { defaultValue: 'Change Directory...' })}
                            </button>
                            <button
                                type="button"
                                className="btn btn-ghost btn-sm"
                                onClick={() => { void handleOpenModelsDir(); }}
                                disabled={!directoriesInfo?.modelsDir}
                            >
                                <ExternalLink size={14} aria-hidden="true" />
                                {t('settings.storage.open_folder', { defaultValue: 'Open Folder' })}
                            </button>
                            {directoriesInfo?.isCustomModelsDir && (
                                <button
                                    type="button"
                                    className="btn btn-ghost btn-sm"
                                    onClick={() => { void handleResetModelsDir(); }}
                                    disabled={isLocationBusy}
                                >
                                    <RotateCcw size={14} aria-hidden="true" />
                                    {t('settings.storage.restore_default', { defaultValue: 'Restore Default' })}
                                </button>
                            )}
                        </div>
                    </div>
                </div>
            </SettingsSection>

            <SettingsSection
                title={t('settings.storage.usage_title', { defaultValue: 'Data Usage' })}
                description={t('settings.storage.usage_description', {
                    defaultValue: 'Storage is grouped by local audio, SQLite database files, models, temporary files, WebView cache, and other app data.',
                })}
                icon={<HardDrive size={20} />}
            >
                <div className="settings-storage-usage-panel">
                    <div className="settings-storage-overview-header">
                        <div className="settings-storage-total">
                            <span className="settings-storage-total-label">
                                {t('settings.storage.total_usage', { defaultValue: 'Total local usage' })}
                            </span>
                            <strong className="settings-storage-total-value">
                                {usageSnapshot
                                    ? formatBytes(usageSnapshot.totalBytes, i18n.language)
                                    : t('common.loading', { defaultValue: 'Loading...' })}
                            </strong>
                            {usageSnapshot && (
                                <span className="settings-storage-total-meta">
                                    {t('settings.storage.generated_at', {
                                        defaultValue: 'Updated {{time}}',
                                        time: formatGeneratedAt(usageSnapshot.generatedAt, i18n.language),
                                    })}
                                </span>
                            )}
                        </div>
                        <button
                            type="button"
                            className="btn btn-secondary settings-storage-refresh-button"
                            onClick={() => { void loadUsageSnapshot(); }}
                            disabled={isUsageLoading}
                        >
                            <RefreshCw size={16} aria-hidden="true" />
                            {isUsageLoading
                                ? t('settings.storage.refreshing', { defaultValue: 'Refreshing...' })
                                : t('settings.storage.refresh', { defaultValue: 'Refresh' })}
                        </button>
                    </div>

                    {usageError && (
                        <div className="settings-storage-usage-error" role="alert">
                            <strong>{t('settings.storage.usage_error_title', { defaultValue: 'Storage usage unavailable' })}</strong>
                            <span>{usageError}</span>
                        </div>
                    )}

                    {usageSnapshot && (
                        <>
                            <div className="settings-storage-usage-grid">
                                <UsageMetric
                                    icon={<Music size={18} />}
                                    title={t('settings.storage.category_audio', { defaultValue: 'Audio' })}
                                    value={formatBytes(usageSnapshot.categories.audio.bytes, i18n.language)}
                                    detail={t('settings.storage.audio_detail', {
                                        defaultValue: 'History {{history}} · Speaker samples {{samples}}',
                                        history: formatBytes(usageSnapshot.categories.audio.historyAudioBytes, i18n.language),
                                        samples: formatBytes(usageSnapshot.categories.audio.speakerSampleBytes, i18n.language),
                                    })}
                                />
                                <UsageMetric
                                    icon={<Database size={18} />}
                                    title={t('settings.storage.category_database', { defaultValue: 'Database' })}
                                    value={formatBytes(usageSnapshot.categories.database.bytes, i18n.language)}
                                    detail={t('settings.storage.database_detail', {
                                        defaultValue: 'SQLite data {{data}} · indexes {{indexes}}',
                                        data: sqlite ? formatBytes(sqlite.dataBytes, i18n.language) : '0 B',
                                        indexes: sqlite ? formatBytes(sqlite.indexBytes, i18n.language) : '0 B',
                                    })}
                                />
                                <UsageMetric
                                    icon={<Box size={18} />}
                                    title={t('settings.storage.category_models', { defaultValue: 'Models' })}
                                    value={formatBytes(usageSnapshot.categories.models.bytes, i18n.language)}
                                    detail={formatFileCount(usageSnapshot.categories.models.fileCount, t)}
                                />
                                <UsageMetric
                                    icon={<Clock size={18} />}
                                    title={t('settings.storage.category_temporary', { defaultValue: 'Temporary' })}
                                    value={formatBytes(usageSnapshot.categories.temporary.bytes, i18n.language)}
                                    detail={formatFileCount(usageSnapshot.categories.temporary.fileCount, t)}
                                />
                                <UsageMetric
                                    icon={<Globe2 size={18} />}
                                    title={t('settings.storage.category_webview_cache', { defaultValue: 'WebView Cache' })}
                                    value={formatOptionalBytes(usageSnapshot.categories.webviewCache.bytes, t, i18n.language)}
                                    detail={usageSnapshot.categories.webviewCache.path
                                        ? t('settings.storage.webview_observed', { defaultValue: 'Observable cache directory' })
                                        : t('settings.storage.webview_unobserved', { defaultValue: 'Size unavailable on this platform' })}
                                />
                                <UsageMetric
                                    icon={<HardDrive size={18} />}
                                    title={t('settings.storage.category_other', { defaultValue: 'Other' })}
                                    value={formatBytes(usageSnapshot.categories.other.bytes, i18n.language)}
                                    detail={formatFileCount(usageSnapshot.categories.other.fileCount, t)}
                                />
                            </div>

                            <div className="settings-storage-detail-grid">
                                <div className="settings-storage-detail-block">
                                    <div className="settings-storage-detail-heading">
                                        {t('settings.storage.database_breakdown_title', { defaultValue: 'SQLite breakdown' })}
                                    </div>
                                    <StorageDetailRow
                                        label={t('settings.storage.sqlite_data_bytes', { defaultValue: 'Table data' })}
                                        value={sqlite ? formatBytes(sqlite.dataBytes, i18n.language) : '0 B'}
                                    />
                                    <StorageDetailRow
                                        label={t('settings.storage.sqlite_index_bytes', { defaultValue: 'SQLite indexes' })}
                                        value={sqlite ? formatBytes(sqlite.indexBytes, i18n.language) : '0 B'}
                                    />
                                    <StorageDetailRow
                                        label={t('settings.storage.sqlite_free_page_bytes', { defaultValue: 'Free pages' })}
                                        value={sqlite ? formatBytes(sqlite.freePageBytes, i18n.language) : '0 B'}
                                    />
                                    <StorageDetailRow
                                        label={t('settings.storage.sqlite_index_entries', { defaultValue: 'Indexed structures' })}
                                        value={t('settings.storage.indexed_structure_count', {
                                            defaultValue: '{{count}} structures',
                                            count: indexedStructureCount,
                                        })}
                                    />
                                    <StorageDetailRow
                                        label={t('settings.storage.sqlite_main_files', { defaultValue: 'Main DB + WAL/SHM' })}
                                        value={sqlite
                                            ? formatBytes(sqlite.mainDbBytes + sqlite.mainWalBytes + sqlite.mainShmBytes, i18n.language)
                                            : '0 B'}
                                    />
                                    <StorageDetailRow
                                        label={t('settings.storage.sqlite_analytics_files', { defaultValue: 'Analytics DB + WAL/SHM' })}
                                        value={sqlite
                                            ? formatBytes(sqlite.analyticsDbBytes + sqlite.analyticsWalBytes + sqlite.analyticsShmBytes, i18n.language)
                                            : '0 B'}
                                    />
                                </div>

                                <div className="settings-storage-detail-block">
                                    <div className="settings-storage-detail-heading">
                                        {t('settings.storage.webview_title', { defaultValue: 'WebView cache and browsing data' })}
                                    </div>
                                    <p className="settings-storage-detail-copy">
                                        {t('settings.storage.webview_description', {
                                            defaultValue: 'Clears WebView cache and browsing data through Tauri. Some space may only be released after restarting Sona.',
                                        })}
                                    </p>
                                    <button
                                        type="button"
                                        className="btn btn-secondary settings-storage-clean-button"
                                        onClick={() => { void handleClearWebviewData(); }}
                                        disabled={webviewClearDisabled}
                                    >
                                        <Trash2 size={16} aria-hidden="true" />
                                        {isClearingWebview
                                            ? t('settings.storage.clearing_webview', { defaultValue: 'Clearing...' })
                                            : t('settings.storage.clear_webview', { defaultValue: 'Clear WebView Data' })}
                                    </button>
                                    {webviewResultMessage && (
                                        <div
                                            className="settings-storage-webview-result"
                                            data-testid="settings-storage-webview-result"
                                            aria-live="polite"
                                        >
                                            {webviewResultMessage}
                                        </div>
                                    )}
                                </div>
                            </div>
                        </>
                    )}
                </div>
            </SettingsSection>

            <SettingsSection
                title={t('settings.storage.history_audio_title', { defaultValue: 'History Audio' })}
                description={t('settings.storage.history_audio_description', {
                    defaultValue: 'Cleanup affects only managed audio files. Saved transcripts, summaries, and history rows remain available.',
                })}
                icon={<HardDrive size={20} />}
            >
                <SettingsItem
                    title={t('settings.storage.retention_label', { defaultValue: 'Audio retention' })}
                    hint={t('settings.storage.retention_hint', {
                        defaultValue: 'Automatic cleanup runs after startup and then at most once per day while Sona stays open.',
                    })}
                >
                    <div className="settings-storage-retention-control">
                        <Dropdown
                            id="settings-history-audio-retention"
                            aria-label={t('settings.storage.retention_label', { defaultValue: 'Audio retention' })}
                            value={retentionValue}
                            onChange={handleRetentionChange}
                            options={retentionOptions}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.storage.manual_cleanup_title', { defaultValue: 'Manual cleanup' })}
                    hint={manualCleanupDisabled && !isCleaning
                        ? t('settings.storage.manual_cleanup_disabled_hint', {
                            defaultValue: 'Select a finite retention period to clean audio now.',
                        })
                        : t('settings.storage.manual_cleanup_hint', {
                            defaultValue: 'Preview eligible audio first, then confirm before anything is removed.',
                        })}
                >
                    <button
                        type="button"
                        className="btn btn-secondary settings-storage-clean-button"
                        onClick={() => { void handleCleanNow(); }}
                        disabled={manualCleanupDisabled}
                    >
                        <Trash2 size={16} aria-hidden="true" />
                        {isCleaning
                            ? t('settings.storage.cleaning', { defaultValue: 'Cleaning...' })
                            : t('settings.storage.clean_now', { defaultValue: 'Clean Now' })}
                    </button>
                </SettingsItem>

                {resultMessage && (
                    <div
                        className="settings-storage-result"
                        data-testid="settings-storage-cleanup-result"
                        aria-live="polite"
                    >
                        {resultMessage}
                    </div>
                )}
            </SettingsSection>
        </SettingsTabContainer>
    );
}
