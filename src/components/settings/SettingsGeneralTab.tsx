import React from 'react';
import { useTranslation } from 'react-i18next';
import { Database, Languages, Stethoscope } from 'lucide-react';
import { GeneralIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { backupService } from '../../services/backupService';
import type { BackupManifestV1 } from '../../types/backup';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useUIConfig, useSetConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useTranscriptStore } from '../../stores/transcriptStore';
import type { UIConfig } from '../../types/config';
import { extractErrorMessage } from '../../utils/errorUtils';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';

interface SettingsGeneralTabProps {
    onOpenDiagnostics?: () => void;
}

type FontValue = NonNullable<UIConfig['font']>;

function getFontFamily(fontValue: string): string {
    switch (fontValue) {
        case 'mono': return 'monospace';
        case 'serif': return 'serif';
        default: return 'inherit';
    }
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
    const isRecording = useTranscriptStore((state) => state.isRecording);
    const hasBlockingQueueItems = useBatchQueueStore((state) => state.queueItems.some((item) => (
        item.status === 'pending' || item.status === 'processing'
    )));
    const [busyAction, setBusyAction] = React.useState<'export' | 'import' | null>(null);

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
            await alert(extractErrorMessage(error), {
                title: t('settings.backup.error_title', {
                    defaultValue: 'Backup failed',
                }),
                variant: 'error',
            });
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
            await alert(extractErrorMessage(error), {
                title: t('settings.backup.error_title', {
                    defaultValue: 'Backup failed',
                }),
                variant: 'error',
            });
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
