import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { HistoryItem } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import { Modal } from '../Modal';
import { Dropdown, type DropdownOption } from '../Dropdown';

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

  const projectOptions = useMemo<DropdownOption[]>(() => [
    { value: '', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
    ...projects.map((project) => ({ value: project.id, label: project.name })),
  ], [projects, t]);
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
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <div style={{ color: 'var(--color-text-secondary)', fontSize: '0.875rem' }}>
          {t('projects.selected_count', { count: items.length, defaultValue: '{{count}} selected' })}
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
          <label style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
            {t('projects.target_project', { defaultValue: 'Target Project' })}
          </label>
          <Dropdown
            value={projectId ?? ''}
            onChange={(val) => setProjectId(val || null)}
            options={projectOptions}
            style={{ width: '100%' }}
            aria-label={t('projects.target_project', { defaultValue: 'Target Project' })}
          />
        </div>
      </div>
    </Modal>
  );
}
