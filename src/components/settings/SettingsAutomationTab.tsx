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
import {
    getAutomationPresetDefinition,
    AUTOMATION_PRESETS,
    applyAutomationPreset,
    DEFAULT_AUTOMATION_PRESET_ID,
    isBuiltInAutomationPresetId,
} from '../../utils/automationPresets';
import { getPolishPresetLabel } from '../../utils/polishPresets';
import { SettingsItem, SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import type {
    AutomationRule,
    AutomationRuntimeStatus,
    BuiltInAutomationPresetId,
} from '../../types/automation';
import type { ExportFormat, ExportMode } from '../../utils/exportFormats';

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

    if (normalizedMode === draft.exportConfig.mode) {
        return draft;
    }

    return {
        ...draft,
        exportConfig: {
            ...draft.exportConfig,
            mode: normalizedMode,
        },
    };
}

function createRuleDraft(projectId: string): AutomationRuleDraft {
    const presetConfig = applyAutomationPreset(DEFAULT_AUTOMATION_PRESET_ID, {
        stageConfig: {
            autoPolish: false,
            autoTranslate: false,
            exportEnabled: true,
        },
        exportConfig: {
            directory: '',
            format: 'txt',
            mode: 'original',
        },
    });

    return normalizeAutomationRuleDraft({
        name: '',
        projectId,
        presetId: DEFAULT_AUTOMATION_PRESET_ID,
        watchDirectory: '',
        recursive: false,
        enabled: false,
        stageConfig: presetConfig.stageConfig,
        exportConfig: {
            ...presetConfig.exportConfig,
            directory: '',
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
        stageConfig: { ...rule.stageConfig },
        exportConfig: { ...rule.exportConfig },
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
    const activeProjectId = useProjectStore((state) => state.activeProjectId);
    const alert = useDialogStore((state) => state.alert);
    const confirm = useDialogStore((state) => state.confirm);
    const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set());
    const [drafts, setDrafts] = useState<Record<string, AutomationRuleDraft>>({});
    const [pendingPresetIds, setPendingPresetIds] = useState<Record<string, BuiltInAutomationPresetId | undefined>>({});

    const projectOptions = useMemo(() => (
        projects.map((project) => ({ value: project.id, label: project.name }))
    ), [projects]);
    const presetOptions = useMemo(() => (
        AUTOMATION_PRESETS.map((preset) => ({
            value: preset.id,
            label: t(preset.labelKey, { defaultValue: preset.defaultLabel }),
        }))
    ), [t]);
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

    const getPresetLabel = (presetId: AutomationRule['presetId']) => {
        const preset = getAutomationPresetDefinition(presetId);
        return t(preset.labelKey, { defaultValue: preset.defaultLabel });
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

    const clearPendingPreset = (draftKey: string) => {
        setPendingPresetIds((current) => {
            if (!(draftKey in current)) {
                return current;
            }

            const nextPending = { ...current };
            delete nextPending[draftKey];
            return nextPending;
        });
    };

    const getPendingBuiltInPresetId = (
        draftKey: string,
        draft: AutomationRuleDraft,
    ): BuiltInAutomationPresetId | undefined => {
        const pendingPresetId = pendingPresetIds[draftKey];
        if (pendingPresetId) {
            return pendingPresetId;
        }

        return isBuiltInAutomationPresetId(draft.presetId) ? draft.presetId : undefined;
    };

    const updateTemplateControlledDraft = (
        draftKey: string,
        updater: (current: AutomationRuleDraft) => AutomationRuleDraft,
    ) => {
        clearPendingPreset(draftKey);
        updateDraft(draftKey, (currentDraft) => ({
            ...updater(currentDraft),
            presetId: 'custom',
        }));
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
        clearPendingPreset(draftKey);
        setDrafts((current) => {
            const nextDrafts = { ...current };
            delete nextDrafts[draftKey];
            return nextDrafts;
        });
    };

    const beginCreateRule = () => {
        const fallbackProjectId = activeProjectId || projects[0]?.id || '';
        ensureDraft(NEW_RULE_KEY, createRuleDraft(fallbackProjectId));
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

    const handleTemplateSelectionChange = (draftKey: string, draft: AutomationRuleDraft, value: string) => {
        if (!isBuiltInAutomationPresetId(value)) {
            clearPendingPreset(draftKey);
            return;
        }

        if (draft.presetId !== 'custom' && value === draft.presetId) {
            clearPendingPreset(draftKey);
            return;
        }

        setPendingPresetIds((current) => ({
            ...current,
            [draftKey]: value,
        }));
    };

    const handleApplyTemplate = (draftKey: string) => {
        const draft = drafts[draftKey];
        if (!draft) {
            return;
        }

        const targetPresetId = getPendingBuiltInPresetId(draftKey, draft);
        if (!targetPresetId) {
            return;
        }

        const applied = applyAutomationPreset(targetPresetId, draft);
        clearPendingPreset(draftKey);
        updateDraft(draftKey, (currentDraft) => ({
            ...currentDraft,
            presetId: targetPresetId,
            stageConfig: applied.stageConfig,
            exportConfig: {
                ...currentDraft.exportConfig,
                format: applied.exportConfig.format,
                mode: applied.exportConfig.mode,
            },
        }));
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

        const liveRule = draft.id ? rules.find((rule) => rule.id === draft.id) : null;

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

    const renderTemplateControlRow = (
        label: string,
        control: React.JSX.Element,
        key: string,
        bordered = true,
    ) => (
        <div
            key={key}
            style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                gap: '12px',
                padding: '12px 0',
                borderTop: bordered ? '1px solid var(--color-border-subtle)' : 'none',
            }}
        >
            <div style={{ minWidth: 0 }}>
                <div className="settings-item-title" style={{ fontSize: '0.92rem' }}>{label}</div>
                <div className="settings-item-hint">
                    {t('automation.template_controlled', { defaultValue: 'Template-controlled' })}
                </div>
            </div>
            <div style={{ flexShrink: 0, minWidth: '160px', display: 'flex', justifyContent: 'flex-end' }}>
                {control}
            </div>
        </div>
    );

    const renderRuleEditor = (draftKey: string, draft: AutomationRuleDraft) => {
        const selectedProject = projects.find((project) => project.id === draft.projectId) || null;
        const currentPreset = getAutomationPresetDefinition(draft.presetId);
        const pendingPresetId = pendingPresetIds[draftKey];
        const pendingPreset = pendingPresetId ? getAutomationPresetDefinition(pendingPresetId) : null;
        const dropdownValue = draft.presetId === 'custom'
            ? 'custom'
            : (pendingPresetId || draft.presetId);
        const templateOptions = draft.presetId === 'custom'
            ? [
                {
                    value: 'custom',
                    label: t('automation.presets.custom', { defaultValue: 'Custom' }),
                },
                ...presetOptions,
            ]
            : presetOptions;
        const templateApplyTargetId = getPendingBuiltInPresetId(draftKey, draft);
        const canApplyTemplate = Boolean(templateApplyTargetId);
        const templateApplyTargetLabel = pendingPreset
            ? t(pendingPreset.labelKey, { defaultValue: pendingPreset.defaultLabel })
            : null;
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

                    <SettingsItem
                        title={t('automation.template', { defaultValue: 'Template' })}
                        hint={t('automation.template_section_description', {
                            defaultValue: 'Templates shape the automation flow. Projects still control translation language, polish preset, and export prefix.',
                        })}
                        layout="vertical"
                    >
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', width: '100%' }}>
                            <div
                                style={{
                                    padding: '14px 16px',
                                    borderRadius: '14px',
                                    background: 'var(--color-bg-secondary)',
                                    display: 'flex',
                                    flexDirection: 'column',
                                    gap: '6px',
                                }}
                            >
                                <div className="settings-item-hint">
                                    {t('automation.current_template', { defaultValue: 'Current Template' })}
                                </div>
                                <div
                                    style={{
                                        display: 'flex',
                                        alignItems: 'center',
                                    }}
                                >
                                    <div className="settings-item-title">
                                        {t(currentPreset.labelKey, { defaultValue: currentPreset.defaultLabel })}
                                    </div>
                                </div>
                                <div className="settings-item-hint">
                                    {t(currentPreset.descriptionKey, {
                                        defaultValue: currentPreset.defaultDescription,
                                    })}
                                </div>
                            </div>

                            <div style={{ display: 'flex', gap: '8px', width: '100%', alignItems: 'stretch' }}>
                                <Dropdown
                                    value={dropdownValue}
                                    onChange={(value) => handleTemplateSelectionChange(draftKey, draft, value)}
                                    options={templateOptions}
                                    style={{ flex: 1 }}
                                    aria-label={t('automation.template', { defaultValue: 'Template' })}
                                />
                                <button
                                    className="btn btn-secondary"
                                    onClick={() => handleApplyTemplate(draftKey)}
                                    type="button"
                                    disabled={!canApplyTemplate}
                                >
                                    {t('automation.apply_template', { defaultValue: 'Apply Template' })}
                                </button>
                            </div>

                            {templateApplyTargetLabel && (
                                <div className="settings-item-hint">
                                    {t('automation.template_pending_apply', {
                                        defaultValue: 'Selected to apply: {{template}}',
                                        template: templateApplyTargetLabel,
                                    })}
                                </div>
                            )}

                            <div
                                style={{
                                    padding: '0 16px',
                                    borderRadius: '14px',
                                    border: '1px solid var(--color-border-subtle)',
                                    background: 'var(--color-bg-primary)',
                                }}
                            >
                                {renderTemplateControlRow(
                                    t('automation.auto_polish', { defaultValue: 'Auto-Polish' }),
                                    (
                                        <Switch
                                            checked={draft.stageConfig.autoPolish}
                                            onChange={(value) => updateTemplateControlledDraft(draftKey, (currentDraft) => ({
                                                ...currentDraft,
                                                stageConfig: {
                                                    ...currentDraft.stageConfig,
                                                    autoPolish: value,
                                                },
                                            }))}
                                            aria-label={t('automation.auto_polish', { defaultValue: 'Auto-Polish' })}
                                        />
                                    ),
                                    'auto-polish',
                                    false,
                                )}
                                {renderTemplateControlRow(
                                    t('automation.auto_translate', { defaultValue: 'Auto-Translate' }),
                                    (
                                        <Switch
                                            checked={draft.stageConfig.autoTranslate}
                                            onChange={(value) => updateTemplateControlledDraft(draftKey, (currentDraft) => ({
                                                ...currentDraft,
                                                stageConfig: {
                                                    ...currentDraft.stageConfig,
                                                    autoTranslate: value,
                                                },
                                            }))}
                                            aria-label={t('automation.auto_translate', { defaultValue: 'Auto-Translate' })}
                                        />
                                    ),
                                    'auto-translate',
                                )}
                                {renderTemplateControlRow(
                                    t('automation.auto_export', { defaultValue: 'Auto-Export' }),
                                    (
                                        <Switch
                                            checked={draft.stageConfig.exportEnabled}
                                            onChange={(value) => updateTemplateControlledDraft(draftKey, (currentDraft) => ({
                                                ...currentDraft,
                                                stageConfig: {
                                                    ...currentDraft.stageConfig,
                                                    exportEnabled: value,
                                                },
                                            }))}
                                            aria-label={t('automation.auto_export', { defaultValue: 'Auto-Export' })}
                                        />
                                    ),
                                    'auto-export',
                                )}
                                {renderTemplateControlRow(
                                    t('automation.export_format', { defaultValue: 'Export Format' }),
                                    (
                                        <Dropdown
                                            value={draft.exportConfig.format}
                                            onChange={(value) => updateTemplateControlledDraft(draftKey, (currentDraft) => ({
                                                ...currentDraft,
                                                exportConfig: {
                                                    ...currentDraft.exportConfig,
                                                    format: value as ExportFormat,
                                                },
                                            }))}
                                            options={exportFormatOptions}
                                            style={{ width: '160px' }}
                                            aria-label={t('automation.export_format', { defaultValue: 'Export Format' })}
                                        />
                                    ),
                                    'export-format',
                                )}
                                {renderTemplateControlRow(
                                    t('automation.export_mode', { defaultValue: 'Export Mode' }),
                                    (
                                        <Dropdown
                                            value={draft.exportConfig.mode}
                                            onChange={(value) => updateTemplateControlledDraft(draftKey, (currentDraft) => ({
                                                ...currentDraft,
                                                exportConfig: {
                                                    ...currentDraft.exportConfig,
                                                    mode: value as ExportMode,
                                                },
                                            }))}
                                            options={exportModeOptions}
                                            style={{ width: '160px' }}
                                            aria-label={t('automation.export_mode', { defaultValue: 'Export Mode' })}
                                        />
                                    ),
                                    'export-mode',
                                )}
                            </div>

                            <div className="settings-item-hint">
                                {t('automation.template_apply_notice', {
                                    defaultValue: 'Applying a template updates only the template-controlled fields above.',
                                })}
                            </div>
                        </div>
                    </SettingsItem>

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

                    <SettingsItem
                        title={t('automation.inherited_defaults', { defaultValue: 'Inherited Project Defaults' })}
                        hint={t('automation.inherited_defaults_description', {
                            defaultValue: 'These values come from the selected project and are applied at queue time.',
                        })}
                        layout="vertical"
                    >
                        <div
                            style={{
                                width: '100%',
                                padding: '12px 14px',
                                borderRadius: '12px',
                                background: 'var(--color-bg-secondary)',
                                display: 'flex',
                                flexDirection: 'column',
                                gap: '6px',
                            }}
                        >
                            <div className="settings-item-hint">
                                {t('automation.inherited_defaults_hint', {
                                    defaultValue: 'Templates do not change these project-level defaults.',
                                })}
                            </div>
                            <div className="settings-item-hint">
                                {t('projects.translation_language')}:
                                {' '}
                                {selectedProject?.defaults.translationLanguage || config.translationLanguage || 'zh'}
                            </div>
                            <div className="settings-item-hint">
                                {t('projects.polish_preset')}:
                                {' '}
                                {getPolishPresetLabel(selectedProject?.defaults.polishPresetId, config.polishCustomPresets, t)}
                            </div>
                            <div className="settings-item-hint">
                                {t('projects.export_prefix')}:
                                {' '}
                                {selectedProject?.defaults.exportFileNamePrefix || t('automation.none', { defaultValue: 'None' })}
                            </div>
                        </div>
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
            presetLabel,
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
            presetLabel: string;
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
                                <SummaryChip label={presetLabel} tone="neutral" />
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
                    defaultValue: 'Each rule binds to a project and inherits its translation language, polish preset, and export prefix.',
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
                                    defaultValue: 'Templates shape the pipeline. Projects keep long-lived defaults like translation language and export prefix.',
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
                        projectLabel: projects.find((project) => project.id === newRuleDraft.projectId)?.name
                            || t('projects.unknown_project'),
                        presetLabel: getPresetLabel(newRuleDraft.presetId),
                        watchDirectory: newRuleDraft.watchDirectory,
                        outputDirectory: newRuleDraft.exportConfig.directory,
                        resultLabel: t('automation.draft_badge', { defaultValue: 'Draft' }),
                        enabled: true,
                        canToggle: false,
                        onToggleExpand: () => toggleExpanded(NEW_RULE_KEY, createRuleDraft(activeProjectId || projects[0]?.id || '')),
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
                    ) : rules.map((rule) => {
                        const draft = drafts[rule.id];
                        const displayRule = draft || createDraftFromRule(rule);
                        const runtime = runtimeStates[rule.id];
                        const project = projects.find((projectItem) => projectItem.id === displayRule.projectId) || null;

                        return renderRuleCard(rule.id, {
                            title: displayRule.name,
                            projectLabel: project?.name || t('projects.unknown_project'),
                            presetLabel: getPresetLabel(displayRule.presetId),
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
