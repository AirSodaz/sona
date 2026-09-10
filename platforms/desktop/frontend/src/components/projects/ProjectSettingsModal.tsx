import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ProjectRecord } from '../../types/project';
import { FolderIcon } from '../Icons';
import { IconPicker } from '../IconPicker';
import { Modal } from '../Modal';

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
  const { t } = useTranslation();

  if (!isOpen || !project) {
    return null;
  }

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
              defaultValue: 'Configure project details and its processing pipeline.',
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
        {draftPipeline && onPipelineChange && (
          <div className="projects-field">
            <label>{t('projects.pipeline', { defaultValue: 'Pipeline' })}</label>
            {(['autoPolish', 'autoTranslate', 'autoSummary', 'autoExport'] as const).map((key) => (
              <label key={key} style={{ display: 'flex', gap: 8 }}>
                <input type="checkbox" checked={Boolean(draftPipeline[key])} onChange={(e) => onPipelineChange({ ...draftPipeline, [key]: e.target.checked })} />
                {key === 'autoPolish' ? 'Polish' : key === 'autoTranslate' ? 'Translate' : key === 'autoSummary' ? 'Summary' : 'Auto export'}
              </label>
            ))}
          </div>
        )}
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
          <label htmlFor="project-settings-color">
            {t('projects.tag_color', { defaultValue: 'Color' })}
          </label>
          <input id="project-settings-color" type="color" value={draftColor} onChange={(event) => onColorChange(event.target.value)} />
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
            style={{ minHeight: '80px' }}
          />
        </div>
      </div>
    </Modal>
  );
}
