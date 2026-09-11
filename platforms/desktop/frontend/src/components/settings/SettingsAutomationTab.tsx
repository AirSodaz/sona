import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderSync, Activity, Sparkles } from 'lucide-react';
import { AutomationIcon } from '../Icons';
import { useAutomationStore } from '../../stores/automationStore';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useProjectStore } from '../../stores/projectStore';
import { useDialogStore } from '../../stores/dialogStore';
import { openDialog } from '../../services/tauri/platform/dialog';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import './SettingsAutomation.css';
import type {
    AutomationRule,
    AutomationRuntimeStatus,
} from '../../types/automation';
import {
    AutomationRuleCard,
    AutomationRuleEditor,
    createDraftFromRule,
    createRuleDraft,
    NEW_RULE_KEY,
    setExportConfigField,
    type AutomationDraftUpdate,
    type AutomationRuleDraft,
} from './automation';

type BrowseField = 'watchDirectory' | 'directory';
type SelectOption = {
    value: string;
    label: string;
};

export function SettingsAutomationTab(): React.JSX.Element {
    const { t } = useTranslation();
    const rules = useAutomationStore((state) => state.rules);
    const runtimeStates = useAutomationStore((state) => state.runtimeStates);
    const processedEntries = useAutomationStore((state) => state.processedEntries);
    const saveRule = useAutomationStore((state) => state.saveRule);
    const deleteRule = useAutomationStore((state) => state.deleteRule);
    const toggleRuleEnabled = useAutomationStore((state) => state.toggleRuleEnabled);
    const scanRuleNow = useAutomationStore((state) => state.scanRuleNow);
    const retryFailed = useAutomationStore((state) => state.retryFailed);
    const queueItems = useBatchQueueStore((state) => state.queueItems);
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
        { value: 'inbox', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
        ...projects.map((project) => ({ value: project.id, label: project.name })),
    ], [projects, t]);

    const visibleRules = useMemo(
        () => rules.filter((rule) => (rule.kind ?? 'file') === 'file'),
        [rules],
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

        const fileName = runtime.lastBlockedFilePath
            ? runtime.lastBlockedFilePath.split(/[\\/]/).filter(Boolean).pop()
            : null;
        if (fileName) {
            return `${fileName}: ${reasonLabel}`;
        }

        return reasonLabel;
    };

    const toggleExpanded = (ruleId: string, defaultDraft?: AutomationRuleDraft) => {
        setExpandedRuleIds((current) => {
            const next = new Set(current);
            if (next.has(ruleId)) {
                next.delete(ruleId);
            } else {
                next.add(ruleId);
                if (defaultDraft && !drafts[ruleId]) {
                    setDrafts((prev) => ({ ...prev, [ruleId]: defaultDraft }));
                }
            }
            return next;
        });
    };

    const beginCreateRule = () => {
        setDrafts((current) => ({
            ...current,
            [NEW_RULE_KEY]: createRuleDraft('inbox', 'file'),
        }));
        setExpandedRuleIds((current) => new Set(current).add(NEW_RULE_KEY));
    };

    const closeDraft = (draftKey: string) => {
        setDrafts((current) => {
            const next = { ...current };
            delete next[draftKey];
            return next;
        });
        setExpandedRuleIds((current) => {
            const next = new Set(current);
            next.delete(draftKey);
            return next;
        });
    };

    const updateDraft = (draftKey: string, updater: AutomationDraftUpdate) => {
        setDrafts((current) => {
            const target = current[draftKey];
            if (!target) {
                return current;
            }
            return {
                ...current,
                [draftKey]: updater(target),
            };
        });
    };

    const handleBrowseDirectory = async (draftKey: string, field: BrowseField) => {
        const selected = await openDialog({
            directory: true,
            multiple: false,
        });
        if (!selected || typeof selected !== 'string') {
            return;
        }
        updateDraft(
            draftKey,
            field === 'directory'
                ? setExportConfigField('directory', selected)
                : (draft) => ({
                    ...draft,
                    [field]: selected,
                }),
        );
    };

    const handleSave = async (draftKey: string) => {
        const draft = drafts[draftKey];
        if (!draft) {
            return;
        }

        if (!draft.name.trim() || !draft.watchDirectory.trim()) {
            await alert(
                t('automation.required_fields', {
                    defaultValue: 'Complete the rule name and watch directory before saving.',
                }),
                { variant: 'warning' },
            );
            return;
        }

        if (!draft.saveHistory && !draft.exportConfig.directory.trim()) {
            await alert(
                t('automation.export_directory_required', {
                    defaultValue: 'Please specify an output directory when Save to History is disabled.',
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
        const targetRule = rules.find((rule) => rule.id === ruleId);
        const confirmed = await confirm(
            t('automation.delete_confirm', {
                defaultValue: 'Delete automation rule "{{name}}"?',
                name: targetRule?.name || ruleId,
            }),
            {
                title: t('automation.delete_rule', { defaultValue: 'Delete Automation Rule' }),
                variant: 'error',
            },
        );

        if (!confirmed) {
            return;
        }

        try {
            await deleteRule(ruleId);
            closeDraft(ruleId);
        } catch (error) {
            await showError({
                code: 'automation.delete_failed',
                messageKey: 'errors.automation.delete_failed',
                cause: error,
            });
        }
    };

    const handleToggleEnabled = async (ruleId: string, enabled: boolean) => {
        try {
            await toggleRuleEnabled(ruleId, enabled);
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

    const createEditor = (key: string, draft: AutomationRuleDraft) => (
        <AutomationRuleEditor
            draft={draft}
            onBrowseDirectory={(field) => { void handleBrowseDirectory(key, field); }}
            onCancel={() => closeDraft(key)}
            onSave={() => { void handleSave(key); }}
            onUpdateDraft={(updater) => updateDraft(key, updater)}
            projectOptions={projectOptions}
        />
    );

    const newRuleDraft = drafts[NEW_RULE_KEY];
    const activeWatchingCount = rules.filter((r) => r.enabled && (r.kind ?? 'file') === 'file').length;
    const totalActiveRulesCount = rules.filter((r) => r.enabled).length;

    return (
        <SettingsTabContainer id="settings-panel-automation" ariaLabelledby="settings-tab-automation">
            <SettingsPageHeader
                icon={<AutomationIcon width={28} height={28} />}
                title={t('automation.title', { defaultValue: 'Automation' })}
                description={t('automation.description', {
                    defaultValue: 'Watch local folders for new audio files, automatically ingest them into target projects, and run their deterministic pipelines.',
                })}
            />

            {/* Dashboard Overview Metrics */}
            <div className="automation-stats-banner">
                <div className="automation-stat-item">
                    <div className="automation-stat-icon-box icon-active">
                        <FolderSync size={18} />
                    </div>
                    <div className="automation-stat-copy">
                        <span className="automation-stat-value">{activeWatchingCount}</span>
                        <span className="automation-stat-label">{t('automation.stat_watchers', { defaultValue: 'Active Folder Watchers' })}</span>
                    </div>
                </div>

                <div className="automation-stat-item">
                    <div className="automation-stat-icon-box icon-rules">
                        <Sparkles size={18} />
                    </div>
                    <div className="automation-stat-copy">
                        <span className="automation-stat-value">{totalActiveRulesCount} / {rules.length}</span>
                        <span className="automation-stat-label">{t('automation.stat_rules', { defaultValue: 'Enabled Rules' })}</span>
                    </div>
                </div>

                <div className="automation-stat-item">
                    <div className="automation-stat-icon-box icon-history">
                        <Activity size={18} />
                    </div>
                    <div className="automation-stat-copy">
                        <span className="automation-stat-value">{processedEntries.length}</span>
                        <span className="automation-stat-label">{t('automation.stat_processed', { defaultValue: 'Processed Files' })}</span>
                    </div>
                </div>
            </div>

            <SettingsSection
                title={t('automation.file_rules', { defaultValue: 'Folder Watchers' })}
                description={t('automation.file_rules_description', {
                    defaultValue: 'Auto-ingest dropped audio from watched folders into assigned projects.',
                })}
            >
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('automation.rule_count', { defaultValue: '{{count}} rules configured.', count: visibleRules.length })}
                        </div>
                        <div className="settings-item-hint">
                            {t('automation.file_list_hint', {
                                defaultValue: 'Each watched folder forwards new audio files directly to its target project pipeline.',
                            })}
                        </div>
                    </div>
                    <div className="settings-item-action">
                        <button type="button" className="btn btn-primary" onClick={beginCreateRule}>
                            {t('automation.new_rule', { defaultValue: 'New Rule' })}
                        </button>
                    </div>
                </div>

                {newRuleDraft && (
                    <AutomationRuleCard
                        title={newRuleDraft.name.trim() || t('automation.create_rule', { defaultValue: 'Create Rule' })}
                        typeLabel={t('automation.file_rule', { defaultValue: 'Folder' })}
                        projectLabel={projectOptions.find((o) => o.value === (newRuleDraft.projectId || 'inbox'))?.label || t('projects.inbox', { defaultValue: 'Inbox' })}
                        watchDirectory={newRuleDraft.watchDirectory}
                        enabled={newRuleDraft.enabled}
                        canToggle={false}
                        statusLabel={t('automation.status_draft', { defaultValue: 'Draft' })}
                        isExpanded={expandedRuleIds.has(NEW_RULE_KEY)}
                        onToggleExpand={() => toggleExpanded(NEW_RULE_KEY, createRuleDraft('inbox', 'file'))}
                        editor={createEditor(NEW_RULE_KEY, newRuleDraft)}
                    />
                )}

                {visibleRules.length === 0 && !newRuleDraft ? (
                    <div className="settings-item-container">
                        <div className="settings-item-info">
                            <div className="settings-item-title">
                                {t('automation.empty_title', { defaultValue: 'No folder rules yet.' })}
                            </div>
                            <div className="settings-item-hint">
                                {t('automation.empty_hint', {
                                    defaultValue: 'Set up a watched directory to automatically transcribe and process files.',
                                })}
                            </div>
                        </div>
                    </div>
                ) : (
                    visibleRules.map((rule) => {
                        const draft = drafts[rule.id];
                        const displayRule = draft ?? rule;
                        const runtime = runtimeStates[rule.id];
                        const queueSummary = queueSummaryByRuleId.get(rule.id);
                        const targetProject = projects.find((p) => p.id === (displayRule.projectId || displayRule.tagIds?.[0]));
                        const projectLabel = displayRule.saveHistory
                            ? (targetProject ? targetProject.name : t('projects.inbox', { defaultValue: 'Inbox' }))
                            : t('automation.history_disabled', { defaultValue: 'History off' });

                        return (
                            <AutomationRuleCard
                                key={rule.id}
                                title={displayRule.name}
                                typeLabel={t('automation.file_rule', { defaultValue: 'Folder' })}
                                projectLabel={projectLabel}
                                outputDirectory={displayRule.saveHistory ? undefined : displayRule.exportConfig?.directory}
                                watchDirectory={displayRule.watchDirectory}
                                enabled={displayRule.enabled}
                                canToggle={true}
                                onToggleEnabled={(value) => { void handleToggleEnabled(rule.id, value); }}
                                statusLabel={getRuntimeStatusLabel(runtime?.status)}
                                resultLabel={describeLastResult(rule.id)}
                                resultMessage={runtime?.lastResultMessage}
                                blockedHint={describeLatestBlockedHint(rule.id)}
                                pendingCount={queueSummary?.pending}
                                processingCount={queueSummary?.processing}
                                failureCount={runtime?.failureCount}
                                isExpanded={expandedRuleIds.has(rule.id)}
                                onToggleExpand={() => toggleExpanded(rule.id, createDraftFromRule(rule))}
                                onScanNow={() => { void handleScanNow(rule.id); }}
                                onRetryFailed={() => { void handleRetryFailed(rule.id); }}
                                onDelete={() => { void handleDelete(rule.id); }}
                                editor={draft ? createEditor(rule.id, draft) : null}
                            />
                        );
                    })
                )}
            </SettingsSection>
        </SettingsTabContainer>
    );
}

export default SettingsAutomationTab;
