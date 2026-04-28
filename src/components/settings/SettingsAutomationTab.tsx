import React, { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { AutomationIcon, FolderIcon, PauseIcon, PlayIcon, TrashIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { useAutomationStore } from '../../stores/automationStore';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useDialogStore } from '../../stores/dialogStore';
import { getPolishPresetOptions } from '../../utils/polishPresets';
import { SettingsItem, SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import type {
    AutomationRule,
    AutomationRuntimeStatus,
} from '../../types/automation';
import type { ExportFormat, ExportMode } from '../../utils/exportFormats';

const LANGUAGE_OPTIONS = ['zh', 'en', 'ja', 'ko', 'fr', 'de', 'es'];

interface AutomationRuleDraft {
    id?: string;
    name: string;
    projectId: string;
    presetId: AutomationRule['presetId'];
    watchDirectory: string;
    recursive: boolean;
    enabled: boolean;
    stageConfig: AutomationRule['stageConfig'];
    exportConfig: AutomationRule['exportConfig'];
}

const NEW_RULE_KEY = '__new__';

function normalizeExportMode(autoTranslate: boolean, mode: ExportMode): ExportMode {
    if (!autoTranslate && (mode === 'translation' || mode === 'bilingual')) {
        return 'original';
    }

    return mode;
}

function normalizeAutomationRuleDraft(draft: AutomationRuleDraft): AutomationRuleDraft {
    const normalizedMode = normalizeExportMode(draft.stageConfig.autoTranslate, draft.exportConfig.mode);

    const stageConfig = {
        polishPresetId: 'general',
        translationLanguage: 'en',
        ...draft.stageConfig,
    };

    const exportConfig = {
        prefix: '',
        ...draft.exportConfig,
        mode: normalizedMode,
    };

    if (
        normalizedMode === draft.exportConfig.mode &&
        stageConfig.polishPresetId === draft.stageConfig.polishPresetId &&
        stageConfig.translationLanguage === draft.stageConfig.translationLanguage &&
        exportConfig.prefix === draft.exportConfig.prefix
    ) {
        return draft;
    }

    return {
        ...draft,
        stageConfig,
        exportConfig,
    };
}

function createRuleDraft(projectId: string): AutomationRuleDraft {
    return normalizeAutomationRuleDraft({
        name: '',
        projectId,
        presetId: 'custom',
        watchDirectory: '',
        recursive: false,
        enabled: false,
        stageConfig: {
            autoPolish: false,
            polishPresetId: 'general',
            autoTranslate: false,
            translationLanguage: 'en',
            exportEnabled: false,
        },
        exportConfig: {
            directory: '',
            format: 'txt',
            mode: 'original',
            prefix: '',
        },
    });
}

function createDraftFromRule(rule: AutomationRule): AutomationRuleDraft {
    return normalizeAutomationRuleDraft({
        id: rule.id,
        name: rule.name,
        projectId: rule.projectId,
        presetId: rule.presetId,
        watchDirectory: rule.watchDirectory,
        recursive: rule.recursive,
        enabled: rule.enabled,
        stageConfig: {
            polishPresetId: 'general',
            translationLanguage: 'en',
            ...rule.stageConfig,
        },
        exportConfig: {
            prefix: '',
            ...rule.exportConfig,
        },
    });
}

function SummaryChip({
    label,
    tone = 'neutral',
}: {
    label: string;
    tone?: 'neutral' | 'warning' | 'danger' | 'success';
}): React.JSX.Element {
    const chipColors = {
        neutral: {
            background: 'var(--color-bg-secondary)',
            color: 'var(--color-text-muted)',
        },
        warning: {
            background: 'var(--color-warning-bg, rgba(245, 158, 11, 0.12))',
            color: 'var(--color-warning-text, #b7791f)',
        },
        danger: {
            background: 'var(--color-danger-bg, rgba(239, 68, 68, 0.12))',
            color: 'var(--color-danger-text, #b91c1c)',
        },
        success: {
            background: 'var(--color-success-bg, rgba(16, 185, 129, 0.12))',
            color: 'var(--color-success-text, #047857)',
        },
    } as const;

    return (
        <span
            style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                padding: '2px 8px',
                borderRadius: '999px',
                fontSize: '0.75rem',
                fontWeight: 500,
                lineHeight: 1.4,
                ...chipColors[tone],
            }}
        >
            {label}
        </span>
    );
}

