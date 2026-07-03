import React from 'react';
import { useTranslation } from 'react-i18next';
import { HardDrive, Trash2 } from 'lucide-react';
import { Dropdown } from '../Dropdown';
import type { DropdownOption } from '../Dropdown';
import { historyService } from '../../services/historyService';
import { useHistoryStorageConfig, useSetConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import type { HistoryAudioCleanupReport } from '../../types/history';
import { SettingsItem, SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';

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

    const manualCleanupDisabled = isCleaning || historyAudioRetentionDays === null || historyAudioRetentionDays === undefined;

    return (
        <SettingsTabContainer id="settings-panel-storage" ariaLabelledby="settings-tab-storage">
            <SettingsPageHeader
                icon={<HardDrive size={28} />}
                title={t('settings.storage.title', { defaultValue: 'Storage' })}
                description={t('settings.storage.description', {
                    defaultValue: 'Control how long history audio attachments are kept while preserving transcript text.',
                })}
            />

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
