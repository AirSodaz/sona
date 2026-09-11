import { Zap } from 'lucide-react';
import type React from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { LANGUAGE_OPTIONS } from '../../../constants/languages';
import { useProjectStore } from '../../../stores/projectStore';
import type { ExportFormat } from '../../../utils/exportFormats';
import { getLocalizedLanguageName } from '../../../utils/languageUtils';
import { getPolishPresetOptions } from '../../../utils/polishPresets';
import { Dropdown } from '../../Dropdown';
import { FolderIcon } from '../../Icons';
import { Switch } from '../../Switch';
import { SettingsItem } from '../SettingsLayout';
import {
  type AutomationDraftUpdate,
  type AutomationRuleDraft,
  setActionField,
  setDraftField,
  setExportConfigField,
} from './automationRuleDraft';

type SelectOption = { value: string; label: string };
type BrowseField = 'watchDirectory' | 'directory';

const DEFAULT_EXPORT_FORMAT_OPTIONS: SelectOption[] = [
  { value: 'txt', label: 'TXT' },
  { value: 'srt', label: 'SRT' },
  { value: 'vtt', label: 'VTT' },
  { value: 'json', label: 'JSON' },
  { value: 'docx', label: 'DOCX' },
];

type Props = {
  draft: AutomationRuleDraft;
  exportFormatOptions?: SelectOption[];
  exportModeOptions?: SelectOption[];
  languageOptions?: SelectOption[];
  onBrowseDirectory: (field: BrowseField) => void;
  onCancel: () => void;
  onSave: () => void;
  onUpdateDraft: (updater: AutomationDraftUpdate) => void;
  polishPresetOptions?: SelectOption[];
  profileOptions?: SelectOption[];
  projectOptions: SelectOption[];
};

