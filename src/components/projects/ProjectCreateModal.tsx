import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { XIcon } from '../Icons';

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
      if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
      } else if (event.key === 'Enter' && name.trim()) {
        event.preventDefault();
        void onCreate();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, name, onClose, onCreate]);

  if (!isOpen) {
    return null;
  }

  return (
    <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
      <div
        className="dialog-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-create-title"
        style={{
          background: 'var(--color-bg-elevated)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: 'var(--shadow-xl)',
          width: '520px',
          maxWidth: '95vw',
          display: 'flex',
          flexDirection: 'column',
          border: '1px solid var(--color-border)',
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'flex-start',
            justifyContent: 'space-between',
            padding: 'var(--spacing-lg)',
            borderBottom: '1px solid var(--color-border)',
          }}
        >
          <div>
            <h3 id="project-create-title" style={{ margin: 0, fontSize: '1.125rem', fontWeight: 600 }}>
              {t('projects.new_project_title', { defaultValue: 'New Project' })}
            </h3>
          </div>
          <button
            type="button"
            className="btn btn-icon"
            onClick={onClose}
            aria-label={t('common.close')}
          >
            <XIcon />
          </button>
        </div>

        <div
          style={{
            padding: 'var(--spacing-lg)',
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

        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'flex-end',
            gap: 'var(--spacing-sm)',
            padding: 'var(--spacing-md) var(--spacing-lg)',
            borderTop: '1px solid var(--color-border)',
            background: 'var(--color-bg-elevated)',
          }}
        >
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
        </div>
      </div>
    </div>
  );
}
