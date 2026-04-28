import React, { useMemo, useState } from 'react';
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  KeyboardSensor,
  PointerSensor,
  closestCorners,
  defaultDropAnimationSideEffects,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { restrictToVerticalAxis, restrictToWindowEdges } from '@dnd-kit/modifiers';
import type { ProjectRecord } from '../../types/project';
import { PlusCircleIcon } from '../Icons';
import { ALL_ITEMS_SCOPE, INBOX_SCOPE } from './constants';
import type { ProjectBrowseScope, TranslationFn } from './types';
import { RailItemContent, renderScopeIcon } from './utils';

interface ProjectsRailProps {
  browseProjectId: string | null;
  historyItemsCount: number;
  inboxCount: number;
  isAllItemsScope: boolean;
  isInboxScope: boolean;
  itemCounts: Map<string | null, number>;
  onOpenCreateModal: () => void;
  onReorderProjects: (projectIds: string[]) => Promise<void>;
  onSwitchScope: (scope: ProjectBrowseScope) => Promise<void>;
  projects: ProjectRecord[];
  t: TranslationFn;
}

interface SortableProjectItemProps {
  isActive: boolean;
  onSwitchScope: (id: string) => Promise<void>;
  project: ProjectRecord;
  projectCount: number;
  t: TranslationFn;
}

function SortableProjectItem({
  isActive,
  onSwitchScope,
  project,
  projectCount,
  t,
}: SortableProjectItemProps): React.JSX.Element {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: project.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 100 : undefined,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`projects-rail-item-container ${isDragging ? 'is-dragging' : ''}`}
      {...attributes}
      {...listeners}
    >
      <button
        type="button"
        className={`projects-rail-item ${isActive ? 'active' : ''}`}
        onClick={() => void onSwitchScope(project.id)}
        aria-pressed={isActive}
      >
        <RailItemContent
          icon={renderScopeIcon(project.id, project)}
          title={project.name}
          description={project.description || t('projects.items_title', {
            count: projectCount,
            defaultValue: `${projectCount} items`,
          })}
        />
        <span className="projects-rail-count">{projectCount}</span>
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
  itemCounts,
  onOpenCreateModal,
  onReorderProjects,
  onSwitchScope,
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
    }),
  );

  const activeDragProject = useMemo(
    () => (activeId ? projects.find((project) => project.id === activeId) || null : null),
    [activeId, projects],
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
          className={`projects-rail-item ${isAllItemsScope ? 'active' : ''}`}
          onClick={() => void onSwitchScope(ALL_ITEMS_SCOPE)}
          aria-pressed={isAllItemsScope}
        >
          <RailItemContent
            icon={renderScopeIcon(ALL_ITEMS_SCOPE)}
            title={t('projects.all_items', { defaultValue: 'All Items' })}
          />
          <span className="projects-rail-count">{historyItemsCount}</span>
        </button>

        <button
          type="button"
          className={`projects-rail-item ${isInboxScope ? 'active' : ''}`}
          onClick={() => void onSwitchScope(INBOX_SCOPE)}
          aria-pressed={isInboxScope}
        >
          <RailItemContent
            icon={renderScopeIcon(INBOX_SCOPE)}
            title={t('projects.inbox', { defaultValue: 'Inbox' })}
          />
          <span className="projects-rail-count">{inboxCount}</span>
        </button>
      </div>

      <div className="projects-rail-projects">
        <div className="projects-rail-list">
          {projects.length === 0 && (
            <div className="projects-rail-empty">
              {t('projects.no_projects', { defaultValue: 'No projects yet.' })}
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
                  >
                    <RailItemContent
                      icon={renderScopeIcon(activeId, activeDragProject)}
                      title={activeDragProject?.name || ''}
                      description={activeDragProject?.description || t('projects.items_title', {
                        count: itemCounts.get(activeId) || 0,
                        defaultValue: `${itemCounts.get(activeId) || 0} items`,
                      })}
                    />
                    <span className="projects-rail-count">{itemCounts.get(activeId) || 0}</span>
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
