import React from 'react';
import { FileTextIcon, MicIcon, SettingsIcon } from '../Icons';
import type { ProjectRecord } from '../../types/project';
import type { ProjectSummaryChip, TranslationFn } from './types';

interface ProjectsHeaderProps {
  browseProject: ProjectRecord | null;
  headerDescription: string;
  headerIcon: React.ReactNode;
  headerTitle: string;
  isScrolled: boolean;
  onOpenBatchImport: () => void;
  onOpenProjectSettings: () => void;
  onStartLiveRecord: () => void;
  showWorkflowActions: boolean;
  summaryChips: ProjectSummaryChip[];
  t: TranslationFn;
}

export function ProjectsHeader({
  browseProject,
  headerDescription,
  headerIcon,
  headerTitle,
  isScrolled,
  onOpenBatchImport,
  onOpenProjectSettings,
  onStartLiveRecord,
  showWorkflowActions,
  summaryChips,
  t,
}: ProjectsHeaderProps): React.JSX.Element {
  return (
    <div className={`projects-main-header ${isScrolled ? 'is-scrolled' : ''}`}>
      <div className="projects-main-header-top">
        <div className="projects-main-heading">
          <div className="projects-main-title-row">
            <span className="projects-main-title-icon" aria-hidden="true">
              {headerIcon}
            </span>
            <h3>{headerTitle}</h3>
          </div>
          <p>{headerDescription}</p>
        </div>
        <div className="projects-main-entry-actions" data-testid="projects-main-entry-actions">
          {showWorkflowActions && (
            <>
              <button
                type="button"
                className="btn btn-icon projects-header-icon"
                onClick={onStartLiveRecord}
                aria-label={t('projects.start_live_record', { defaultValue: 'Start Live Record' })}
                data-tooltip={t('projects.start_live_record', { defaultValue: 'Start Live Record' })}
                data-tooltip-pos="bottom"
              >
                <MicIcon width={16} height={16} />
              </button>
              <button
                type="button"
                className="btn btn-icon projects-header-icon"
                onClick={onOpenBatchImport}
                aria-label={t('projects.open_batch_import', { defaultValue: 'Open Batch Import' })}
                data-tooltip={t('projects.open_batch_import', { defaultValue: 'Open Batch Import' })}
                data-tooltip-pos="bottom"
              >
                <FileTextIcon width={16} height={16} />
              </button>
            </>
          )}
          {browseProject && (
            <button
              type="button"
              className="btn btn-icon projects-header-icon"
              onClick={onOpenProjectSettings}
              aria-label={t('projects.project_settings', { defaultValue: 'Project Settings' })}
              data-tooltip={t('projects.project_settings', { defaultValue: 'Project Settings' })}
              data-tooltip-pos="bottom-left"
            >
              <SettingsIcon width={16} height={16} />
            </button>
          )}
        </div>
      </div>

      <div className="projects-stats-row" data-testid="projects-summary-chips">
        {summaryChips.map((chip) => (
          <div key={chip.key} className="projects-stat-card">
            <span className="projects-stat-label">{chip.label}</span>
            <strong className="projects-stat-value" data-testid={chip.testId}>
              {chip.value}
            </strong>
          </div>
        ))}
      </div>
    </div>
  );
}
