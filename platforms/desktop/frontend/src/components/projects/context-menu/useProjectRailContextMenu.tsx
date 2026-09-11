import { Check, FolderOpen, Plus, Settings as SettingsIcon, Trash2 } from 'lucide-react';
import React from 'react';
import type { ProjectRecord } from '../../../types/project';
import type { ContextMenuOpenRequest } from '../../context-menu/trigger';
import type { ContextMenuAction, OpenContextMenuOptions } from '../../context-menu/types';
import { ALL_ITEMS_SCOPE, TRASH_SCOPE, UNTAGGED_SCOPE } from '../constants';
import type { ProjectBrowseScope, TranslationFn } from '../types';

export interface UseProjectRailContextMenuParams {
  activeProjectId: string | null;
  browseScope: string;
  isLockedLiveDraft: boolean;
  onDeleteProject: (project: ProjectRecord) => void;
  onEmptyTrash?: () => void;
  onOpenCreateModal?: () => void;
  onOpenProjectSettings: (id: string) => void;
  onSetActiveProjectId: (id: string | null) => void;
  onSwitchScope: (scope: ProjectBrowseScope) => Promise<boolean>;
  openContextMenu: (options: OpenContextMenuOptions) => void;
  projects: ProjectRecord[];
  t: TranslationFn;
  trashCount?: number;
  onMenuOpened?: (contextId: string) => void;
}

export interface ProjectRailContextMenuController {
  openRailContextMenu: (scopeOrProjectId: string, request: ContextMenuOpenRequest) => void;
}

