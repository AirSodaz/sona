import { Trash2 } from 'lucide-react';
import type React from 'react';
import type { ProjectSummaryChip, TranslationFn } from './types';

interface ProjectsHeaderProps {
  headerDescription: string;
  headerIcon: React.ReactNode;
  headerTitle: string;
  isScrolled: boolean;
  isTrashScope?: boolean;
  onEmptyTrash?: () => void;
  trashItemCount?: number;
  summaryChips: ProjectSummaryChip[];
  t: TranslationFn;
}

export function ProjectsHeader({
  headerDescription,
  headerIcon,
  headerTitle,
  isScrolled,
  isTrashScope = false,
  onEmptyTrash,
  trashItemCount = 0,
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
        {isTrashScope && onEmptyTrash && (
          <div className="projects-main-entry-actions" data-testid="projects-main-entry-actions">
            <button
              type="button"
              className="btn btn-icon projects-header-icon btn-danger"
              onClick={onEmptyTrash}
              disabled={trashItemCount === 0}
              aria-label={t('history.empty_trash', { defaultValue: 'Empty Trash' })}
              data-tooltip={t('history.empty_trash', { defaultValue: 'Empty Trash' })}
              data-tooltip-pos="bottom-left"
            >
              <Trash2 size={16} />
            </button>
          </div>
        )}
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
