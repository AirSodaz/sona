import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { HistoryItem } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import { Modal } from '../Modal';

interface ProjectAssignmentModalProps {
  isOpen: boolean;
  items: HistoryItem[];
  projects: ProjectRecord[];
  onClose: () => void;
  onApply: (projectId: string | null) => Promise<void>;
}

export function ProjectAssignmentModal({
  isOpen,
  items,
  projects,
  onClose,
  onApply,
}: ProjectAssignmentModalProps): React.JSX.Element | null {
  if (!isOpen) {
    return null;
  }

  const assignmentKey = JSON.stringify(items.map((item) => [item.id, item.projectId]));
  return (
    <OpenProjectAssignmentModal
      key={assignmentKey}
      items={items}
      projects={projects}
      onClose={onClose}
      onApply={onApply}
    />
  );
}

function OpenProjectAssignmentModal({
  items,
  projects,
  onClose,
  onApply,
}: Omit<ProjectAssignmentModalProps, 'isOpen'>): React.JSX.Element {
  const { t } = useTranslation();
  const [projectId, setProjectId] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const handleApply = async () => {
    setIsSaving(true);
    try {
      await onApply(projectId);
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Modal
      isOpen
      onClose={onClose}
      title={t('projects.assign_project', { defaultValue: 'Assign Project' })}
      size="sm"
      footer={
        <>
          <button type="button" className="btn btn-secondary" onClick={onClose} disabled={isSaving}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => void handleApply()} disabled={isSaving}>
            {t('common.save', { defaultValue: 'Save' })}
          </button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
        <span>{items.length} {t('projects.items_selected', { defaultValue: 'items selected' })}</span>
        <select value={projectId ?? ''} onChange={(e) => setProjectId(e.target.value || null)}>
          <option value="">{t('projects.inbox', { defaultValue: 'Inbox' })}</option>
          {projects.map((project) => {
          return (
            <option key={project.id} value={project.id}>{project.name}</option>
          );
          })}
        </select>
      </div>
    </Modal>
  );
}