export function useProjectRailContextMenu({
  activeProjectId,
  browseScope,
  isLockedLiveDraft,
  onDeleteProject,
  onEmptyTrash,
  onOpenCreateModal,
  onOpenProjectSettings,
  onSetActiveProjectId,
  onSwitchScope,
  openContextMenu,
  projects,
  t,
  trashCount = 0,
  onMenuOpened,
}: UseProjectRailContextMenuParams): ProjectRailContextMenuController {
  const openRailContextMenu = React.useCallback(
    (scopeOrProjectId: string, request: ContextMenuOpenRequest) => {
      // 1. Custom Project
      const project = projects.find((item) => item.id === scopeOrProjectId);
      if (project) {
        const isCurrentProject = browseScope === project.id;
        const contextId = `workspace:project:${project.id}`;
        onMenuOpened?.(contextId);

        const actions: ContextMenuAction[] = [
          {
            id: 'open',
            label: t('common.open', { defaultValue: 'Open' }),
            icon: <FolderOpen size={16} />,
            shortcut: 'Enter',
            disabled: isCurrentProject || isLockedLiveDraft,
            onSelect: () => {
              void onSwitchScope(project.id);
            },
          },
          {
            id: 'set_active',
            label: t('projects.set_as_active', { defaultValue: '设为当前活跃' }),
            icon: <Check size={16} />,
            disabled: activeProjectId === project.id,
            onSelect: () => {
              onSetActiveProjectId(project.id);
            },
          },
          {
            id: 'settings',
            label: t('projects.tag_settings', { defaultValue: 'Tag Settings' }),
            icon: <SettingsIcon size={16} />,
            dividerBefore: true,
            disabled: isLockedLiveDraft && !isCurrentProject,
            onSelect: () => {
              onOpenProjectSettings(project.id);
            },
          },
          {
            id: 'delete',
            label: t('projects.delete_project', { defaultValue: '删除项目' }),
            icon: <Trash2 size={16} />,
            tone: 'danger',
            dividerBefore: true,
            disabled: isLockedLiveDraft,
            onSelect: () => {
              onDeleteProject(project);
            },
          },
        ];

        openContextMenu({
          contextId,
          ariaLabel: t('common.actions_for', {
            item: project.name,
            defaultValue: 'Actions for {{item}}',
          }),
          actions,
          ...request,
        });
        return;
      }

      // 2. Untagged / Inbox
      if (scopeOrProjectId === UNTAGGED_SCOPE) {
        const isCurrentInbox = browseScope === UNTAGGED_SCOPE;
        const contextId = 'workspace:scope:untagged';
        onMenuOpened?.(contextId);

        const actions: ContextMenuAction[] = [
          {
            id: 'open_inbox',
            label: t('projects.open_inbox', { defaultValue: '打开收件箱' }),
            icon: <FolderOpen size={16} />,
            disabled: isCurrentInbox || isLockedLiveDraft,
            onSelect: () => {
              void onSwitchScope(UNTAGGED_SCOPE);
            },
          },
          {
            id: 'set_active_inbox',
            label: t('projects.set_as_active', { defaultValue: '设为当前活跃' }),
            icon: <Check size={16} />,
            disabled: activeProjectId === null,
            onSelect: () => {
              onSetActiveProjectId(null);
            },
          },
        ];

        openContextMenu({
          contextId,
          ariaLabel: t('projects.inbox', { defaultValue: '收件箱' }),
          actions,
          ...request,
        });
        return;
      }

      // 3. Trash Scope
      if (scopeOrProjectId === TRASH_SCOPE) {
        const isCurrentTrash = browseScope === TRASH_SCOPE;
        const contextId = 'workspace:scope:trash';
        onMenuOpened?.(contextId);

        const actions: ContextMenuAction[] = [
          {
            id: 'open_trash',
            label: t('projects.open_trash', { defaultValue: '打开回收站' }),
            icon: <FolderOpen size={16} />,
            disabled: isCurrentTrash || isLockedLiveDraft,
            onSelect: () => {
              void onSwitchScope(TRASH_SCOPE);
            },
          },
        ];

        if (onEmptyTrash) {
          actions.push({
            id: 'empty_trash',
            label: t('history.empty_trash', { defaultValue: 'Empty Trash' }),
            icon: <Trash2 size={16} />,
            tone: 'danger',
            dividerBefore: true,
            disabled: trashCount === 0 || isLockedLiveDraft,
            onSelect: () => {
              onEmptyTrash();
            },
          });
        }

        openContextMenu({
          contextId,
          ariaLabel: t('projects.trash', { defaultValue: '回收站' }),
          actions,
          ...request,
        });
        return;
      }

      // 4. All Items Scope
      if (scopeOrProjectId === ALL_ITEMS_SCOPE) {
        const isCurrentAll = browseScope === ALL_ITEMS_SCOPE;
        const contextId = 'workspace:scope:all';
        onMenuOpened?.(contextId);

        const actions: ContextMenuAction[] = [
          {
            id: 'open_all',
            label: t('projects.open_all_items', { defaultValue: '打开全部内容' }),
            icon: <FolderOpen size={16} />,
            disabled: isCurrentAll || isLockedLiveDraft,
            onSelect: () => {
              void onSwitchScope(ALL_ITEMS_SCOPE);
            },
          },
        ];

        openContextMenu({
          contextId,
          ariaLabel: t('projects.all_items', { defaultValue: '全部内容' }),
          actions,
          ...request,
        });
        return;
      }

      // 5. Rail Empty Area
      if (scopeOrProjectId === 'rail_empty' && onOpenCreateModal) {
        const contextId = 'workspace:rail:empty';
        onMenuOpened?.(contextId);

        const actions: ContextMenuAction[] = [
          {
            id: 'new_project',
            label: t('projects.new_project_button', { defaultValue: '新建项目' }),
            icon: <Plus size={16} />,
            onSelect: () => {
              onOpenCreateModal();
            },
          },
        ];

        openContextMenu({
          contextId,
          ariaLabel: t('projects.rail_menu', { defaultValue: '项目栏操作' }),
          actions,
          ...request,
        });
      }
    },
    [
      activeProjectId,
      browseScope,
      isLockedLiveDraft,
      onDeleteProject,
      onEmptyTrash,
      onMenuOpened,
      onOpenCreateModal,
      onOpenProjectSettings,
      onSetActiveProjectId,
      onSwitchScope,
      openContextMenu,
      projects,
      t,
      trashCount,
    ]
  );

  return { openRailContextMenu };
}