export function AutomationRuleEditor({
  draft,
  exportFormatOptions,
  languageOptions: propsLanguageOptions,
  onBrowseDirectory,
  onCancel,
  onSave,
  onUpdateDraft,
  polishPresetOptions: propsPolishPresetOptions,
  projectOptions,
}: Props): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const projects = useProjectStore((state) => state.projects);
  const selectedProject = projects.find((p) => p.id === draft.projectId);
  const resolvedPolishPresetOptions = useMemo(
    () => propsPolishPresetOptions ?? getPolishPresetOptions(undefined, t),
    [propsPolishPresetOptions, t]
  );
  const resolvedLanguageOptions = useMemo(
    () =>
      propsLanguageOptions ??
      LANGUAGE_OPTIONS.map((language) => ({
        value: language.code,
        label: getLocalizedLanguageName(language.code, i18n?.language || 'zh'),
      })),
    [propsLanguageOptions, i18n?.language]
  );

  return (
    <div className="automation-editor-container">
      <div className="automation-editor-steps">
        {/* Step 1: WHEN - Trigger & Source */}
        <div className="automation-editor-step-card">
          <div className="automation-step-header">
            <span className="automation-step-badge">
              {t('automation.step_badge_when', { defaultValue: '1. WHEN' })}
            </span>
            <span className="automation-step-title">
              {t('automation.step_file_trigger', { defaultValue: 'Watch Local Folder' })}
            </span>
            <span className="automation-step-subtitle">
              {t('automation.step_file_hint', {
                defaultValue: 'Auto-ingests new audio dropped in directory',
              })}
            </span>
          </div>

          <div className="automation-step-content">
            <SettingsItem
              title={t('automation.rule_name', { defaultValue: 'Rule Name' })}
              layout="vertical"
            >
              <input
                className="settings-input"
                value={draft.name}
                onChange={(event) => onUpdateDraft(setDraftField('name', event.target.value))}
                placeholder={t('automation.rule_name_placeholder', {
                  defaultValue: 'e.g. Weekly Meeting Inbox',
                })}
              />
            </SettingsItem>

            <SettingsItem
              title={t('automation.watch_directory', { defaultValue: 'Watch Directory' })}
              layout="vertical"
            >
              <div style={{ display: 'flex', gap: '8px', width: '100%', alignItems: 'center' }}>
                <input
                  className="settings-input"
                  value={draft.watchDirectory}
                  onChange={(event) =>
                    onUpdateDraft(setDraftField('watchDirectory', event.target.value))
                  }
                  placeholder={t('automation.watch_directory_placeholder', {
                    defaultValue: 'Choose a folder to monitor...',
                  })}
                  style={{ flex: 1, minWidth: 0, height: 36 }}
                />
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => onBrowseDirectory('watchDirectory')}
                  title={t('settings.browse', { defaultValue: 'Browse' })}
                  style={{
                    height: 36,
                    padding: '0 12px',
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 6,
                    flexShrink: 0,
                    whiteSpace: 'nowrap',
                  }}
                >
                  <FolderIcon width={15} height={15} />
                  <span>{t('settings.browse', { defaultValue: 'Browse' })}</span>
                </button>
                <Switch
                  checked={draft.recursive}
                  onChange={(value) => onUpdateDraft(setDraftField('recursive', value))}
                  label={t('automation.recursive', { defaultValue: 'Subdirectories' })}
                  style={{ flexShrink: 0, marginLeft: 4, whiteSpace: 'nowrap' }}
                />
              </div>
            </SettingsItem>
          </div>
        </div>

        {/* Step 2: DESTINATION & PIPELINE */}
        <div className="automation-editor-step-card">
          <div className="automation-step-header">
            <span className="automation-step-badge">
              {draft.saveHistory
                ? t('automation.step_badge_target', { defaultValue: '2. TARGET' })
                : t('automation.step_badge_export', { defaultValue: '2. EXPORT' })}
            </span>
            <span className="automation-step-title">
              {draft.saveHistory
                ? t('automation.step_target_project', { defaultValue: 'Target Project & Pipeline' })
                : t('automation.step_export_title', {
                    defaultValue: 'Export Directory & Pipeline',
                  })}
            </span>
            <span className="automation-step-subtitle">
              {draft.saveHistory
                ? t('automation.step_target_hint', {
                    defaultValue: 'Assign ingested audio to a project and run its pipeline',
                  })
                : t('automation.step_export_hint', {
                    defaultValue:
                      'Specify output directory and optionally apply a processing pipeline',
                  })}
            </span>
          </div>

          <div className="automation-step-content">
            <SettingsItem title={t('automation.save_history', { defaultValue: 'Save to History' })}>
              <Switch
                checked={draft.saveHistory}
                onChange={(value) => {
                  onUpdateDraft((current) => ({
                    ...current,
                    saveHistory: value,
                    ...(!value
                      ? { projectId: 'none', tagIds: [] }
                      : current.projectId === 'none'
                        ? { projectId: 'inbox', tagIds: [] }
                        : {}),
                  }));
                }}
                aria-label={t('automation.save_history', { defaultValue: 'Save to History' })}
              />
            </SettingsItem>

            {draft.saveHistory ? (
              <>
                <SettingsItem
                  title={t('automation.target_project', { defaultValue: 'Target Project' })}
                >
                  <Dropdown
                    value={
                      draft.projectId && draft.projectId !== 'none' ? draft.projectId : 'inbox'
                    }
                    onChange={(value) => {
                      onUpdateDraft((current) => ({
                        ...current,
                        projectId: value,
                        tagIds: value && value !== 'none' && value !== 'inbox' ? [value] : [],
                      }));
                    }}
                    options={projectOptions}
                    style={{ width: '240px' }}
                    aria-label={t('automation.target_project', { defaultValue: 'Target Project' })}
                  />
                </SettingsItem>

                {/* Pipeline Overview Card */}
                <div
                  style={{
                    padding: '12px 14px',
                    background: 'var(--color-bg-secondary)',
                    borderRadius: 'var(--radius-md)',
                    border: '1px solid var(--color-border)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px',
                    fontSize: '0.8125rem',
                  }}
                >
                  <span style={{ fontWeight: 600, color: 'var(--color-text-primary)' }}>
                    {t('projects.pipeline_overview', { defaultValue: 'Pipeline Execution' })}
                  </span>
                  {selectedProject?.pipeline?.enabled ? (
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '4px',
                        color: 'var(--color-text-secondary)',
                      }}
                    >
                      <span>
                        ✓{' '}
                        {t('projects.pipeline_will_run', {
                          defaultValue:
                            'Will execute deterministic pipeline for project "{{name}}":',
                          name: selectedProject.name,
                        })}
                      </span>
                      <div
                        style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginTop: '4px' }}
                      >
                        {selectedProject.pipeline.autoPolish && (
                          <span className="automation-chip automation-chip-success">
                            {t('automation.auto_polish', { defaultValue: 'Polish' })}:{' '}
                            {selectedProject.pipeline.polishPresetId || 'general'}
                          </span>
                        )}
                        {selectedProject.pipeline.autoTranslate && (
                          <span className="automation-chip automation-chip-success">
                            {t('automation.auto_translate', { defaultValue: 'Translate' })}:{' '}
                            {selectedProject.pipeline.targetLanguage || 'en'}
                          </span>
                        )}
                        {selectedProject.pipeline.autoSummary && (
                          <span className="automation-chip automation-chip-success">
                            {t('automation.auto_summary', { defaultValue: 'Summary' })}:{' '}
                            {selectedProject.pipeline.summaryTemplateId || 'general'}
                          </span>
                        )}
                        {selectedProject.pipeline.autoExport && (
                          <span className="automation-chip automation-chip-success">
                            {t('automation.auto_export', { defaultValue: 'Export' })}:{' '}
                            {selectedProject.pipeline.exportDirectory}
                          </span>
                        )}
                      </div>
                    </div>
                  ) : (
                    <span style={{ color: 'var(--color-text-secondary)' }}>
                      {draft.projectId === 'inbox' || !selectedProject
                        ? t('automation.inbox_pipeline_hint', {
                            defaultValue:
                              'Audio will be assigned to Inbox and follow global default settings.',
                          })
                        : t('automation.disabled_pipeline_hint', {
                            defaultValue:
                              'Project pipeline is disabled. Will follow global default settings.',
                          })}
                    </span>
                  )}
                </div>
              </>
            ) : (
              <>
                <div
                  style={{
                    padding: '10px 12px',
                    background: 'var(--color-bg-secondary)',
                    border: '1px solid var(--color-border)',
                    borderRadius: 'var(--radius-md)',
                    fontSize: '0.8125rem',
                    color: 'var(--color-text-secondary)',
                    lineHeight: 1.5,
                  }}
                >
                  {t('automation.pure_export_notice', {
                    defaultValue:
                      'Export-only mode: audio will be transcribed and exported to the directory below without keeping records in local history.',
                  })}
                </div>
                <div className="project-pipeline-section">
                  <div
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 6,
                      fontWeight: 500,
                      fontSize: 13,
                      color: 'var(--color-text-primary)',
                    }}
                  >
                    <Zap size={15} style={{ color: 'var(--color-accent, #6366F1)' }} />
                    <span>
                      {t('automation.pipeline_preset', { defaultValue: 'Processing Pipeline' })}
                    </span>
                  </div>
                  <div className="project-pipeline-options">
                    {/* Auto Polish */}
                    <div className="project-pipeline-item">
                      <div className="project-pipeline-item-header">
                        <Switch
                          checked={Boolean(draft.actions?.autoPolish)}
                          onChange={(checked) =>
                            onUpdateDraft(setActionField('autoPolish', checked))
                          }
                          label={t('automation.auto_polish', { defaultValue: 'Auto Polish' })}
                        />
                      </div>
                      {draft.actions?.autoPolish && (
                        <div className="project-pipeline-item-content">
                          <Dropdown
                            value={draft.stageConfig?.polishPresetId || 'general'}
                            onChange={(value) =>
                              onUpdateDraft((current) => ({
                                ...current,
                                stageConfig: {
                                  ...current.stageConfig,
                                  polishPresetId: value,
                                },
                              }))
                            }
                            options={resolvedPolishPresetOptions}
                            style={{ width: '100%' }}
                            aria-label={t('automation.auto_polish', {
                              defaultValue: 'Auto Polish',
                            })}
                          />
                        </div>
                      )}
                    </div>

                    {/* Auto Translate */}
                    <div className="project-pipeline-item">
                      <div className="project-pipeline-item-header">
                        <Switch
                          checked={Boolean(draft.actions?.autoTranslate)}
                          onChange={(checked) =>
                            onUpdateDraft(setActionField('autoTranslate', checked))
                          }
                          label={t('automation.auto_translate', { defaultValue: 'Auto Translate' })}
                        />
                      </div>
                      {draft.actions?.autoTranslate && (
                        <div className="project-pipeline-item-content">
                          <Dropdown
                            value={draft.stageConfig?.translationLanguage || 'en'}
                            onChange={(value) =>
                              onUpdateDraft((current) => ({
                                ...current,
                                stageConfig: {
                                  ...current.stageConfig,
                                  translationLanguage: value,
                                },
                              }))
                            }
                            options={resolvedLanguageOptions}
                            style={{ width: '100%' }}
                            aria-label={t('automation.auto_translate', {
                              defaultValue: 'Auto Translate',
                            })}
                          />
                        </div>
                      )}
                    </div>
                  </div>
                </div>

                <SettingsItem
                  title={t('automation.output_directory', { defaultValue: 'Output Directory' })}
                >
                  <div
                    style={{
                      display: 'flex',
                      gap: '8px',
                      width: '100%',
                      maxWidth: '360px',
                      alignItems: 'center',
                    }}
                  >
                    <input
                      type="text"
                      className="settings-input"
                      value={draft.exportConfig.directory}
                      onChange={(e) =>
                        onUpdateDraft(setExportConfigField('directory', e.target.value))
                      }
                      placeholder={t('automation.output_directory_placeholder', {
                        defaultValue: 'Choose export directory...',
                      })}
                      style={{ flex: 1, minWidth: 0, height: 36 }}
                    />
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => onBrowseDirectory('directory')}
                      title={t('settings.browse', { defaultValue: 'Browse' })}
                      style={{
                        height: 36,
                        padding: '0 12px',
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 6,
                        flexShrink: 0,
                        whiteSpace: 'nowrap',
                      }}
                    >
                      <FolderIcon width={15} height={15} />
                      <span>{t('settings.browse', { defaultValue: 'Browse' })}</span>
                    </button>
                  </div>
                </SettingsItem>

                <SettingsItem
                  title={t('automation.export_format', { defaultValue: 'Export Format' })}
                >
                  <Dropdown
                    value={draft.exportConfig.format || 'txt'}
                    onChange={(value) =>
                      onUpdateDraft(setExportConfigField('format', value as ExportFormat))
                    }
                    options={exportFormatOptions ?? DEFAULT_EXPORT_FORMAT_OPTIONS}
                    style={{ width: '140px' }}
                    aria-label={t('automation.export_format', { defaultValue: 'Export Format' })}
                  />
                </SettingsItem>
              </>
            )}
          </div>
        </div>
      </div>

      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', paddingTop: '8px' }}>
        <button type="button" className="btn" onClick={onCancel}>
          {t('common.cancel')}
        </button>
        <button type="button" className="btn btn-primary" onClick={onSave}>
          {t('common.save')}
        </button>
      </div>
    </div>
  );
}

export default AutomationRuleEditor;
