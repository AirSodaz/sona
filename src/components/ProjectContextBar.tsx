import React from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectStore } from '../stores/projectStore';

export function ProjectContextBar(): React.JSX.Element | null {
  const { t } = useTranslation();
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => state.projects.find((item) => item.id === state.activeProjectId) || null);
  const setActiveProjectId = useProjectStore((state) => state.setActiveProjectId);

  if (!activeProjectId || !activeProject) {
    return null;
  }

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: 'var(--spacing-md)',
        padding: 'var(--spacing-sm) var(--spacing-md)',
        borderBottom: '1px solid var(--color-border)',
        background: 'var(--color-bg-secondary)',
      }}
    >
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
          {t('projects.context_label', { defaultValue: 'Current Project' })}
        </div>
        <div
          style={{
            fontWeight: 600,
            color: 'var(--color-text-primary)',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
        >
          {activeProject.name}
        </div>
      </div>

      <button
        type="button"
        className="btn btn-secondary"
        onClick={() => void setActiveProjectId(null)}
      >
        {t('projects.exit_to_inbox', { defaultValue: 'Exit to Inbox' })}
      </button>
    </div>
  );
}

export default ProjectContextBar;
