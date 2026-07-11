import React from 'react';
import { ArrowRight, ListChecks, Trash2, X } from 'lucide-react';
import { Dropdown } from '../Dropdown';
import type { TranslationFn } from './types';

interface ProjectsSelectionBarProps {
  currentScopeMoveTarget: string | null;
  moveOptions: Array<{ value: string; label: string }>;
  moveTarget: string;
  onCancel: () => void;
  onDeleteSelected: () => void;
  onMoveSelected: () => void;
  onMoveTargetChange: (value: string) => void;
  onToggleSelectAll: () => void;
  selectedIds: string[];
  totalVisibleItems: number;
  t: TranslationFn;
}

export function ProjectsSelectionBar({
  currentScopeMoveTarget,
  moveOptions,
  moveTarget,
  onCancel,
  onDeleteSelected,
  onMoveSelected,
  onMoveTargetChange,
  onToggleSelectAll,
  selectedIds,
  totalVisibleItems,
  t,
}: ProjectsSelectionBarProps): React.JSX.Element {
  return (
    <div className="projects-fab" data-testid="projects-fab">
      <div className="projects-selection-copy">
        {t('projects.selected_count', {
          count: selectedIds.length,
          defaultValue: `${selectedIds.length} selected`,
        })}
      </div>
      <div className="projects-fab-actions">
        <button
          type="button"
          className={`btn btn-icon projects-toolbar-icon ${selectedIds.length === totalVisibleItems && totalVisibleItems > 0 ? 'active' : ''}`}
          onClick={onToggleSelectAll}
          aria-label={t('common.select_all', { defaultValue: 'Select All' })}
          data-tooltip={t('common.select_all', { defaultValue: 'Select All' })}
          data-tooltip-pos="top"
        >
          <ListChecks size={16} />
        </button>
        <div className="projects-toolbar-divider" />
        <Dropdown
          value={moveTarget}
          onChange={onMoveTargetChange}
          options={moveOptions}
          style={{ width: '200px' }}
          aria-label={t('projects.move_target', { defaultValue: 'Move target' })}
        />
        <button
          type="button"
          className="btn btn-icon projects-toolbar-icon"
          onClick={onMoveSelected}
          disabled={selectedIds.length === 0 || (currentScopeMoveTarget !== null && moveTarget === currentScopeMoveTarget)}
          aria-label={t('projects.move_selected', { defaultValue: 'Move Selected' })}
          data-tooltip={t('projects.move_selected', { defaultValue: 'Move Selected' })}
          data-tooltip-pos="top"
        >
          <ArrowRight size={16} />
        </button>
        <button
          type="button"
          className="btn btn-icon projects-toolbar-icon btn-danger"
          onClick={onDeleteSelected}
          disabled={selectedIds.length === 0}
          aria-label={t('common.delete', { defaultValue: 'Delete' })}
          data-tooltip={t('common.delete', { defaultValue: 'Delete' })}
          data-tooltip-pos="top"
        >
          <Trash2 size={16} />
        </button>
        <button
          type="button"
          className="btn btn-icon projects-toolbar-icon"
          onClick={onCancel}
          aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
          data-tooltip={t('common.cancel', { defaultValue: 'Cancel' })}
          data-tooltip-pos="top"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
}
