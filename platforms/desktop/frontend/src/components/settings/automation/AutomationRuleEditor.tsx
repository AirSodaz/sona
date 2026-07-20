import React from 'react';
import { useTranslation } from 'react-i18next';
import { FolderIcon } from '../../Icons';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import { Checkbox } from '../../Checkbox';
import { SettingsItem } from '../SettingsLayout';
import type { ExportFormat, ExportMode } from '../../../utils/exportFormats';
import type { AutomationDraftUpdate, AutomationRuleDraft } from './automationRuleDraft';
import {
    setActionField,
    setDraftField,
    setExportConfigField,
    setStageConfigField,
} from './automationRuleDraft';

type SelectOption = { value: string; label: string };
type BrowseField = 'watchDirectory' | 'directory';

type Props = {
    draft: AutomationRuleDraft;
    exportFormatOptions: SelectOption[];
    exportModeOptions: SelectOption[];
    languageOptions: SelectOption[];
    onBrowseDirectory: (field: BrowseField) => void;
    onCancel: () => void;
    onSave: () => void;
    onUpdateDraft: (updater: AutomationDraftUpdate) => void;
    polishPresetOptions: SelectOption[];
    profileOptions: SelectOption[];
    projectOptions: SelectOption[];
};

function ActionSwitch({
    label,
    checked,
    onChange,
}: {
    label: string;
    checked: boolean;
    onChange: (value: boolean) => void;
}): React.JSX.Element {
    return (
        <SettingsItem title={label}>
            <Switch checked={checked} onChange={onChange} aria-label={label} />
        </SettingsItem>
    );
}

