import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '../Modal';

interface ProjectCreateModalProps {
  isOpen: boolean;
  name: string;
  description: string;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onClose: () => void;
  onCreate: () => void;
}

export function ProjectCreateModal({
  isOpen,
  name,
  description,
  onNameChange,
  onDescriptionChange,
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
      title={t('projects.new_project_title', { defaultValue: 'New Project' })}
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
            {t('projects.create_action', { defaultValue: 'Create Project' })}
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
            {t('projects.project_name', { defaultValue: 'Project Name' })}
          </label>
          <input
            id="project-create-name"
            type="text"
            className="settings-input"
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            placeholder={t('projects.new_project_name', { defaultValue: 'Project name' })}
            autoFocus
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
