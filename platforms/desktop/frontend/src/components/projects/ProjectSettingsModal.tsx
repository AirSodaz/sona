import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Zap, FolderOpen } from 'lucide-react';
import type { ProjectPipelineConfig, ProjectRecord } from '../../types/project';
import { DEFAULT_PROJECT_PIPELINE } from '../../types/project';
import { FolderIcon } from '../Icons';
import { IconPicker } from '../IconPicker';
import { Modal } from '../Modal';
import { Dropdown, type DropdownOption } from '../Dropdown';
import { Switch } from '../Switch';
import { getPolishPresetOptions } from '../../utils/polishPresets';
import { getSummaryTemplateOptions } from '../../utils/summaryTemplates';
import { LANGUAGE_OPTIONS } from '../../constants/languages';
import { getLocalizedLanguageName } from '../../utils/languageUtils';
import { openDialog } from '../../services/tauri/platform/dialog';
import { useConfigStore } from '../../stores/configStore';

const EXPORT_FORMAT_OPTIONS: DropdownOption[] = [
  { value: 'txt', label: 'TXT' },
  { value: 'srt', label: 'SRT' },
  { value: 'vtt', label: 'VTT' },
  { value: 'json', label: 'JSON' },
  { value: 'docx', label: 'DOCX' },
];
interface ProjectSettingsModalProps {
  isOpen: boolean;
  project: ProjectRecord | null;
  draftName: string;
  draftDescription: string;
  draftIcon: string;
  draftColor: string;
  draftPipeline?: ProjectRecord['pipeline'];
  onClose: () => void;
  onSave: () => void;
  onDelete: () => void;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onIconChange: (value: string) => void;
  onColorChange: (value: string) => void;
  onPipelineChange?: (value: ProjectRecord['pipeline']) => void;
}

