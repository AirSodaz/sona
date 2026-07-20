import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AutomationIcon } from '../Icons';
import { useAutomationStore } from '../../stores/automationStore';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useDialogStore } from '../../stores/dialogStore';
import { getPolishPresetOptions } from '../../utils/polishPresets';
import { getSummaryTemplateOptions } from '../../utils/summaryTemplates';
import { getLocalizedLanguageName } from '../../utils/languageUtils';
import { LANGUAGE_OPTIONS } from '../../constants/languages';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import type {
    AutomationRule,
    AutomationRuntimeStatus,
} from '../../types/automation';
import { AutomationRuleCard } from './automation/AutomationRuleCard';
import { AutomationRuleEditor } from './automation/AutomationRuleEditor';
import { AutomationProfileEditor, type AutomationProfileDraft } from './automation/AutomationProfileEditor';
import {
    createDraftFromRule,
    createRuleDraft,
    NEW_RULE_KEY,
    normalizeAutomationRuleDraft,
    setDraftField,
    setExportConfigField,
    type AutomationDraftUpdate,
    type AutomationRuleDraft,
} from './automation/automationRuleDraft';
import { openDialog } from '../../services/tauri/platform/dialog';

type BrowseField = 'watchDirectory' | 'directory';
type SelectOption = {
    value: string;
    label: string;
};

