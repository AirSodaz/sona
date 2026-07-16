import React from 'react';
import { ListChecks, RotateCcw, Tags, Trash2, X } from 'lucide-react';
import type { TranslationFn } from './types';

interface ProjectsSelectionBarProps {
  isTrashScope: boolean;
  onCancel: () => void;
  onDeleteSelected: () => void;
  onEditTags: () => void;
  onRestoreSelected: () => void;
  onToggleSelectAll: () => void;
  selectedIds: string[];
  totalVisibleItems: number;
  t: TranslationFn;
}

export function ProjectsSelectionBar({
  isTrashScope,
  onCancel,
  onDeleteSelected,
  onEditTags,
  onRestoreSelected,
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
        <button
          type="button"
          className="btn btn-icon projects-toolbar-icon"
          onClick={isTrashScope ? onRestoreSelected : onEditTags}
          disabled={selectedIds.length === 0}
          aria-label={isTrashScope
            ? t('history.restore', { defaultValue: 'Restore' })
            : t('projects.edit_tags', { defaultValue: 'Edit Tags' })}
          data-tooltip={isTrashScope
            ? t('history.restore', { defaultValue: 'Restore' })
            : t('projects.edit_tags', { defaultValue: 'Edit Tags' })}
          data-tooltip-pos="top"
        >
          {isTrashScope ? <RotateCcw size={16} /> : <Tags size={16} />}
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