export function SettingsAutomationTab(): React.JSX.Element {
    const { t } = useTranslation();
    const rules = useAutomationStore((state) => state.rules);
    const runtimeStates = useAutomationStore((state) => state.runtimeStates);
    const saveRule = useAutomationStore((state) => state.saveRule);
    const deleteRule = useAutomationStore((state) => state.deleteRule);
    const toggleRuleEnabled = useAutomationStore((state) => state.toggleRuleEnabled);
    const scanRuleNow = useAutomationStore((state) => state.scanRuleNow);
    const retryFailed = useAutomationStore((state) => state.retryFailed);
    const config = useConfigStore((state) => state.config);
    const projects = useProjectStore((state) => state.projects);
    const alert = useDialogStore((state) => state.alert);
    const confirm = useDialogStore((state) => state.confirm);
    const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set());
    const [drafts, setDrafts] = useState<Record<string, AutomationRuleDraft>>({});

    const projectOptions = useMemo(() => [
        ...projects.map((project) => ({ value: project.id, label: project.name })),
        { value: 'inbox', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
        { value: 'none', label: t('automation.target_none', { defaultValue: 'None (Delete record after export)' }) },
    ], [projects, t]);

    const languageOptions = useMemo(() => (
        LANGUAGE_OPTIONS.map((language) => ({
            value: language,
            label: t(`translation.languages.${language}`),
        }))
    ), [t]);

    const polishPresetOptions = useMemo(() => (
        getPolishPresetOptions(config.polishCustomPresets, t)
    ), [config.polishCustomPresets, t]);

    const exportFormatOptions = useMemo(() => ([
        { value: 'txt', label: 'TXT' },
        { value: 'srt', label: 'SRT' },
        { value: 'vtt', label: 'VTT' },
        { value: 'json', label: 'JSON' },
    ]), []);
    const allExportModeOptions = useMemo(() => ([
        { value: 'original', label: t('export.mode_original', { defaultValue: 'Original' }) },
        { value: 'translation', label: t('export.mode_translation', { defaultValue: 'Translation' }) },
        { value: 'bilingual', label: t('export.mode_bilingual', { defaultValue: 'Bilingual' }) },
    ]), [t]);
    const getExportModeOptions = (autoTranslate: boolean) => (
        autoTranslate
            ? allExportModeOptions
            : allExportModeOptions.filter((option) => option.value === 'original')
    );

    const getRuntimeStatusLabel = (status: AutomationRuntimeStatus | undefined) => {
        switch (status) {
            case 'watching':
                return t('automation.status_watching', { defaultValue: 'Watching' });
            case 'scanning':
                return t('automation.status_scanning', { defaultValue: 'Scanning' });
            case 'error':
                return t('automation.status_error', { defaultValue: 'Error' });
            case 'stopped':
            default:
                return t('automation.status_stopped', { defaultValue: 'Stopped' });
        }
    };

    const describeLastResult = (ruleId: string) => {
        const runtime = runtimeStates[ruleId];
        if (!runtime) {
            return t('automation.last_result_idle', { defaultValue: 'No runs yet' });
        }

        if (runtime.lastResult === 'success') {
            return t('automation.last_result_success', { defaultValue: 'Success' });
        }

        if (runtime.lastResult === 'error') {
            return t('automation.last_result_error', { defaultValue: 'Failed' });
        }

        return t('automation.last_result_idle', { defaultValue: 'No runs yet' });
    };

    const updateDraft = (
        draftKey: string,
        updater: (current: AutomationRuleDraft) => AutomationRuleDraft,
    ) => {
        setDrafts((current) => {
            const existingDraft = current[draftKey];
            if (!existingDraft) {
                return current;
            }

            return {
                ...current,
                [draftKey]: normalizeAutomationRuleDraft(updater(existingDraft)),
            };
        });
    };

    const ensureDraft = (draftKey: string, nextDraft: AutomationRuleDraft) => {
        setDrafts((current) => {
            if (current[draftKey]) {
                return current;
            }

            return {
                ...current,
                [draftKey]: normalizeAutomationRuleDraft(nextDraft),
            };
        });
    };

    const toggleExpanded = (draftKey: string, nextDraft: AutomationRuleDraft) => {
        ensureDraft(draftKey, nextDraft);
        setExpandedRuleIds((current) => {
            const nextExpanded = new Set(current);
            if (nextExpanded.has(draftKey)) {
                nextExpanded.delete(draftKey);
            } else {
                nextExpanded.add(draftKey);
            }
            return nextExpanded;
        });
    };

    const closeDraft = (draftKey: string) => {
        setExpandedRuleIds((current) => {
            const nextExpanded = new Set(current);
            nextExpanded.delete(draftKey);
            return nextExpanded;
        });
        setDrafts((current) => {
            const nextDrafts = { ...current };
            delete nextDrafts[draftKey];
            return nextDrafts;
        });
    };

    const beginCreateRule = () => {
        ensureDraft(NEW_RULE_KEY, createRuleDraft('inbox'));
        setExpandedRuleIds((current) => new Set(current).add(NEW_RULE_KEY));
    };

    const handleBrowseDirectory = async (draftKey: string, field: 'watchDirectory' | 'directory') => {
        const draft = drafts[draftKey];
        if (!draft) {
            return;
        }

        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: field === 'watchDirectory'
                ? draft.watchDirectory || undefined
                : draft.exportConfig.directory || undefined,
        });

        if (!selected || typeof selected !== 'string') {
            return;
        }

        updateDraft(draftKey, (currentDraft) => {
            if (field === 'watchDirectory') {
                return {
                    ...currentDraft,
                    watchDirectory: selected,
                };
            }

            return {
                ...currentDraft,
                exportConfig: {
                    ...currentDraft.exportConfig,
                    directory: selected,
                },
            };
        });
    };

    const handleSave = async (draftKey: string) => {
        const draft = drafts[draftKey];
        if (!draft) {
            return;
        }

        if (!draft.name.trim() || !draft.projectId || !draft.watchDirectory.trim() || !draft.exportConfig.directory.trim()) {
            await alert(
                t('automation.required_fields', {
                    defaultValue: 'Complete the name, project, watch directory, and output directory before saving.',
                }),
                { variant: 'warning' },
            );
            return;
        }

        const liveRule = draft.id ? rules.find((rule: AutomationRule) => rule.id === draft.id) : null;

        try {
            await saveRule({
                ...draft,
                enabled: liveRule?.enabled ?? draft.enabled,
            });
            closeDraft(draftKey);
        } catch (error) {
            await alert(error instanceof Error ? error.message : String(error), { variant: 'error' });
        }
    };

    const handleDelete = async (ruleId: string) => {
        const confirmed = await confirm(
            t('automation.delete_confirm', { defaultValue: 'Delete this automation rule?' }),
            {
                title: t('automation.delete_title', { defaultValue: 'Delete Automation Rule' }),
            },
        );
        if (!confirmed) {
            return;
        }

        await deleteRule(ruleId);
        closeDraft(ruleId);
    };

    const handleToggleRule = async (rule: AutomationRule, enabled: boolean) => {
        try {
            await toggleRuleEnabled(rule.id, enabled);
            updateDraft(rule.id, (currentDraft) => ({
                ...currentDraft,
                enabled,
            }));
        } catch (error) {
            await alert(error instanceof Error ? error.message : String(error), { variant: 'error' });
        }
    };

    const handleScanNow = async (ruleId: string) => {
        try {
            await scanRuleNow(ruleId);
        } catch (error) {
            await alert(error instanceof Error ? error.message : String(error), { variant: 'error' });
        }
    };

    const handleRetryFailed = async (ruleId: string) => {
        try {
            await retryFailed(ruleId);
        } catch (error) {
            await alert(error instanceof Error ? error.message : String(error), { variant: 'error' });
        }
    };

    const renderRuleEditor = (draftKey: string, draft: AutomationRuleDraft) => {
        const exportModeOptions = getExportModeOptions(draft.stageConfig.autoTranslate);

        return (
            <div style={{ padding: '0 24px 24px 56px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div className="settings-section-content">
                    <SettingsItem title={t('automation.rule_name', { defaultValue: 'Rule Name' })} layout="vertical">
                        <input
                            className="settings-input"
                            value={draft.name}
                            onChange={(event) => updateDraft(draftKey, (currentDraft) => ({
                                ...currentDraft,
                                name: event.target.value,
                            }))}
                            placeholder={t('automation.rule_name_placeholder', { defaultValue: 'e.g. Weekly Meeting Inbox' })}
                        />
                    </SettingsItem>

                    <SettingsItem title={t('automation.auto_polish', { defaultValue: 'Auto-Polish' })}>
                        <Switch
                            checked={draft.stageConfig.autoPolish}
                            onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                ...currentDraft,
                                presetId: 'custom',
                                stageConfig: {
                                    ...currentDraft.stageConfig,
                                    autoPolish: value,
                                },
                            }))}
                            aria-label={t('automation.auto_polish', { defaultValue: 'Auto-Polish' })}
                        />
                    </SettingsItem>

                    {draft.stageConfig.autoPolish && (
                        <SettingsItem indent title={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}>
                            <Dropdown
                                value={draft.stageConfig.polishPresetId || 'general'}
                                onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                    ...currentDraft,
                                    presetId: 'custom',
                                    stageConfig: {
                                        ...currentDraft.stageConfig,
                                        polishPresetId: value,
                                    },
                                }))}
                                options={polishPresetOptions}
                                style={{ width: '160px' }}
                                aria-label={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}
                            />
                        </SettingsItem>
                    )}

                    <SettingsItem title={t('automation.auto_translate', { defaultValue: 'Auto-Translate' })}>
                        <Switch
                            checked={draft.stageConfig.autoTranslate}
                            onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                ...currentDraft,
                                presetId: 'custom',
                                stageConfig: {
                                    ...currentDraft.stageConfig,
                                    autoTranslate: value,
                                },
                            }))}
                            aria-label={t('automation.auto_translate', { defaultValue: 'Auto-Translate' })}
                        />
                    </SettingsItem>

                    {draft.stageConfig.autoTranslate && (
                        <SettingsItem indent title={t('translation.target_language', { defaultValue: 'Target Language' })}>
                            <Dropdown
                                value={draft.stageConfig.translationLanguage || 'en'}
                                onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                    ...currentDraft,
                                    presetId: 'custom',
                                    stageConfig: {
                                        ...currentDraft.stageConfig,
                                        translationLanguage: value,
                                    },
                                }))}
                                options={languageOptions}
                                style={{ width: '160px' }}
                                aria-label={t('translation.target_language', { defaultValue: 'Target Language' })}
                            />
                        </SettingsItem>
                    )}

                    <SettingsItem title={t('automation.auto_export', { defaultValue: 'Auto-Export' })}>
                        <Switch
                            checked={draft.stageConfig.exportEnabled}
                            onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                ...currentDraft,
                                presetId: 'custom',
                                stageConfig: {
                                    ...currentDraft.stageConfig,
                                    exportEnabled: value,
                                },
                            }))}
                            aria-label={t('automation.auto_export', { defaultValue: 'Auto-Export' })}
                        />
                    </SettingsItem>

                    {draft.stageConfig.exportEnabled && (
                        <>
                            <SettingsItem indent title={t('projects.export_prefix', { defaultValue: 'Filename Prefix' })}>
                                <input
                                    className="settings-input"
                                    value={draft.exportConfig.prefix || ''}
                                    onChange={(event) => updateDraft(draftKey, (currentDraft) => ({
                                        ...currentDraft,
                                        presetId: 'custom',
                                        exportConfig: {
                                            ...currentDraft.exportConfig,
                                            prefix: event.target.value,
                                        },
                                    }))}
                                    placeholder={t('projects.export_prefix', { defaultValue: 'e.g. [Auto]' })}
                                    style={{ width: '160px' }}
                                />
                            </SettingsItem>

                            <SettingsItem indent title={t('automation.export_format', { defaultValue: 'Export Format' })}>
                                <Dropdown
                                    value={draft.exportConfig.format}
                                    onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                        ...currentDraft,
                                        presetId: 'custom',
                                        exportConfig: {
                                            ...currentDraft.exportConfig,
                                            format: value as ExportFormat,
                                        },
                                    }))}
                                    options={exportFormatOptions}
                                    style={{ width: '160px' }}
                                    aria-label={t('automation.export_format', { defaultValue: 'Export Format' })}
                                />
                            </SettingsItem>

                            <SettingsItem indent title={t('automation.export_mode', { defaultValue: 'Export Mode' })}>
                                <Dropdown
                                    value={draft.exportConfig.mode}
                                    onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                        ...currentDraft,
                                        presetId: 'custom',
                                        exportConfig: {
                                            ...currentDraft.exportConfig,
                                            mode: value as ExportMode,
                                        },
                                    }))}
                                    options={exportModeOptions}
                                    style={{ width: '160px' }}
                                    aria-label={t('automation.export_mode', { defaultValue: 'Export Mode' })}
                                />
                            </SettingsItem>
                        </>
                    )}

                    <SettingsItem title={t('automation.watch_directory', { defaultValue: 'Watch Directory' })} layout="vertical">
                        <div style={{ display: 'flex', gap: '8px', width: '100%' }}>
                            <input
                                className="settings-input"
                                value={draft.watchDirectory}
                                onChange={(event) => updateDraft(draftKey, (currentDraft) => ({
                                    ...currentDraft,
                                    watchDirectory: event.target.value,
                                }))}
                                placeholder={t('automation.watch_directory_placeholder', { defaultValue: 'Choose a folder to monitor...' })}
                                style={{ flex: 1 }}
                            />
                            <button className="btn btn-secondary" onClick={() => { void handleBrowseDirectory(draftKey, 'watchDirectory'); }}>
                                <FolderIcon />
                                <span>{t('settings.browse', { defaultValue: 'Browse' })}</span>
                            </button>
                        </div>
                    </SettingsItem>

                    <SettingsItem title={t('automation.output_directory', { defaultValue: 'Output Directory' })} layout="vertical">
                        <div style={{ display: 'flex', gap: '8px', width: '100%' }}>
                            <input
                                className="settings-input"
                                value={draft.exportConfig.directory}
                                onChange={(event) => updateDraft(draftKey, (currentDraft) => ({
                                    ...currentDraft,
                                    exportConfig: {
                                        ...currentDraft.exportConfig,
                                        directory: event.target.value,
                                    },
                                }))}
                                placeholder={t('automation.output_directory_placeholder', { defaultValue: 'Choose where exports should be written...' })}
                                style={{ flex: 1 }}
                            />
                            <button className="btn btn-secondary" onClick={() => { void handleBrowseDirectory(draftKey, 'directory'); }}>
                                <FolderIcon />
                                <span>{t('settings.browse', { defaultValue: 'Browse' })}</span>
                            </button>
                        </div>
                    </SettingsItem>

                    <SettingsItem title={t('automation.recursive', { defaultValue: 'Watch Subfolders' })}>
                        <Switch
                            checked={draft.recursive}
                            onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                ...currentDraft,
                                recursive: value,
                            }))}
                            aria-label={t('automation.recursive', { defaultValue: 'Watch Subfolders' })}
                        />
                    </SettingsItem>

                    <SettingsItem title={t('automation.target_project', { defaultValue: 'Target Project' })} layout="vertical">
                        <Dropdown
                            value={draft.projectId}
                            onChange={(value) => updateDraft(draftKey, (currentDraft) => ({
                                ...currentDraft,
                                projectId: value,
                            }))}
                            options={projectOptions}
                            style={{ width: '100%' }}
                            aria-label={t('automation.target_project', { defaultValue: 'Target Project' })}
                        />
                    </SettingsItem>
                </div>

                <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
                    <button className="btn" onClick={() => closeDraft(draftKey)}>
                        {t('common.cancel')}
                    </button>
                    <button className="btn btn-primary" onClick={() => { void handleSave(draftKey); }}>
                        {t('common.save')}
                    </button>
                </div>
            </div>
        );
    };

    const renderRuleCard = (
        draftKey: string,
        {
            title,
            projectLabel,
            watchDirectory,
            outputDirectory,
            statusLabel,
            resultLabel,
            failureCount,
            resultMessage,
            enabled,
            canToggle,
            onToggleExpand,
            onToggleEnabled,
            onScanNow,
            onRetryFailed,
            onDelete,
            editor,
        }: {
            title: string;
            projectLabel: string;
            watchDirectory: string;
            outputDirectory: string;
            statusLabel?: string;
            resultLabel?: string;
            failureCount?: number;
            resultMessage?: string;
            enabled: boolean;
            canToggle: boolean;
            onToggleExpand: () => void;
            onToggleEnabled?: (value: boolean) => void;
            onScanNow?: () => void;
            onRetryFailed?: () => void;
            onDelete?: () => void;
            editor?: React.JSX.Element | null;
        },
    ) => {
        const isExpanded = expandedRuleIds.has(draftKey);
        const failureChipTone = (failureCount || 0) > 0 ? 'danger' : 'neutral';
        const resultChipTone = resultLabel === t('automation.last_result_success', { defaultValue: 'Success' })
            ? 'success'
            : resultLabel === t('automation.last_result_error', { defaultValue: 'Failed' })
                ? 'danger'
                : 'neutral';

        return (
            <div
                key={draftKey}
                style={{
                    borderBottom: '1px solid var(--color-border-subtle)',
                    background: enabled ? 'var(--color-bg-primary)' : 'var(--color-bg-secondary-soft)',
                }}
            >
                <div
                    style={{
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: '16px',
                        padding: '16px 24px',
                    }}
                >
                    <button
                        type="button"
                        onClick={onToggleExpand}
                        aria-expanded={isExpanded}
                        style={{
                            flex: 1,
                            minWidth: 0,
                            display: 'flex',
                            alignItems: 'flex-start',
                            gap: '12px',
                            background: 'transparent',
                            border: 'none',
                            padding: 0,
                            textAlign: 'left',
                            cursor: 'pointer',
                            color: 'inherit',
                        }}
                    >
                        <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)', paddingTop: '2px' }}>
                            {isExpanded ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                        </div>

                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', minWidth: 0, flex: 1 }}>
                            <div className="settings-item-title">{title}</div>

                            <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                                <SummaryChip label={projectLabel} tone="neutral" />
                                {statusLabel && <SummaryChip label={statusLabel} tone="neutral" />}
                                {resultLabel && <SummaryChip label={resultLabel} tone={resultChipTone} />}
                                {typeof failureCount === 'number' && (
                                    <SummaryChip
                                        label={t('automation.failure_count', {
                                            defaultValue: '{{count}} failures',
                                            count: failureCount,
                                        })}
                                        tone={failureChipTone}
                                    />
                                )}
                            </div>

                            <div className="settings-item-hint" style={{ wordBreak: 'break-all' }}>
                                {t('automation.watch_directory', { defaultValue: 'Watch Directory' })}: {watchDirectory || t('automation.none', { defaultValue: 'None' })}
                            </div>
                            <div className="settings-item-hint" style={{ wordBreak: 'break-all' }}>
                                {t('automation.output_directory', { defaultValue: 'Output Directory' })}: {outputDirectory || t('automation.none', { defaultValue: 'None' })}
                            </div>
                            {resultMessage && (
                                <div className="settings-item-hint" style={{ wordBreak: 'break-word' }}>
                                    {resultMessage}
                                </div>
                            )}
                        </div>
                    </button>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap', justifyContent: 'flex-end' }}>
                        {canToggle && onToggleEnabled && (
                            <Switch
                                checked={enabled}
                                onChange={onToggleEnabled}
                                aria-label={t('automation.toggle_rule', {
                                    defaultValue: 'Enable {{name}}',
                                    name: title,
                                })}
                            />
                        )}

                        {onScanNow && (
                            <button className="btn btn-secondary" onClick={onScanNow}>
                                <PlayIcon />
                                <span>{t('automation.scan_now', { defaultValue: 'Scan Now' })}</span>
                            </button>
                        )}

                        {onRetryFailed && (
                            <button className="btn btn-secondary" onClick={onRetryFailed} disabled={!failureCount}>
                                <PauseIcon />
                                <span>{t('automation.retry_failed', { defaultValue: 'Retry Failed' })}</span>
                            </button>
                        )}

                        {onDelete && (
                            <button className="btn btn-secondary" onClick={onDelete}>
                                <TrashIcon />
                                <span>{t('common.delete')}</span>
                            </button>
                        )}
                    </div>
                </div>

                {isExpanded && editor}
            </div>
        );
    };

    const newRuleDraft = drafts[NEW_RULE_KEY];

    return (
        <SettingsTabContainer id="settings-panel-automation" ariaLabelledby="settings-tab-automation">
            <SettingsPageHeader
                icon={<AutomationIcon width={28} height={28} />}
                title={t('automation.title', { defaultValue: 'Automation' })}
                description={t('automation.description', {
                    defaultValue: 'Monitor folders and automatically transcribe, polish, translate, and export new media files while Sona is running.',
                })}
            />

            <SettingsSection
                title={t('automation.rules', { defaultValue: 'Rules' })}
                description={t('automation.rules_description', {
                    defaultValue: 'Each rule binds to a project. Language, Polish Preset, and Export Prefix are configured independently.',
                })}
            >
                <div className="settings-section-content">
                    <div className="settings-item-container layout-horizontal">
                        <div className="settings-item-info">
                            <div className="settings-item-title">
                                {projects.length === 0
                                    ? t('automation.no_projects', { defaultValue: 'Create a project first before adding automation rules.' })
                                    : t('automation.rule_count', { defaultValue: '{{count}} rules configured.', count: rules.length })}
                            </div>
                            <div className="settings-item-hint">
                                {t('automation.list_hint', {
                                    defaultValue: 'Configure folder monitoring and define behavior for each automation stage.',
                                })}
                            </div>
                        </div>
                        <div className="settings-item-action">
                            <button className="btn btn-primary" onClick={beginCreateRule} disabled={projects.length === 0}>
                                {t('automation.new_rule', { defaultValue: 'New Rule' })}
                            </button>
                        </div>
                    </div>

                    {newRuleDraft && renderRuleCard(NEW_RULE_KEY, {
                        title: newRuleDraft.name.trim() || t('automation.create_rule', { defaultValue: 'Create Rule' }),
                        projectLabel: projectOptions.find((opt) => opt.value === newRuleDraft.projectId)?.label
                            || t('projects.unknown_project'),
                        watchDirectory: newRuleDraft.watchDirectory,
                        outputDirectory: newRuleDraft.exportConfig.directory,
                        resultLabel: t('automation.draft_badge', { defaultValue: 'Draft' }),
                        enabled: true,
                        canToggle: false,
                        onToggleExpand: () => toggleExpanded(NEW_RULE_KEY, createRuleDraft('inbox')),
                        editor: renderRuleEditor(NEW_RULE_KEY, newRuleDraft),
                    })}

                    {rules.length === 0 && !newRuleDraft ? (
                        <div className="settings-item-container">
                            <div className="settings-item-info">
                                <div className="settings-item-title">
                                    {t('automation.empty_title', { defaultValue: 'No automation rules yet.' })}
                                </div>
                                <div className="settings-item-hint">
                                    {t('automation.empty_hint', {
                                        defaultValue: 'Add a rule to keep a folder watched and push new files through the batch pipeline automatically.',
                                    })}
                                </div>
                            </div>
                        </div>
                    ) : rules.map((rule: AutomationRule) => {
                        const draft = drafts[rule.id];
                        const displayRule = draft || createDraftFromRule(rule);
                        const runtime = runtimeStates[rule.id];

                        return renderRuleCard(rule.id, {
                            title: displayRule.name,
                            projectLabel: projectOptions.find((opt) => opt.value === displayRule.projectId)?.label
                                || t('projects.unknown_project'),
                            watchDirectory: displayRule.watchDirectory,
                            outputDirectory: displayRule.exportConfig.directory,
                            statusLabel: getRuntimeStatusLabel(runtime?.status),
                            resultLabel: describeLastResult(rule.id),
                            failureCount: runtime?.failureCount || 0,
                            resultMessage: runtime?.lastResultMessage,
                            enabled: rule.enabled,
                            canToggle: true,
                            onToggleExpand: () => toggleExpanded(rule.id, createDraftFromRule(rule)),
                            onToggleEnabled: (value) => { void handleToggleRule(rule, value); },
                            onScanNow: () => { void handleScanNow(rule.id); },
                            onRetryFailed: () => { void handleRetryFailed(rule.id); },
                            onDelete: () => { void handleDelete(rule.id); },
                            editor: renderRuleEditor(rule.id, displayRule),
                        });
                    })}
                </div>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