export function SettingsAutomationTab(): React.JSX.Element {
    const { t, i18n } = useTranslation();
    const rules = useAutomationStore((state) => state.rules);
    const profiles = useAutomationStore((state) => state.profiles);
    const runtimeStates = useAutomationStore((state) => state.runtimeStates);
    const saveRule = useAutomationStore((state) => state.saveRule);
    const deleteRule = useAutomationStore((state) => state.deleteRule);
    const toggleRuleEnabled = useAutomationStore((state) => state.toggleRuleEnabled);
    const scanRuleNow = useAutomationStore((state) => state.scanRuleNow);
    const retryFailed = useAutomationStore((state) => state.retryFailed);
    const saveProfile = useAutomationStore((state) => state.saveProfile);
    const deleteProfile = useAutomationStore((state) => state.deleteProfile);
    const applyTagRuleToExisting = useAutomationStore((state) => state.applyTagRuleToExisting);
    const focusTagId = useAutomationStore((state) => state.focusTagId);
    const setFocusTagId = useAutomationStore((state) => state.setFocusTagId);
    const queueItems = useBatchQueueStore((state) => state.queueItems);
    const config = useConfigStore((state) => state.config);
    const projects = useProjectStore((state) => state.projects);
    const alert = useDialogStore((state) => state.alert);
    const confirm = useDialogStore((state) => state.confirm);
    const showError = useDialogStore((state) => state.showError);
    const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set());
    const [drafts, setDrafts] = useState<Record<string, AutomationRuleDraft>>({});
    const [selectedSection, setSelectedSection] = useState<'profiles' | 'tag' | 'file'>('file');
    const [profileDrafts, setProfileDrafts] = useState<Record<string, AutomationProfileDraft>>({});
    const [expandedProfileIds, setExpandedProfileIds] = useState<Set<string>>(new Set());
    const activeSection = focusTagId ? 'tag' : selectedSection;

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
            value: language.code,
            label: getLocalizedLanguageName(language.code, i18n?.language || 'zh'),
        }))
    ), [i18n?.language]);

    const polishPresetOptions = useMemo<SelectOption[]>(() => (
        getPolishPresetOptions(config.polishCustomPresets, t)
    ), [config.polishCustomPresets, t]);

    const summaryTemplateOptions = useMemo<SelectOption[]>(() => (
        getSummaryTemplateOptions(config.summaryCustomTemplates, t)
    ), [config.summaryCustomTemplates, t]);

    const profileOptions = useMemo<SelectOption[]>(() => ([
        { value: '', label: t('automation.profile_global_fallback', { defaultValue: 'Global settings (fallback)' }) },
        ...profiles.map((profile) => ({ value: profile.id, label: profile.name })),
    ]), [profiles, t]);

    const namedSets = useMemo(() => ({
        textReplacementSets: (config.textReplacementSets || []).map((item) => ({ id: item.id, name: item.name })),
        hotwordSets: (config.hotwordSets || []).map((item) => ({ id: item.id, name: item.name })),
        polishKeywordSets: (config.polishKeywordSets || []).map((item) => ({ id: item.id, name: item.name })),
        speakerProfiles: (config.speakerProfiles || []).map((item) => ({ id: item.id, name: item.name })),
    }), [config.hotwordSets, config.polishKeywordSets, config.speakerProfiles, config.textReplacementSets]);

    const visibleRules = useMemo(
        () => rules.filter((rule) => (
            (rule.kind ?? 'file') === activeSection
            && (activeSection !== 'tag' || !focusTagId || (rule.tagIds || []).includes(focusTagId))
        )),
        [activeSection, focusTagId, rules],
    );

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

    const beginCreateRule = (kind: 'tag' | 'file' = activeSection === 'tag' ? 'tag' : 'file') => {
        ensureDraft(NEW_RULE_KEY, createRuleDraft('inbox', kind));
        setExpandedRuleIds((current) => new Set(current).add(NEW_RULE_KEY));
    };

    const handleBrowseDirectory = async (draftKey: string, field: BrowseField) => {
        const draft = drafts[draftKey];
        if (!draft) {
            return;
        }

        const selected = await openDialog({
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

        const missingTagFields = draft.kind === 'tag' && draft.tagIds.length === 0;
        const missingFileFields = draft.kind === 'file'
            && (!draft.watchDirectory.trim() || !draft.exportConfig.directory.trim());
        if (!draft.name.trim() || missingTagFields || missingFileFields) {
            await alert(
                draft.kind === 'tag'
                    ? t('automation.tag_required_fields', {
                        defaultValue: 'Complete the name and select at least one Tag before saving.',
                    })
                    : t('automation.required_fields', {
                        defaultValue: 'Complete the name, watch directory, and output directory before saving.',
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
            profileOptions={profileOptions}
            projectOptions={projectOptions}
        />
    );

    const createProfileDraft = (source?: AutomationProfileDraft): AutomationProfileDraft => source ?? {
        id: '',
        name: '',
        translationLanguage: config.translationLanguage || 'zh',
        polishPresetId: config.polishPresetId || 'general',
        summaryTemplateId: config.summaryTemplateId || 'general',
        enabledTextReplacementSetIds: (config.textReplacementSets || []).filter((item) => item.enabled).map((item) => item.id),
        enabledHotwordSetIds: (config.hotwordSets || []).filter((item) => item.enabled).map((item) => item.id),
        enabledPolishKeywordSetIds: (config.polishKeywordSets || []).filter((item) => item.enabled).map((item) => item.id),
        enabledSpeakerProfileIds: (config.speakerProfiles || []).filter((item) => item.enabled).map((item) => item.id),
    };

    const handleApplyExisting = async (ruleId: string) => {
        const confirmed = await confirm(
            t('automation.apply_existing_confirm', {
                defaultValue: 'Apply this rule to all matching existing records? This can update transcript text, translations, and summaries.',
            }),
            { title: t('automation.apply_existing', { defaultValue: 'Apply to existing records' }) },
        );
        if (!confirmed) return;

        try {
            const count = await applyTagRuleToExisting(ruleId);
            await alert(t('automation.apply_existing_complete', {
                defaultValue: 'Processed {{count}} matching records.',
                count,
            }), { variant: 'success' });
        } catch (error) {
            await showError({
                code: 'automation.apply_existing_failed',
                messageKey: 'errors.automation.apply_existing_failed',
                cause: error,
            });
        }
    };

    const beginCreateProfile = () => {
        setProfileDrafts((current) => ({ ...current, __new_profile__: createProfileDraft() }));
        setExpandedProfileIds((current) => new Set(current).add('__new_profile__'));
    };

    const beginEditProfile = (profileId: string) => {
        const profile = profiles.find((item) => item.id === profileId);
        if (!profile) return;
        setProfileDrafts((current) => ({
            ...current,
            [profileId]: {
                id: profile.id,
                name: profile.name,
                translationLanguage: profile.translationLanguage,
                polishPresetId: profile.polishPresetId,
                summaryTemplateId: profile.summaryTemplateId,
                enabledTextReplacementSetIds: [...profile.enabledTextReplacementSetIds],
                enabledHotwordSetIds: [...profile.enabledHotwordSetIds],
                enabledPolishKeywordSetIds: [...profile.enabledPolishKeywordSetIds],
                enabledSpeakerProfileIds: [...profile.enabledSpeakerProfileIds],
            },
        }));
        setExpandedProfileIds((current) => new Set(current).add(profileId));
    };

    const closeProfileDraft = (key: string) => {
        setProfileDrafts((current) => {
            const next = { ...current };
            delete next[key];
            return next;
        });
        setExpandedProfileIds((current) => {
            const next = new Set(current);
            next.delete(key);
            return next;
        });
    };

    const handleSaveProfile = async (key: string) => {
        const draft = profileDrafts[key];
        if (!draft?.name.trim()) {
            await alert(t('automation.profile_name_required', { defaultValue: 'Enter a profile name before saving.' }), { variant: 'warning' });
            return;
        }
        await saveProfile({ ...draft, id: draft.id || undefined, name: draft.name.trim() });
        closeProfileDraft(key);
    };

    const handleDuplicateProfile = async (profileId: string) => {
        const profile = profiles.find((item) => item.id === profileId);
        if (!profile) return;
        await saveProfile({
            ...profile,
            id: undefined,
            name: t('automation.profile_copy_name', { defaultValue: '{{name}} Copy', name: profile.name }),
        });
    };

    const handleDeleteProfile = async (profileId: string) => {
        const dependencies = rules.filter((rule) => rule.profileId === profileId);
        if (dependencies.length > 0) {
            await alert(t('automation.profile_in_use', {
                defaultValue: 'This profile is used by {{count}} automation rules.',
                count: dependencies.length,
            }), { variant: 'warning' });
            return;
        }
        const confirmed = await confirm(t('automation.profile_delete_confirm', { defaultValue: 'Delete this configuration profile?' }), {
            title: t('automation.profile_delete_title', { defaultValue: 'Delete Profile' }),
        });
        if (confirmed) await deleteProfile(profileId);
    };

    const newRuleDraft = drafts[NEW_RULE_KEY];
    const visibleNewRuleDraft = newRuleDraft && newRuleDraft.kind === activeSection ? newRuleDraft : undefined;

    const renderProfileEditor = (key: string, draft: AutomationProfileDraft) => (
        <AutomationProfileEditor
            draft={draft}
            hotwordSets={namedSets.hotwordSets}
            languageOptions={languageOptions}
            onCancel={() => closeProfileDraft(key)}
            onChange={(nextDraft) => setProfileDrafts((current) => ({ ...current, [key]: nextDraft }))}
            onSave={() => { void handleSaveProfile(key); }}
            polishKeywordSets={namedSets.polishKeywordSets}
            polishPresetOptions={polishPresetOptions}
            speakerProfiles={namedSets.speakerProfiles}
            summaryTemplateOptions={summaryTemplateOptions}
            textReplacementSets={namedSets.textReplacementSets}
        />
    );

    return (
        <SettingsTabContainer id="settings-panel-automation" ariaLabelledby="settings-tab-automation">
            <SettingsPageHeader
                icon={<AutomationIcon width={28} height={28} />}
                title={t('automation.title', { defaultValue: 'Automation' })}
                description={t('automation.description', {
                    defaultValue: 'Profiles define reusable processing settings. Tag automation runs post-processing, and file automation owns folder watching and export.',
                })}
            />

            <div
                role="tablist"
                aria-label={t('automation.sections', { defaultValue: 'Automation sections' })}
                style={{ display: 'flex', gap: '8px', padding: '0 24px 16px', flexWrap: 'wrap' }}
            >
                {([
                    ['profiles', t('automation.profiles', { defaultValue: 'Profiles' })],
                    ['tag', t('automation.tag_rules', { defaultValue: 'Tag Automation' })],
                    ['file', t('automation.file_rules', { defaultValue: 'File Automation' })],
                ] as const).map(([id, label]) => (
                    <button
                        key={id}
                        type="button"
                        role="tab"
                        aria-selected={activeSection === id}
                        className={`btn ${activeSection === id ? 'btn-primary' : 'btn-secondary'}`}
                        onClick={() => {
                            setSelectedSection(id);
                            if (id !== 'tag') setFocusTagId(null);
                            closeDraft(NEW_RULE_KEY);
                        }}
                    >
                        {label}
                    </button>
                ))}
            </div>

            {activeSection === 'profiles' ? (
                <SettingsSection
                    title={t('automation.profiles', { defaultValue: 'Configuration Profiles' })}
                    description={t('automation.profiles_description', {
                        defaultValue: 'Bundle language, templates, vocabularies, hotwords, polish keywords, and speaker profiles for reuse.',
                    })}
                >
                    <div className="settings-item-container layout-horizontal">
                        <div className="settings-item-info">
                            <div className="settings-item-title">
                                {t('automation.profile_count', { defaultValue: '{{count}} profiles configured.', count: profiles.length })}
                            </div>
                            <div className="settings-item-hint">
                                {t('automation.profile_fallback_hint', { defaultValue: 'Rules without a profile use global settings.' })}
                            </div>
                        </div>
                        <div className="settings-item-action">
                            <button className="btn btn-primary" onClick={beginCreateProfile}>
                                {t('automation.new_profile', { defaultValue: 'New Profile' })}
                            </button>
                        </div>
                    </div>

                    {profileDrafts.__new_profile__ && renderProfileEditor('__new_profile__', profileDrafts.__new_profile__)}

                    {profiles.length === 0 && !profileDrafts.__new_profile__ ? (
                        <div className="settings-item-container">
                            <div className="settings-item-info">
                                <div className="settings-item-title">
                                    {t('automation.profile_empty', { defaultValue: 'No profiles yet.' })}
                                </div>
                                <div className="settings-item-hint">
                                    {t('automation.profile_empty_hint', { defaultValue: 'Create a profile or keep using global settings as the fallback.' })}
                                </div>
                            </div>
                        </div>
                    ) : profiles.map((profile) => {
                        const dependencies = rules.filter((rule) => rule.profileId === profile.id);
                        return (
                            <div key={profile.id} style={{ borderTop: '1px solid var(--color-border)' }}>
                                <div className="settings-item-container layout-horizontal">
                                    <div className="settings-item-info">
                                        <div className="settings-item-title">{profile.name}</div>
                                        <div className="settings-item-hint">
                                            {dependencies.length > 0
                                                ? t('automation.profile_dependencies', {
                                                    defaultValue: 'Used by: {{names}}',
                                                    names: dependencies.map((rule) => rule.name).join(', '),
                                                })
                                                : t('automation.profile_no_dependencies', { defaultValue: 'Not used by any rule.' })}
                                        </div>
                                    </div>
                                    <div className="settings-item-action" style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                                        <button className="btn btn-secondary" onClick={() => beginEditProfile(profile.id)}>
                                            {t('common.edit', { defaultValue: 'Edit' })}
                                        </button>
                                        <button className="btn btn-secondary" onClick={() => { void handleDuplicateProfile(profile.id); }}>
                                            {t('common.duplicate', { defaultValue: 'Duplicate' })}
                                        </button>
                                        <button className="btn btn-secondary" onClick={() => { void handleDeleteProfile(profile.id); }}>
                                            {t('common.delete')}
                                        </button>
                                    </div>
                                </div>
                                {expandedProfileIds.has(profile.id) && profileDrafts[profile.id]
                                    ? renderProfileEditor(profile.id, profileDrafts[profile.id])
                                    : null}
                            </div>
                        );
                    })}
                </SettingsSection>
            ) : (
            <SettingsSection
                title={activeSection === 'tag'
                    ? t('automation.tag_rules', { defaultValue: 'Tag Automation' })
                    : t('automation.file_rules', { defaultValue: 'File Automation' })}
                description={activeSection === 'tag'
                    ? t('automation.tag_rules_description', {
                        defaultValue: 'The highest-priority matching rule runs polish, translation, and summary after transcription. Tag automation never exports.',
                    })
                    : t('automation.file_rules_description', {
                        defaultValue: 'Watch folders, transcribe files, resolve Tag post-processing, then export with this file rule.',
                    })}
            >
                {activeSection === 'tag' && focusTagId && (
                    <div className="settings-item-container layout-horizontal">
                        <div className="settings-item-info">
                            <div className="settings-item-title">
                                {t('automation.filtered_tag', {
                                    defaultValue: 'Filtered by Tag: {{name}}',
                                    name: projects.find((project) => project.id === focusTagId)?.name || focusTagId,
                                })}
                            </div>
                            <div className="settings-item-hint">
                                {t('automation.filtered_tag_hint', { defaultValue: 'Only rules that match this Tag are shown.' })}
                            </div>
                        </div>
                        <div className="settings-item-action">
                            <button className="btn btn-secondary" onClick={() => setFocusTagId(null)}>
                                {t('common.clear_filter', { defaultValue: 'Clear filter' })}
                            </button>
                        </div>
                    </div>
                )}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('automation.rule_count', { defaultValue: '{{count}} rules configured.', count: visibleRules.length })}
                        </div>
                        <div className="settings-item-hint">
                            {activeSection === 'tag'
                                ? t('automation.tag_list_hint', {
                                    defaultValue: 'A record uses one complete matching rule. Equal priorities are ordered by stable rule ID.',
                                })
                                : t('automation.file_list_hint', {
                                    defaultValue: 'File profile selection overrides Tag-matched profiles, then falls back to global settings.',
                                })}
                        </div>
                    </div>
                    <div className="settings-item-action">
                        <button className="btn btn-primary" onClick={() => beginCreateRule(activeSection)}>
                            {t('automation.new_rule', { defaultValue: 'New Rule' })}
                        </button>
                    </div>
                </div>

                {visibleNewRuleDraft && (
                    <AutomationRuleCard
                        title={visibleNewRuleDraft.name.trim() || t('automation.create_rule', { defaultValue: 'Create Rule' })}
                        typeLabel={visibleNewRuleDraft.kind === 'tag'
                            ? t('automation.tag_rule', { defaultValue: 'Tag' })
                            : t('automation.file_rule', { defaultValue: 'File' })}
                        projectLabel={visibleNewRuleDraft.saveHistory
                            ? visibleNewRuleDraft.tagIds
                                .map((tagId) => projectOptions.find((option) => option.value === tagId)?.label)
                                .filter(Boolean).join(', ') || t('projects.untagged', { defaultValue: 'Untagged' })
                            : t('automation.history_disabled', { defaultValue: 'History off' })}
                        profileLabel={profileOptions.find((option) => option.value === (visibleNewRuleDraft.profileId || ''))?.label}
                        priorityLabel={visibleNewRuleDraft.kind === 'tag'
                            ? t('automation.priority_value', { defaultValue: 'Priority {{priority}}', priority: visibleNewRuleDraft.priority })
                            : undefined}
                        watchDirectory={visibleNewRuleDraft.kind === 'file' ? visibleNewRuleDraft.watchDirectory : undefined}
                        outputDirectory={visibleNewRuleDraft.kind === 'file' ? visibleNewRuleDraft.exportConfig.directory : undefined}
                        resultLabel={t('automation.draft_badge', { defaultValue: 'Draft' })}
                        enabled={true}
                        canToggle={false}
                        isExpanded={expandedRuleIds.has(NEW_RULE_KEY)}
                        onToggleExpand={() => toggleExpanded(NEW_RULE_KEY, createRuleDraft('inbox', activeSection))}
                        editor={createEditor(NEW_RULE_KEY, visibleNewRuleDraft)}
                    />
                )}

                {visibleRules.length === 0 && !visibleNewRuleDraft ? (
                    <div className="settings-item-container">
                        <div className="settings-item-info">
                            <div className="settings-item-title">
                                {t('automation.empty_title', { defaultValue: 'No automation rules yet.' })}
                            </div>
                            <div className="settings-item-hint">
                                {activeSection === 'tag'
                                    ? t('automation.tag_empty_hint', {
                                        defaultValue: 'Add a rule to run post-processing when a transcription has any matching Tag.',
                                    })
                                    : t('automation.file_empty_hint', {
                                        defaultValue: 'Add a rule to watch a folder, transcribe new files, and export the results.',
                                    })}
                            </div>
                        </div>
                    </div>
                ) : visibleRules.map((rule: AutomationRule) => {
                    const draft = drafts[rule.id];
                    const displayRule = draft || createDraftFromRule(rule);
                    const runtime = runtimeStates[rule.id];
                    const queueSummary = queueSummaryByRuleId.get(rule.id);

                    return (
                        <AutomationRuleCard
                            key={rule.id}
                            title={displayRule.name}
                            typeLabel={displayRule.kind === 'tag'
                                ? t('automation.tag_rule', { defaultValue: 'Tag' })
                                : t('automation.file_rule', { defaultValue: 'File' })}
                            projectLabel={displayRule.saveHistory
                                ? displayRule.tagIds
                                    .map((tagId) => projectOptions.find((option) => option.value === tagId)?.label)
                                    .filter(Boolean).join(', ') || t('projects.untagged', { defaultValue: 'Untagged' })
                                : t('automation.history_disabled', { defaultValue: 'History off' })}
                            profileLabel={profileOptions.find((option) => option.value === (displayRule.profileId || ''))?.label}
                            priorityLabel={displayRule.kind === 'tag'
                                ? t('automation.priority_value', { defaultValue: 'Priority {{priority}}', priority: displayRule.priority })
                                : undefined}
                            watchDirectory={displayRule.kind === 'file' ? displayRule.watchDirectory : undefined}
                            outputDirectory={displayRule.kind === 'file' ? displayRule.exportConfig.directory : undefined}
                            statusLabel={displayRule.kind === 'file' ? getRuntimeStatusLabel(runtime?.status) : undefined}
                            resultLabel={displayRule.kind === 'file' ? describeLastResult(rule.id) : undefined}
                            failureCount={displayRule.kind === 'file' ? runtime?.failureCount || 0 : undefined}
                            pendingCount={displayRule.kind === 'file' ? queueSummary?.pending || 0 : undefined}
                            processingCount={displayRule.kind === 'file' ? queueSummary?.processing || 0 : undefined}
                            resultMessage={displayRule.kind === 'file' ? runtime?.lastResultMessage : undefined}
                            blockedHint={displayRule.kind === 'file' ? describeLatestBlockedHint(rule.id) : undefined}
                            migrationNotice={displayRule.migrationNotice}
                            enabled={rule.enabled}
                            canToggle={true}
                            isExpanded={expandedRuleIds.has(rule.id)}
                            onToggleExpand={() => toggleExpanded(rule.id, createDraftFromRule(rule))}
                            onToggleEnabled={(value) => { void handleToggleRule(rule, value); }}
                            onScanNow={displayRule.kind === 'file' ? () => { void handleScanNow(rule.id); } : undefined}
                            onRetryFailed={displayRule.kind === 'file' ? () => { void handleRetryFailed(rule.id); } : undefined}
                            onApplyExisting={displayRule.kind === 'tag'
                                ? () => { void handleApplyExisting(rule.id); }
                                : undefined}
                            onDelete={() => { void handleDelete(rule.id); }}
                            editor={createEditor(rule.id, displayRule)}
                        />
                    );
                })}
            </SettingsSection>
            )}
        </SettingsTabContainer>
    );
}
