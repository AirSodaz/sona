import React from 'react';
import { useTranslation } from 'react-i18next';
import { FolderIcon } from '../../Icons';
import { Dropdown } from '../../Dropdown';
import { Switch } from '../../Switch';
import { SettingsItem } from '../SettingsLayout';
import type { ExportFormat, ExportMode } from '../../../utils/exportFormats';
import type { AutomationDraftUpdate, AutomationRuleDraft } from './automationRuleDraft';
import {
    setDraftField,
    setExportConfigField,
    setStageConfigField,
} from './automationRuleDraft';

type SelectOption = {
    value: string;
    label: string;
};

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
    projectOptions: SelectOption[];
};

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
    projectOptions,
}: Props): React.JSX.Element {
    const { t } = useTranslation();

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

                <SettingsItem title={t('automation.auto_polish', { defaultValue: 'Auto-Polish' })}>
                    <Switch
                        checked={draft.stageConfig.autoPolish}
                        onChange={(value) => onUpdateDraft(setStageConfigField('autoPolish', value))}
                        aria-label={t('automation.auto_polish', { defaultValue: 'Auto-Polish' })}
                    />
                </SettingsItem>

                {draft.stageConfig.autoPolish && (
                    <SettingsItem indent title={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}>
                        <Dropdown
                            value={draft.stageConfig.polishPresetId || 'general'}
                            onChange={(value) => onUpdateDraft(setStageConfigField('polishPresetId', value))}
                            options={polishPresetOptions}
                            style={{ width: '160px' }}
                            aria-label={t('projects.polish_preset', { defaultValue: 'Polish Preset' })}
                        />
                    </SettingsItem>
                )}

                <SettingsItem title={t('automation.auto_translate', { defaultValue: 'Auto-Translate' })}>
                    <Switch
                        checked={draft.stageConfig.autoTranslate}
                        onChange={(value) => onUpdateDraft(setStageConfigField('autoTranslate', value))}
                        aria-label={t('automation.auto_translate', { defaultValue: 'Auto-Translate' })}
                    />
                </SettingsItem>

                {draft.stageConfig.autoTranslate && (
                    <SettingsItem indent title={t('translation.target_language', { defaultValue: 'Target Language' })}>
                        <Dropdown
                            value={draft.stageConfig.translationLanguage || 'en'}
                            onChange={(value) => onUpdateDraft(setStageConfigField('translationLanguage', value))}
                            options={languageOptions}
                            style={{ width: '160px' }}
                            aria-label={t('translation.target_language', { defaultValue: 'Target Language' })}
                        />
                    </SettingsItem>
                )}

                <SettingsItem title={t('automation.auto_export', { defaultValue: 'Auto-Export' })}>
                    <Switch
                        checked={draft.stageConfig.exportEnabled}
                        onChange={(value) => onUpdateDraft(setStageConfigField('exportEnabled', value))}
                        aria-label={t('automation.auto_export', { defaultValue: 'Auto-Export' })}
                    />
                </SettingsItem>

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

                <SettingsItem title={t('automation.target_project', { defaultValue: 'Target Project' })} layout="vertical">
                    <Dropdown
                        value={draft.projectId}
                        onChange={(value) => onUpdateDraft(setDraftField('projectId', value))}
                        options={projectOptions}
                        style={{ width: '100%' }}
                        aria-label={t('automation.target_project', { defaultValue: 'Target Project' })}
                    />
                </SettingsItem>
            </div>

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
                <button className="btn" onClick={onCancel}>
                    {t('common.cancel')}
                </button>
                <button className="btn btn-primary" onClick={onSave}>
                    {t('common.save')}
                </button>
            </div>
        </div>
    );
}