export function ProjectSettingsModal({
  isOpen,
  project,
  draftName,
  draftDescription,
  draftIcon,
  draftColor,
  onClose,
  onSave,
  onDelete,
  onNameChange,
  onDescriptionChange,
  onIconChange,
  onColorChange,
  draftPipeline,
  onPipelineChange,
}: ProjectSettingsModalProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const hotwordSets = config.hotwordSets || [];
  const replacementSets = config.textReplacementSets || [];
  const polishPresetOptions = useMemo(() => getPolishPresetOptions(undefined, t), [t]);
  const summaryTemplateOptions = useMemo(() => getSummaryTemplateOptions(undefined, t), [t]);
  const languageOptions = useMemo(() => (
    LANGUAGE_OPTIONS.map((language) => ({
      value: language.code,
      label: getLocalizedLanguageName(language.code, i18n?.language || 'zh'),
    }))
  ), [i18n?.language]);

  if (!isOpen || !project) {
    return null;
  }

  const pipeline: ProjectPipelineConfig = draftPipeline ?? { ...DEFAULT_PROJECT_PIPELINE };

  const updatePipeline = (patch: Partial<ProjectPipelineConfig>) => {
    onPipelineChange?.({ ...pipeline, ...patch });
  };

  const handleBrowseExportDir = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath: pipeline.exportDirectory || undefined,
    });
    if (selected && typeof selected === 'string') {
      updatePipeline({ exportDirectory: selected });
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={
        <div>
          <span style={{ fontSize: '1.125rem', fontWeight: 600, color: 'var(--color-text-primary)' }}>
            {t('projects.project_settings_title', { defaultValue: 'Project settings' })}
          </span>
          <p style={{ fontSize: '0.8125rem', color: 'var(--color-text-secondary)', margin: '4px 0 0 0', fontWeight: 400 }}>
            {t('projects.project_settings_hint', {
              defaultValue: 'Configure project details and its deterministic processing pipeline.',
            })}
          </p>
        </div>
      }
      size="lg"
      footer={
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%', gap: '12px' }}>
          <button type="button" className="btn btn-danger" onClick={() => void onDelete()}>
            {t('projects.delete_project', { defaultValue: 'Delete Project' })}
          </button>
          <div style={{ display: 'flex', gap: 'var(--spacing-sm)', flexWrap: 'wrap', justifyContent: 'flex-end' }}>
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button type="button" className="btn btn-primary" onClick={() => void onSave()}>
              {t('common.save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      }
    >
      <div className="settings-content-scroll" style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-lg)' }}>
        {/* Basic Info */}
        <div className="projects-field">
          <label htmlFor="project-settings-name">
            {t('projects.project_name', { defaultValue: 'Project name' })}
          </label>
          <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
            <IconPicker
              icon={draftIcon}
              onChange={onIconChange}
              defaultIcon={<FolderIcon />}
              color={draftColor}
              onColorChange={onColorChange}
            />
            <input
              id="project-settings-name"
              type="text"
              className="settings-input"
              style={{ flex: 1 }}
              value={draftName}
              onChange={(event) => onNameChange(event.target.value)}
              placeholder={t('projects.new_project_name', { defaultValue: 'Project name' })}
              autoFocus
            />
          </div>
        </div>


        <div className="projects-field">
          <label htmlFor="project-settings-description">
            {t('projects.project_description', { defaultValue: 'Description' })}
          </label>
          <textarea
            id="project-settings-description"
            className="settings-input"
            value={draftDescription}
            onChange={(event) => onDescriptionChange(event.target.value)}
            style={{ minHeight: '60px' }}
          />
        </div>

        {/* Pipeline Configuration */}
        {onPipelineChange && (
          <div className="project-pipeline-section">
            <div className="project-pipeline-toggle">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                <Zap size={15} style={{ color: 'var(--color-accent-primary, #64748b)' }} />
                {t('projects.pipeline_enable', { defaultValue: 'Enable Project Pipeline' })}
              </span>
              <Switch
                checked={Boolean(pipeline.enabled)}
                onChange={(checked) => updatePipeline({ enabled: checked })}
                aria-label={t('projects.pipeline_enable', { defaultValue: 'Enable Project Pipeline' })}
              />
            </div>

            {pipeline.enabled && (
              <div className="project-pipeline-options">
                {/* Auto Polish */}
                <div className="project-pipeline-item">
                  <div className="project-pipeline-item-header">
                    <Switch
                      checked={Boolean(pipeline.autoPolish)}
                      onChange={(checked) => updatePipeline({ autoPolish: checked })}
                      label={t('automation.auto_polish', { defaultValue: 'Auto Polish' })}
                    />
                  </div>
                  {pipeline.autoPolish && (
                    <div className="project-pipeline-item-content">
                      <Dropdown
                        value={pipeline.polishPresetId || 'general'}
                        onChange={(value) => updatePipeline({ polishPresetId: value })}
                        options={polishPresetOptions}
                        style={{ width: '100%' }}
                        aria-label={t('automation.auto_polish', { defaultValue: 'Auto Polish' })}
                      />
                    </div>
                  )}
                </div>

                {/* Auto Translate */}
                <div className="project-pipeline-item">
                  <div className="project-pipeline-item-header">
                    <Switch
                      checked={Boolean(pipeline.autoTranslate)}
                      onChange={(checked) => updatePipeline({ autoTranslate: checked })}
                      label={t('automation.auto_translate', { defaultValue: 'Auto Translate' })}
                    />
                  </div>
                  {pipeline.autoTranslate && (
                    <div className="project-pipeline-item-content">
                      <Dropdown
                        value={pipeline.targetLanguage || 'en'}
                        onChange={(value) => updatePipeline({ targetLanguage: value })}
                        options={languageOptions}
                        style={{ width: '100%' }}
                        aria-label={t('automation.auto_translate', { defaultValue: 'Auto Translate' })}
                      />
                    </div>
                  )}
                </div>

                {/* Auto Summary */}
                <div className="project-pipeline-item">
                  <div className="project-pipeline-item-header">
                    <Switch
                      checked={Boolean(pipeline.autoSummary)}
                      onChange={(checked) => updatePipeline({ autoSummary: checked })}
                      label={t('automation.auto_summary', { defaultValue: 'Auto Summary' })}
                    />
                  </div>
                  {pipeline.autoSummary && (
                    <div className="project-pipeline-item-content">
                      <Dropdown
                        value={pipeline.summaryTemplateId || 'general'}
                        onChange={(value) => updatePipeline({ summaryTemplateId: value })}
                        options={summaryTemplateOptions}
                        style={{ width: '100%' }}
                        aria-label={t('automation.auto_summary', { defaultValue: 'Auto Summary' })}
                      />
                    </div>
                  )}
                </div>

                {/* Auto Export */}
                <div className="project-pipeline-item">
                  <div className="project-pipeline-item-header">
                    <Switch
                      checked={Boolean(pipeline.autoExport)}
                      onChange={(checked) => updatePipeline({ autoExport: checked })}
                      label={t('automation.auto_export', { defaultValue: 'Auto Export' })}
                    />
                  </div>
                  {pipeline.autoExport && (
                    <div className="project-pipeline-item-content" style={{ flexDirection: 'column', gap: 6 }}>
                      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                        <div style={{ width: 110, flexShrink: 0 }}>
                          <Dropdown
                            value={pipeline.exportFormat || 'txt'}
                            onChange={(value) => updatePipeline({ exportFormat: value as ProjectPipelineConfig['exportFormat'] })}
                            options={EXPORT_FORMAT_OPTIONS}
                            style={{ width: '100%' }}
                            aria-label={t('automation.export_format', { defaultValue: 'Export Format' })}
                          />
                        </div>
                        <input
                          type="text"
                          className="settings-input"
                          value={pipeline.exportDirectory || ''}
                          onChange={(e) => updatePipeline({ exportDirectory: e.target.value })}
                          placeholder={t('automation.export_directory_placeholder', { defaultValue: 'Export directory' })}
                          style={{ flex: 1 }}
                        />
                        <button
                          type="button"
                          className="btn btn-secondary"
                          onClick={() => void handleBrowseExportDir()}
                          title={t('settings.browse', { defaultValue: 'Browse' })}
                          style={{ height: 32, padding: '0 10px', display: 'inline-flex', alignItems: 'center', gap: 4 }}
                        >
                          <FolderOpen size={14} />
                        </button>
                      </div>
                    </div>
                  )}
                </div>

                {/* Hotword Sets */}
                {hotwordSets.length > 0 && (
                  <div className="project-pipeline-item">
                    <span style={{ fontSize: '0.8125rem', fontWeight: 500, color: 'var(--color-text-secondary)' }}>
                      {t('projects.pipeline_hotwords', { defaultValue: 'Hotword Sets' })}
                    </span>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '4px' }}>
                      {hotwordSets.map((set) => (
                        <label key={set.id} style={{ display: 'inline-flex', alignItems: 'center', gap: '6px', fontSize: '0.8125rem', cursor: 'pointer' }}>
                          <input
                            type="checkbox"
                            checked={Boolean(pipeline.hotwordSetIds?.includes(set.id))}
                            onChange={(e) => {
                              const currentIds = pipeline.hotwordSetIds || [];
                              const nextIds = e.target.checked
                                ? [...currentIds, set.id]
                                : currentIds.filter((id) => id !== set.id);
                              updatePipeline({ hotwordSetIds: nextIds });
                            }}
                          />
                          <span>{set.name}</span>
                        </label>
                      ))}
                    </div>
                  </div>
                )}

                {/* Text Replacement Sets */}
                {replacementSets.length > 0 && (
                  <div className="project-pipeline-item">
                    <span style={{ fontSize: '0.8125rem', fontWeight: 500, color: 'var(--color-text-secondary)' }}>
                      {t('projects.pipeline_replacements', { defaultValue: 'Text Replacement Sets' })}
                    </span>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '4px' }}>
                      {replacementSets.map((set) => (
                        <label key={set.id} style={{ display: 'inline-flex', alignItems: 'center', gap: '6px', fontSize: '0.8125rem', cursor: 'pointer' }}>
                          <input
                            type="checkbox"
                            checked={Boolean(pipeline.replacementSetIds?.includes(set.id))}
                            onChange={(e) => {
                              const currentIds = pipeline.replacementSetIds || [];
                              const nextIds = e.target.checked
                                ? [...currentIds, set.id]
                                : currentIds.filter((id) => id !== set.id);
                              updatePipeline({ replacementSetIds: nextIds });
                            }}
                          />
                          <span>{set.name}</span>
                        </label>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}
