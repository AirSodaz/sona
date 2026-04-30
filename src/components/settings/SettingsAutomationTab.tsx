import React, { useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { AutomationIcon } from '../Icons';
import { useAutomationStore } from '../../stores/automationStore';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useDialogStore } from '../../stores/dialogStore';
import { getPolishPresetOptions } from '../../utils/polishPresets';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import type {
    AutomationRule,
    AutomationRuntimeStatus,
} from '../../types/automation';
import { AutomationRuleCard } from './automation/AutomationRuleCard';
import { AutomationRuleEditor } from './automation/AutomationRuleEditor';
import {
    createDraftFromRule,
    createRuleDraft,
    LANGUAGE_OPTIONS,
    NEW_RULE_KEY,
    normalizeAutomationRuleDraft,
    setDraftField,
    setExportConfigField,
    type AutomationDraftUpdate,
    type AutomationRuleDraft,
} from './automation/automationRuleDraft';

type BrowseField = 'watchDirectory' | 'directory';
type SelectOption = {
    value: string;
    label: string;
};

export function SettingsAutomationTab(): React.JSX.Element {
    const { t } = useTranslation();
    const rules = useAutomationStore((state) => state.rules);
    const runtimeStates = useAutomationStore((state) => state.runtimeStates);
    const saveRule = useAutomationStore((state) => state.saveRule);
    const deleteRule = useAutomationStore((state) => state.deleteRule);
    const toggleRuleEnabled = useAutomationStore((state) => state.toggleRuleEnabled);
    const scanRuleNow = useAutomationStore((state) => state.scanRuleNow);
    const retryFailed = useAutomationStore((state) => state.retryFailed);
    const queueItems = useBatchQueueStore((state) => state.queueItems);
    const config = useConfigStore((state) => state.config);
    const projects = useProjectStore((state) => state.projects);
    const alert = useDialogStore((state) => state.alert);
    const confirm = useDialogStore((state) => state.confirm);
    const showError = useDialogStore((state) => state.showError);
    const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set());
    const [drafts, setDrafts] = useState<Record<string, AutomationRuleDraft>>({});

    const queueSummaryByRuleId = useMemo(() => {
        const summary = new Map<string, { pending: number; processing: number }>();

        queueItems.forEach((item) => {
            if (item.origin !== 'automation' || !item.automationRuleId) {
                return;
            }

            const counts = summary.get(item.automationRuleId) || { pending: 0, processing: 0 };
            if (item.status === 'pending') {
                counts.pending += 1;
            } else if (item.status === 'processing') {
                counts.processing += 1;
            }
            summary.set(item.automationRuleId, counts);
        });

        return summary;
    }, [queueItems]);

    const projectOptions = useMemo<SelectOption[]>(() => [
        ...projects.map((project) => ({ value: project.id, label: project.name })),
        { value: 'inbox', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
        { value: 'none', label: t('automation.target_none', { defaultValue: 'None (Delete record after export)' }) },
    ], [projects, t]);

    const languageOptions = useMemo<SelectOption[]>(() => (
        LANGUAGE_OPTIONS.map((language) => ({
            value: language,
            label: t(`translation.languages.${language}`),
        }))
    ), [t]);

    const polishPresetOptions = useMemo<SelectOption[]>(() => (
        getPolishPresetOptions(config.polishCustomPresets, t)
    ), [config.polishCustomPresets, t]);

    const exportFormatOptions = useMemo<SelectOption[]>(() => ([
        { value: 'txt', label: 'TXT' },
        { value: 'srt', label: 'SRT' },
        { value: 'vtt', label: 'VTT' },
        { value: 'json', label: 'JSON' },
    ]), []);

    const allExportModeOptions = useMemo<SelectOption[]>(() => ([
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

    const getRuntimeBlockedReasonLabel = (reason: string | undefined) => {
        switch (reason) {
            case 'already_processed':
                return t('automation.blocked_reason_already_processed', { defaultValue: 'already processed' });
            case 'already_pending':
                return t('automation.blocked_reason_already_pending', { defaultValue: 'already queued' });
            case 'recovery_blocked':
                return t('automation.blocked_reason_recovery_blocked', { defaultValue: 'blocked by recovery' });
            case 'project_missing':
                return t('automation.blocked_reason_project_missing', { defaultValue: 'target project is missing' });
            case 'retry_source_missing':
                return t('automation.blocked_reason_retry_source_missing', { defaultValue: 'retry source is unavailable' });
            default:
                return null;
        }
    };

    const describeLatestBlockedHint = (ruleId: string) => {
        const runtime = runtimeStates[ruleId];
        if (!runtime?.lastBlockedAt || !runtime.lastBlockedReason) {
            return null;
        }

        if (runtime.lastQueuedAt && runtime.lastBlockedAt <= runtime.lastQueuedAt) {
            return null;
        }

        const reasonLabel = getRuntimeBlockedReasonLabel(runtime.lastBlockedReason);
        if (!reasonLabel) {
            return null;
        }

        const fileName = runtime.lastBlockedFilePath?.split(/[/\\]/).pop()
            || t('automation.blocked_unknown_file', { defaultValue: 'latest file' });

        return t('automation.latest_blocked_hint', {
            defaultValue: 'Skipped {{fileName}}: {{reason}}',
            fileName,
            reason: reasonLabel,
        });
    };

    const updateDraft = (draftKey: string, updater: AutomationDraftUpdate) => {
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

    const handleBrowseDirectory = async (draftKey: string, field: BrowseField) => {
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

        updateDraft(
            draftKey,
            field === 'watchDirectory'
                ? setDraftField('watchDirectory', selected)
                : setExportConfigField('directory', selected, false),
        );
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
            await showError({
                code: 'automation.save_failed',
                messageKey: 'errors.automation.save_failed',
                cause: error,
            });
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
            updateDraft(rule.id, setDraftField('enabled', enabled));
        } catch (error) {
            await showError({
                code: 'automation.toggle_failed',
                messageKey: 'errors.automation.toggle_failed',
                cause: error,
            });
        }
    };

    const handleScanNow = async (ruleId: string) => {
        try {
            await scanRuleNow(ruleId);
        } catch (error) {
            await showError({
                code: 'automation.scan_failed',
                messageKey: 'errors.automation.scan_failed',
                cause: error,
            });
        }
    };

    const handleRetryFailed = async (ruleId: string) => {
        try {
            await retryFailed(ruleId);
        } catch (error) {
            await showError({
                code: 'automation.retry_failed',
                messageKey: 'errors.automation.retry_failed',
                cause: error,
            });
        }
    };

    const createEditor = (draftKey: string, draft: AutomationRuleDraft) => (
        <AutomationRuleEditor
            draft={draft}
            exportFormatOptions={exportFormatOptions}
            exportModeOptions={getExportModeOptions(draft.stageConfig.autoTranslate)}
            languageOptions={languageOptions}
            onBrowseDirectory={(field) => { void handleBrowseDirectory(draftKey, field); }}
            onCancel={() => closeDraft(draftKey)}
            onSave={() => { void handleSave(draftKey); }}
            onUpdateDraft={(updater) => updateDraft(draftKey, updater)}
            polishPresetOptions={polishPresetOptions}
            projectOptions={projectOptions}
        />
    );

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

                    {newRuleDraft && (
                        <AutomationRuleCard
                            title={newRuleDraft.name.trim() || t('automation.create_rule', { defaultValue: 'Create Rule' })}
                            projectLabel={projectOptions.find((opt) => opt.value === newRuleDraft.projectId)?.label
                                || t('projects.unknown_project')}
                            watchDirectory={newRuleDraft.watchDirectory}
                            outputDirectory={newRuleDraft.exportConfig.directory}
                            resultLabel={t('automation.draft_badge', { defaultValue: 'Draft' })}
                            enabled={true}
                            canToggle={false}
                            isExpanded={expandedRuleIds.has(NEW_RULE_KEY)}
                            onToggleExpand={() => toggleExpanded(NEW_RULE_KEY, createRuleDraft('inbox'))}
                            editor={createEditor(NEW_RULE_KEY, newRuleDraft)}
                        />
                    )}

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
                        const queueSummary = queueSummaryByRuleId.get(rule.id);

                        return (
                            <AutomationRuleCard
                                key={rule.id}
                                title={displayRule.name}
                                projectLabel={projectOptions.find((opt) => opt.value === displayRule.projectId)?.label
                                    || t('projects.unknown_project')}
                                watchDirectory={displayRule.watchDirectory}
                                outputDirectory={displayRule.exportConfig.directory}
                                statusLabel={getRuntimeStatusLabel(runtime?.status)}
                                resultLabel={describeLastResult(rule.id)}
                                failureCount={runtime?.failureCount || 0}
                                pendingCount={queueSummary?.pending || 0}
                                processingCount={queueSummary?.processing || 0}
                                resultMessage={runtime?.lastResultMessage}
                                blockedHint={describeLatestBlockedHint(rule.id)}
                                enabled={rule.enabled}
                                canToggle={true}
                                isExpanded={expandedRuleIds.has(rule.id)}
                                onToggleExpand={() => toggleExpanded(rule.id, createDraftFromRule(rule))}
                                onToggleEnabled={(value) => { void handleToggleRule(rule, value); }}
                                onScanNow={() => { void handleScanNow(rule.id); }}
                                onRetryFailed={() => { void handleRetryFailed(rule.id); }}
                                onDelete={() => { void handleDelete(rule.id); }}
                                editor={createEditor(rule.id, displayRule)}
                            />
                        );
                    })}
                </div>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
