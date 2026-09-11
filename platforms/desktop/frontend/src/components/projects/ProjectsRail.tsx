import {
  closestCorners,
  DndContext,
  type DragEndEvent,
  DragOverlay,
  type DragStartEvent,
  defaultDropAnimationSideEffects,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import { restrictToVerticalAxis, restrictToWindowEdges } from '@dnd-kit/modifiers';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { MoreHorizontal, Zap } from 'lucide-react';
import type React from 'react';
import { useMemo, useState } from 'react';
import type { ProjectRecord } from '../../types/project';
import {
  type ContextMenuOpenRequest,
  createKeyboardContextMenuRequest,
  createPointerContextMenuRequest,
  isContextMenuKeyboardEvent,
} from '../context-menu/trigger';
import { PlusCircleIcon } from '../Icons';
import { ALL_ITEMS_SCOPE, TRASH_SCOPE, UNTAGGED_SCOPE } from './constants';
import type { ProjectBrowseScope, TranslationFn } from './types';
import { RailItemContent, renderScopeIcon } from './utils';

interface ProjectsRailProps {
  browseProjectId: string | null;
  historyItemsCount: number;
  inboxCount: number;
  isAllItemsScope: boolean;
  isInboxScope: boolean;
  isTrashScope: boolean;
  trashCount: number;
  itemCounts: Map<string | null, number>;
  onOpenCreateModal: () => void;
  onReorderProjects: (projectIds: string[]) => Promise<void>;
  onSwitchScope: (scope: ProjectBrowseScope) => Promise<boolean>;
  onOpenProjectContextMenu: (id: string, request: ContextMenuOpenRequest) => void;
  activeContextId: string | null;
  projects: ProjectRecord[];
  t: TranslationFn;
}

interface SortableProjectItemProps {
  isActive: boolean;
  onSwitchScope: (id: string) => Promise<boolean>;
  project: ProjectRecord;
  projectCount: number;
  onOpenContextMenu: (id: string, request: ContextMenuOpenRequest) => void;
  isContextMenuOpen: boolean;
  t: TranslationFn;
}

function SortableProjectItem({
  isActive,
  onSwitchScope,
  project,
  projectCount,
  onOpenContextMenu,
  isContextMenuOpen,
  t,
}: SortableProjectItemProps): React.JSX.Element {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: project.id,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 100 : undefined,
  };

  const handleContextMenuKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (isContextMenuKeyboardEvent(event)) {
      event.preventDefault();
      event.stopPropagation();
      onOpenContextMenu(project.id, createKeyboardContextMenuRequest(event.currentTarget));
      return;
    }

    listeners?.onKeyDown?.(event);
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`projects-rail-item-container ${isDragging ? 'is-dragging' : ''}`}
    >
      <button
        type="button"
        className={`projects-rail-item ${isActive ? 'active' : ''} ${isContextMenuOpen ? 'context-menu-active' : ''}`}
        style={
          project.color
            ? ({ '--item-accent-color': project.color } as React.CSSProperties)
            : undefined
        }
        onClick={() => void onSwitchScope(project.id)}
        {...attributes}
        {...listeners}
        onKeyDown={handleContextMenuKeyDown}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onOpenContextMenu(project.id, createPointerContextMenuRequest(event));
        }}
        aria-pressed={isActive}
        title={project.description ? `${project.name}\n${project.description}` : project.name}
      >
        <RailItemContent icon={renderScopeIcon(project.id, project)} title={project.name} />
        <div className="projects-rail-actions">
          {project.pipeline?.enabled && (
            <span
              className="projects-rail-pipeline-badge"
              title={t('projects.pipeline_enabled', { defaultValue: '流水线已启用' })}
            >
              <Zap size={13} />
            </span>
          )}
          <span className="projects-rail-count">{projectCount}</span>
          <button
            type="button"
            className="projects-rail-menu-btn"
            aria-label={t('common.more_options', { defaultValue: 'More options' })}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onOpenContextMenu(project.id, createPointerContextMenuRequest(event));
            }}
          >
            <MoreHorizontal size={14} />
          </button>
        </div>
      </button>
    </div>
  );
}