export function AutomationRuleEditor({
    draft,
    exportFormatOptions,
    exportModeOptions,
    languageOptions,
    onBrowseDirectory,
    onCancel,
    onSave,
    onUpdateDraft,
    polishPresetOptions,
    profileOptions,
    projectOptions,
}: Props): React.JSX.Element {
    const { t } = useTranslation();
    const isTagRule = draft.kind === 'tag';
    const tags = projectOptions.filter((option) => option.value !== 'none' && option.value !== 'inbox');

    return (
        <div style={{ padding: '0 24px 24px 56px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div className="settings-section-content">
                <SettingsItem title={t('automation.rule_name', { defaultValue: 'Rule Name' })} layout="vertical">
                    <input
                        className="settings-input"
                        value={draft.name}
                        onChange={(event) => onUpdateDraft(setDraftField('name', event.target.value))}
                        placeholder={t('automation.rule_name_placeholder', { defaultValue: 'e.g. Weekly Meeting Inbox' })}
                    />
                </SettingsItem>

                {isTagRule ? (
                    <>
                        <SettingsItem title={t('automation.priority', { defaultValue: 'Priority' })}>
                            <input
                                className="settings-input"
                                type="number"
                                min={0}
                                value={draft.priority}
                                onChange={(event) => onUpdateDraft(setDraftField('priority', Number(event.target.value) || 0))}
                                style={{ width: '120px' }}
                                aria-label={t('automation.priority', { defaultValue: 'Priority' })}
                            />
                        </SettingsItem>

                        <SettingsItem title={t('automation.match_tags', { defaultValue: 'Match any Tag' })} layout="vertical">
                            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', width: '100%' }}>
                                {tags.length === 0 ? (
                                    <span className="settings-item-hint">
                                        {t('automation.no_tags', { defaultValue: 'Create a tag first.' })}
                                    </span>
                                ) : tags.map((option) => (
                                    <Checkbox
                                        key={option.value}
                                        checked={draft.tagIds.includes(option.value)}
                                        label={option.label}
                                        onChange={(checked) => onUpdateDraft(setDraftField(
                                            'tagIds',
                                            checked
                                                ? Array.from(new Set([...draft.tagIds, option.value]))
                                                : draft.tagIds.filter((tagId) => tagId !== option.value),
                                        ))}
                                    />
                                ))}
                            </div>
                        </SettingsItem>

                        <SettingsItem title={t('automation.profile', { defaultValue: 'Configuration Profile' })}>
                            <Dropdown
                                value={draft.profileId || ''}
                                onChange={(value) => onUpdateDraft(setDraftField('profileId', value || undefined))}
                                options={profileOptions}
                                style={{ width: '220px' }}
                                aria-label={t('automation.profile', { defaultValue: 'Configuration Profile' })}
                            />
                        </SettingsItem>

                        <ActionSwitch
                            label={t('automation.auto_polish', { defaultValue: 'Polish' })}
                            checked={draft.actions.autoPolish}
                            onChange={(value) => onUpdateDraft(setActionField('autoPolish', value))}
                        />
                        {draft.actions.autoPolish && (
                            <SettingsItem indent title={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}>
                                <Dropdown
                                    value={draft.stageConfig.polishPresetId || 'general'}
                                    onChange={(value) => onUpdateDraft(setStageConfigField('polishPresetId', value))}
                                    options={polishPresetOptions}
                                    style={{ width: '180px' }}
                                    aria-label={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}
                                />
                            </SettingsItem>
                        )}
                        <ActionSwitch
                            label={t('automation.auto_translate', { defaultValue: 'Translate' })}
                            checked={draft.actions.autoTranslate}
                            onChange={(value) => onUpdateDraft(setActionField('autoTranslate', value))}
                        />
                        {draft.actions.autoTranslate && (
                            <SettingsItem indent title={t('translation.target_language', { defaultValue: 'Target Language' })}>
                                <Dropdown
                                    value={draft.stageConfig.translationLanguage || 'en'}
                                    onChange={(value) => onUpdateDraft(setStageConfigField('translationLanguage', value))}
                                    options={languageOptions}
                                    style={{ width: '180px' }}
                                    aria-label={t('translation.target_language', { defaultValue: 'Target Language' })}
                                />
                            </SettingsItem>
                        )}
                        <ActionSwitch
                            label={t('automation.auto_summary', { defaultValue: 'Summarize' })}
                            checked={draft.actions.autoSummary}
                            onChange={(value) => onUpdateDraft(setActionField('autoSummary', value))}
                        />
                    </>
                ) : (
                    <>
                        <SettingsItem title={t('automation.profile_source', { defaultValue: 'Profile Source' })}>
                            <Dropdown
                                value={draft.profileSource || 'explicit'}
                                onChange={(value) => onUpdateDraft(setDraftField('profileSource', value))}
                                options={[
                                    { value: 'explicit', label: t('automation.profile_explicit', { defaultValue: 'Explicit profile' }) },
                                    { value: 'tag_match', label: t('automation.profile_tag_match', { defaultValue: 'Resolve from Tag automation' }) },
                                ]}
                                style={{ width: '240px' }}
                                aria-label={t('automation.profile_source', { defaultValue: 'Profile Source' })}
                            />
                        </SettingsItem>
                        {draft.profileSource !== 'tag_match' && (
                            <SettingsItem indent title={t('automation.profile', { defaultValue: 'Configuration Profile' })}>
                                <Dropdown
                                    value={draft.profileId || ''}
                                    onChange={(value) => onUpdateDraft(setDraftField('profileId', value || undefined))}
                                    options={profileOptions}
                                    style={{ width: '220px' }}
                                    aria-label={t('automation.profile', { defaultValue: 'Configuration Profile' })}
                                />
                            </SettingsItem>
                        )}

                        <ActionSwitch
                            label={t('automation.auto_export', { defaultValue: 'Auto-Export' })}
                            checked={draft.stageConfig.exportEnabled}
                            onChange={(value) => onUpdateDraft(setStageConfigField('exportEnabled', value))}
                        />

                        {draft.stageConfig.exportEnabled && (
                            <>
                                <SettingsItem indent title={t('projects.export_prefix', { defaultValue: 'Filename Prefix' })}>
                                    <input
                                        className="settings-input"
                                        value={draft.exportConfig.prefix || ''}
                                        onChange={(event) => onUpdateDraft(setExportConfigField('prefix', event.target.value))}
                                        placeholder={t('projects.export_prefix', { defaultValue: 'e.g. [Auto]' })}
                                        style={{ width: '160px' }}
                                    />
                                </SettingsItem>
                                <SettingsItem indent title={t('automation.export_format', { defaultValue: 'Export Format' })}>
                                    <Dropdown
                                        value={draft.exportConfig.format}
                                        onChange={(value) => onUpdateDraft(setExportConfigField('format', value as ExportFormat))}
                                        options={exportFormatOptions}
                                        style={{ width: '160px' }}
                                        aria-label={t('automation.export_format', { defaultValue: 'Export Format' })}
                                    />
                                </SettingsItem>
                                <SettingsItem indent title={t('automation.export_mode', { defaultValue: 'Export Mode' })}>
                                    <Dropdown
                                        value={draft.exportConfig.mode}
                                        onChange={(value) => onUpdateDraft(setExportConfigField('mode', value as ExportMode))}
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
                                    onChange={(event) => onUpdateDraft(setDraftField('watchDirectory', event.target.value))}
                                    placeholder={t('automation.watch_directory_placeholder', { defaultValue: 'Choose a folder to monitor...' })}
                                    style={{ flex: 1 }}
                                />
                                <button className="btn btn-secondary" onClick={() => onBrowseDirectory('watchDirectory')}>
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
                                    onChange={(event) => onUpdateDraft(setExportConfigField('directory', event.target.value, false))}
                                    placeholder={t('automation.output_directory_placeholder', { defaultValue: 'Choose where exports should be written...' })}
                                    style={{ flex: 1 }}
                                />
                                <button className="btn btn-secondary" onClick={() => onBrowseDirectory('directory')}>
                                    <FolderIcon />
                                    <span>{t('settings.browse', { defaultValue: 'Browse' })}</span>
                                </button>
                            </div>
                        </SettingsItem>
                        <SettingsItem title={t('automation.recursive', { defaultValue: 'Watch Subfolders' })}>
                            <Switch
                                checked={draft.recursive}
                                onChange={(value) => onUpdateDraft(setDraftField('recursive', value))}
                                aria-label={t('automation.recursive', { defaultValue: 'Watch Subfolders' })}
                            />
                        </SettingsItem>
                        <SettingsItem title={t('automation.target_tags', { defaultValue: 'Output Tags' })} layout="vertical">
                            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', width: '100%' }}>
                                {tags.map((option) => (
                                    <Checkbox
                                        key={option.value}
                                        checked={draft.tagIds.includes(option.value)}
                                        label={option.label}
                                        onChange={(checked) => onUpdateDraft(setDraftField(
                                            'tagIds',
                                            checked
                                                ? Array.from(new Set([...draft.tagIds, option.value]))
                                                : draft.tagIds.filter((tagId) => tagId !== option.value),
                                        ))}
                                    />
                                ))}
                            </div>
                        </SettingsItem>
                    </>
                )}
            </div>

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
                <button className="btn" onClick={onCancel}>{t('common.cancel')}</button>
                <button className="btn btn-primary" onClick={onSave}>{t('common.save')}</button>
            </div>
        </div>
    );
}
