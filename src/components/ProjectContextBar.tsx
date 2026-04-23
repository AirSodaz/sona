import React from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';

export function ProjectContextBar(): React.JSX.Element | null {
  const { t } = useTranslation();
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => state.projects.find((item) => item.id === state.activeProjectId) || null);
  const setMode = useTranscriptStore((state) => state.setMode);

  if (!activeProjectId || !activeProject) {
    return null;
  }

  return (
    <div className="project-context-bar">
      <div className="project-context-copy">
        <div className="project-context-label">
          {t('projects.context_label', { defaultValue: 'Current Project' })}
        </div>
        <div className="project-context-name">
          {activeProject.name}
        </div>
      </div>

      <button
        type="button"
        className="btn btn-secondary project-context-action"
        onClick={() => setMode('projects')}
      >
        {t('projects.open_projects', { defaultValue: 'Open Workspace' })}
      </button>
    </div>
  );
}

export default ProjectContextBar;
