import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Zap, FolderOpen } from 'lucide-react';
import type { ProjectPipelineConfig, ProjectRecord } from '../../types/project';
import { DEFAULT_PROJECT_PIPELINE } from '../../types/project';
import { FolderIcon } from '../Icons';
import { IconPicker } from '../IconPicker';
import { Modal } from '../Modal';
import { getPolishPresetOptions } from '../../utils/polishPresets';
import { getSummaryTemplateOptions } from '../../utils/summaryTemplates';
import { LANGUAGE_OPTIONS } from '../../constants/languages';
import { getLocalizedLanguageName } from '../../utils/languageUtils';
import { openDialog } from '../../services/tauri/platform/dialog';
import { PROJECT_COLOR_PRESETS } from '../../constants/projects';
export { PROJECT_COLOR_PRESETS };
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
            <IconPicker icon={draftIcon} onChange={onIconChange} defaultIcon={<FolderIcon />} />
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
          <label>
            {t('projects.tag_color', { defaultValue: 'Color' })}
          </label>
          <div className="project-color-swatches">
            {PROJECT_COLOR_PRESETS.map((color) => (
              <button
                key={color}
                type="button"
                className={`project-color-swatch ${draftColor.toLowerCase() === color.toLowerCase() ? 'active' : ''}`}
                style={{ backgroundColor: color }}
                onClick={() => onColorChange(color)}
                title={color}
              />
            ))}
            <input
              id="project-settings-color"
              type="color"
              value={draftColor}
              onChange={(event) => onColorChange(event.target.value)}
              title={t('common.custom_color', { defaultValue: 'Custom color' })}
              style={{ width: 26, height: 26, padding: 0, border: 'none', cursor: 'pointer', borderRadius: 4, background: 'transparent' }}
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
            <label className="project-pipeline-toggle">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                <Zap size={15} style={{ color: 'var(--color-accent, #6366F1)' }} />
                {t('projects.pipeline_enable', { defaultValue: 'Enable Project Pipeline' })}
              </span>
              <input
                type="checkbox"
                checked={Boolean(pipeline.enabled)}
                onChange={(e) => updatePipeline({ enabled: e.target.checked })}
              />
            </label>

            {pipeline.enabled && (
              <div className="project-pipeline-options">
                {/* Auto Polish */}
                <div className="project-pipeline-item">
                  <label className="project-pipeline-item-header">
                    <input
                      type="checkbox"
                      checked={Boolean(pipeline.autoPolish)}
                      onChange={(e) => updatePipeline({ autoPolish: e.target.checked })}
                    />
                    <span>{t('automation.auto_polish', { defaultValue: 'Auto Polish' })}</span>
                  </label>
                  {pipeline.autoPolish && (
                    <div className="project-pipeline-item-content">
                      <select
                        value={pipeline.polishPresetId || 'general'}
                        onChange={(e) => updatePipeline({ polishPresetId: e.target.value })}
                      >
                        {polishPresetOptions.map((opt) => (
                          <option key={opt.value} value={opt.value}>{opt.label}</option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>

                {/* Auto Translate */}
                <div className="project-pipeline-item">
                  <label className="project-pipeline-item-header">
                    <input
                      type="checkbox"
                      checked={Boolean(pipeline.autoTranslate)}
                      onChange={(e) => updatePipeline({ autoTranslate: e.target.checked })}
                    />
                    <span>{t('automation.auto_translate', { defaultValue: 'Auto Translate' })}</span>
                  </label>
                  {pipeline.autoTranslate && (
                    <div className="project-pipeline-item-content">
                      <select
                        value={pipeline.targetLanguage || 'en'}
                        onChange={(e) => updatePipeline({ targetLanguage: e.target.value })}
                      >
                        {languageOptions.map((opt) => (
                          <option key={opt.value} value={opt.value}>{opt.label}</option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>

                {/* Auto Summary */}
                <div className="project-pipeline-item">
                  <label className="project-pipeline-item-header">
                    <input
                      type="checkbox"
                      checked={Boolean(pipeline.autoSummary)}
                      onChange={(e) => updatePipeline({ autoSummary: e.target.checked })}
                    />
                    <span>{t('automation.auto_summary', { defaultValue: 'Auto Summary' })}</span>
                  </label>
                  {pipeline.autoSummary && (
                    <div className="project-pipeline-item-content">
                      <select
                        value={pipeline.summaryTemplateId || 'general'}
                        onChange={(e) => updatePipeline({ summaryTemplateId: e.target.value })}
                      >
                        {summaryTemplateOptions.map((opt) => (
                          <option key={opt.value} value={opt.value}>{opt.label}</option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>

                {/* Auto Export */}
                <div className="project-pipeline-item">
                  <label className="project-pipeline-item-header">
                    <input
                      type="checkbox"
                      checked={Boolean(pipeline.autoExport)}
                      onChange={(e) => updatePipeline({ autoExport: e.target.checked })}
                    />
                    <span>{t('automation.auto_export', { defaultValue: 'Auto Export' })}</span>
                  </label>
                  {pipeline.autoExport && (
                    <div className="project-pipeline-item-content" style={{ flexDirection: 'column', gap: 6 }}>
                      <div style={{ display: 'flex', gap: 8 }}>
                        <select
                          value={pipeline.exportFormat || 'txt'}
                          onChange={(e) => updatePipeline({ exportFormat: e.target.value as ProjectPipelineConfig['exportFormat'] })}
                          style={{ width: 100 }}
                        >
                          {(['txt', 'srt', 'vtt', 'json', 'docx'] as const).map((fmt) => (
                            <option key={fmt} value={fmt}>{fmt.toUpperCase()}</option>
                          ))}
                        </select>
                        <input
                          type="text"
                          value={pipeline.exportDirectory || ''}
                          onChange={(e) => updatePipeline({ exportDirectory: e.target.value })}
                          placeholder={t('automation.export_directory_placeholder', { defaultValue: 'Export directory' })}
                          style={{ flex: 1 }}
                        />
                        <button
                          type="button"
                          className="btn btn-secondary"
                          onClick={() => void handleBrowseExportDir()}
                          title={t('common.browse', { defaultValue: 'Browse' })}
                          style={{ height: 30, padding: '0 8px' }}
                        >
                          <FolderOpen size={14} />
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}
