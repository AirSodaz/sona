import { FolderKanban, ListChecks, RotateCcw, Trash2, X } from 'lucide-react';
import type React from 'react';
import { IconButton } from '../ui/Button';
import type { TranslationFn } from './types';

interface ProjectsSelectionBarProps {
  isTrashScope: boolean;
  onCancel: () => void;
  onDeleteSelected: () => void;
  onAssignProject: () => void;
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
  onAssignProject,
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
        <IconButton
          tier="ghost"
          size="md"
          active={selectedIds.length === totalVisibleItems && totalVisibleItems > 0}
          className="projects-toolbar-icon"
          icon={<ListChecks size={16} />}
          onClick={onToggleSelectAll}
          aria-label={t('common.select_all', { defaultValue: 'Select All' })}
          data-tooltip={t('common.select_all', { defaultValue: 'Select All' })}
          data-tooltip-pos="top"
        />
        <div className="projects-toolbar-divider" />
        <IconButton
          tier="ghost"
          size="md"
          className="projects-toolbar-icon"
          icon={isTrashScope ? <RotateCcw size={16} /> : <FolderKanban size={16} />}
          onClick={isTrashScope ? onRestoreSelected : onAssignProject}
          disabled={selectedIds.length === 0}
          aria-label={
            isTrashScope
              ? t('history.restore', { defaultValue: 'Restore' })
              : t('projects.assign_project', { defaultValue: 'Assign Project' })
          }
          data-tooltip={
            isTrashScope
              ? t('history.restore', { defaultValue: 'Restore' })
              : t('projects.assign_project', { defaultValue: 'Assign Project' })
          }
          data-tooltip-pos="top"
        />
        <IconButton
          tier="ghost"
          tone="danger"
          size="md"
          className="projects-toolbar-icon"
          icon={<Trash2 size={16} />}
          onClick={onDeleteSelected}
          disabled={selectedIds.length === 0}
          aria-label={t('common.delete', { defaultValue: 'Delete' })}
          data-tooltip={t('common.delete', { defaultValue: 'Delete' })}
          data-tooltip-pos="top"
        />
        <IconButton
          tier="ghost"
          size="md"
          className="projects-toolbar-icon"
          icon={<X size={16} />}
          onClick={onCancel}
          aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
          data-tooltip={t('common.cancel', { defaultValue: 'Cancel' })}
          data-tooltip-pos="top"
        />
      </div>
    </div>
  );
}