export function ProjectsRail({
  browseProjectId,
  historyItemsCount,
  inboxCount,
  isAllItemsScope,
  isInboxScope,
  isTrashScope,
  trashCount,
  itemCounts,
  onOpenCreateModal,
  onReorderProjects,
  onSwitchScope,
  onOpenProjectContextMenu,
  activeContextId,
  projects,
  t,
}: ProjectsRailProps): React.JSX.Element {
  const [activeId, setActiveId] = useState<string | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  const activeDragProject = useMemo(
    () => (activeId ? projects.find((project) => project.id === activeId) || null : null),
    [activeId, projects]
  );

  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(event.active.id as string);
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveId(null);

    if (!over || active.id === over.id) {
      return;
    }

    const oldIndex = projects.findIndex((project) => project.id === active.id);
    const newIndex = projects.findIndex((project) => project.id === over.id);
    if (oldIndex === -1 || newIndex === -1) {
      return;
    }

    const newOrder = arrayMove(projects, oldIndex, newIndex);
    await onReorderProjects(newOrder.map((project) => project.id));
  };

  return (
    <aside className="projects-rail">
      <div className="projects-rail-header">
        <div className="projects-rail-title-row">
          <div className="projects-rail-eyebrow">
            {t('panel.projects', { defaultValue: 'Workspace' })}
          </div>
          <button
            type="button"
            className="btn btn-icon projects-rail-create"
            onClick={onOpenCreateModal}
            aria-label={t('projects.new_project_button', { defaultValue: 'New Project' })}
            data-tooltip={t('projects.new_project_button', { defaultValue: 'New Project' })}
            data-tooltip-pos="bottom"
          >
            <PlusCircleIcon width={18} height={18} />
          </button>
        </div>
      </div>

      <div className="projects-rail-scopes">
        <button
          type="button"
          className={`projects-rail-item ${isAllItemsScope ? 'active' : ''} ${activeContextId === 'workspace:scope:all' ? 'context-menu-active' : ''}`}
          onClick={() => void onSwitchScope(ALL_ITEMS_SCOPE)}
          onContextMenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onOpenProjectContextMenu(ALL_ITEMS_SCOPE, createPointerContextMenuRequest(event));
          }}
          aria-pressed={isAllItemsScope}
        >
          <RailItemContent
            icon={renderScopeIcon(ALL_ITEMS_SCOPE)}
            title={t('projects.all_items', { defaultValue: 'All Items' })}
          />
          <div className="projects-rail-actions">
            <span className="projects-rail-count">{historyItemsCount}</span>
          </div>
        </button>

        <button
          type="button"
          className={`projects-rail-item ${isTrashScope ? 'active' : ''} ${activeContextId === 'workspace:scope:trash' ? 'context-menu-active' : ''}`}
          onClick={() => void onSwitchScope(TRASH_SCOPE)}
          onContextMenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onOpenProjectContextMenu(TRASH_SCOPE, createPointerContextMenuRequest(event));
          }}
          aria-pressed={isTrashScope}
        >
          <RailItemContent
            icon={renderScopeIcon(TRASH_SCOPE)}
            title={t('projects.trash', { defaultValue: 'Trash' })}
          />
          <div className="projects-rail-actions">
            <span className="projects-rail-count">{trashCount}</span>
          </div>
        </button>
      </div>

      <div className="projects-rail-projects">
        <div
          className="projects-rail-list"
          onContextMenu={(event) => {
            if (event.target === event.currentTarget) {
              event.preventDefault();
              onOpenProjectContextMenu('rail_empty', createPointerContextMenuRequest(event));
            }
          }}
        >
          <button
            type="button"
            className={`projects-rail-item ${isInboxScope ? 'active' : ''} ${activeContextId === 'workspace:scope:untagged' ? 'context-menu-active' : ''}`}
            onClick={() => void onSwitchScope(UNTAGGED_SCOPE)}
            onContextMenu={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onOpenProjectContextMenu(UNTAGGED_SCOPE, createPointerContextMenuRequest(event));
            }}
            aria-pressed={isInboxScope}
          >
            <RailItemContent
              icon={renderScopeIcon(UNTAGGED_SCOPE)}
              title={t('projects.inbox', { defaultValue: 'Inbox' })}
            />
            <div className="projects-rail-actions">
              <span className="projects-rail-count">{inboxCount}</span>
            </div>
          </button>

          {projects.length === 0 && (
            <div className="projects-rail-empty">
              {t('projects.no_tags', { defaultValue: 'No tags yet.' })}
            </div>
          )}

          <DndContext
            sensors={sensors}
            collisionDetection={closestCorners}
            onDragStart={handleDragStart}
            onDragEnd={handleDragEnd}
            modifiers={[restrictToVerticalAxis, restrictToWindowEdges]}
          >
            <SortableContext
              items={projects.map((project) => project.id)}
              strategy={verticalListSortingStrategy}
            >
              {projects.map((project) => (
                <SortableProjectItem
                  key={project.id}
                  project={project}
                  projectCount={itemCounts.get(project.id) || 0}
                  isActive={browseProjectId === project.id}
                  onSwitchScope={onSwitchScope}
                  onOpenContextMenu={onOpenProjectContextMenu}
                  isContextMenuOpen={activeContextId === `workspace:project:${project.id}`}
                  t={t}
                />
              ))}
            </SortableContext>
            <DragOverlay
              dropAnimation={{
                sideEffects: defaultDropAnimationSideEffects({
                  styles: {
                    active: {
                      opacity: '0.4',
                    },
                  },
                }),
              }}
            >
              {activeId ? (
                <div className="projects-rail-item-container is-dragging-overlay">
                  <button
                    type="button"
                    className={`projects-rail-item ${browseProjectId === activeId ? 'active' : ''}`}
                    style={
                      activeDragProject?.color
                        ? ({
                            '--item-accent-color': activeDragProject.color,
                          } as React.CSSProperties)
                        : undefined
                    }
                  >
                    <RailItemContent
                      icon={renderScopeIcon(activeId, activeDragProject)}
                      title={activeDragProject?.name || ''}
                    />
                    <div className="projects-rail-actions">
                      <span className="projects-rail-count">{itemCounts.get(activeId) || 0}</span>
                    </div>
                  </button>
                </div>
              ) : null}
            </DragOverlay>
          </DndContext>
        </div>
      </div>
    </aside>
  );
}
