import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '../Modal';

interface ProjectCreateModalProps {
  isOpen: boolean;
  name: string;
  description: string;
  color: string;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onColorChange: (value: string) => void;
  onClose: () => void;
  onCreate: () => void;
}

export function ProjectCreateModal({
  isOpen,
  name,
  description,
  color,
  onNameChange,
  onDescriptionChange,
  onColorChange,
  onClose,
  onCreate,
}: ProjectCreateModalProps): React.JSX.Element | null {
  const { t } = useTranslation();

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Enter' && name.trim()) {
        event.preventDefault();
        void onCreate();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, name, onCreate]);

  if (!isOpen) {
    return null;
  }

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('projects.new_tag_title', { defaultValue: 'New Tag' })}
      size="md"
      footer={
        <>
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void onCreate()}
            disabled={!name.trim()}
          >
            {t('projects.create_tag_action', { defaultValue: 'Create Tag' })}
          </button>
        </>
      }
    >
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--spacing-lg)',
        }}
      >
        <div className="projects-field">
          <label htmlFor="project-create-name">
            {t('projects.tag_name', { defaultValue: 'Tag Name' })}
          </label>
          <input
            id="project-create-name"
            type="text"
            className="settings-input"
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            placeholder={t('projects.new_tag_name', { defaultValue: 'Tag name' })}
            autoFocus
          />
        </div>

        <div className="projects-field">
          <label htmlFor="project-create-color">
            {t('projects.tag_color', { defaultValue: 'Color' })}
          </label>
          <input
            id="project-create-color"
            type="color"
            value={color}
            onChange={(event) => onColorChange(event.target.value)}
            aria-label={t('projects.tag_color', { defaultValue: 'Color' })}
          />
        </div>

        <div className="projects-field">
          <label htmlFor="project-create-description">
            {t('projects.project_description', { defaultValue: 'Description' })}
          </label>
          <textarea
            id="project-create-description"
            className="settings-input"
            value={description}
            onChange={(event) => onDescriptionChange(event.target.value)}
            placeholder={t('projects.new_project_description', { defaultValue: 'Short description' })}
            style={{ minHeight: '100px' }}
          />
        </div>
      </div>
    </Modal>
  );
}
