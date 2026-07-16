import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Pencil, RotateCcw, Settings as SettingsIcon, Tags, Trash2 } from 'lucide-react';
import { RenameModal } from './RenameModal';
import { ProjectCreateModal } from './projects/ProjectCreateModal';
import { ProjectSettingsModal } from './projects/ProjectSettingsModal';
import { ProjectsHeader } from './projects/ProjectsHeader';
import { ProjectsRail } from './projects/ProjectsRail';
import { ProjectsResults } from './projects/ProjectsResults';
import { ProjectsSelectionBar } from './projects/ProjectsSelectionBar';
import { ProjectsToolbar } from './projects/ProjectsToolbar';
import { TagAssignmentModal } from './projects/TagAssignmentModal';
import { useProjectSettingsDraft } from './projects/hooks/useProjectSettingsDraft';
import { useWorkspaceBrowseState } from './projects/hooks/useWorkspaceBrowseState';
import { useWorkspaceSelectionState } from './projects/hooks/useWorkspaceSelectionState';
import type { RenameTarget } from './projects/types';
import { historyService } from '../services/historyService';
import { historyQueryWorkspace } from '../services/tauri/history';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import {
  clearActiveTranscriptSession,
  openTranscriptSession,
} from '../stores/transcriptCoordinator';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import type { HistoryItem as HistoryItemType } from '../types/history';
import { isLiveRecordDraftHistoryItem } from '../types/history';
import { useContextMenu } from './context-menu/useContextMenu';
import type { ContextMenuOpenRequest } from './context-menu/trigger';

interface ProjectsViewProps {
  isActive?: boolean;
}

interface LiveDraftLockState {
  isLocked: boolean;
  isRecording: boolean;
  sourceHistoryId: string | null;
}

interface WorkspaceMenuSnapshot {
  contextId: string;
  revision: string;
}

function getLiveDraftLockState(): LiveDraftLockState {
  const { isRecording } = useTranscriptRuntimeStore.getState();
  const { sourceHistoryId } = useTranscriptSessionStore.getState();
  const sourceItem = sourceHistoryId
    ? useHistoryStore.getState().items.find((item) => item.id === sourceHistoryId)
    : null;

  return {
    isLocked: isRecording && !!sourceItem && isLiveRecordDraftHistoryItem(sourceItem),
    isRecording,
    sourceHistoryId,
  };
}

function getPrimaryTagId(item: HistoryItemType): string | null {
  return item.tagIds?.[0] ?? item.projectId ?? null;
}

function createWorkspaceMenuRevision(
  contextId: string,
  browseScope: string,
  viewMode: string,
  isSelectionMode: boolean,
  isActive: boolean,
): string {
  const historyPrefix = 'workspace:history:';
  const projectPrefix = 'workspace:project:';
  const historyState = useHistoryStore.getState();
  const projectState = useProjectStore.getState();
  const lockState = getLiveDraftLockState();
  let target: unknown = null;

  if (contextId.startsWith(historyPrefix)) {
    const id = contextId.slice(historyPrefix.length);
    target = historyState.items.find((item) => item.id === id) ?? null;
  } else if (contextId.startsWith(projectPrefix)) {
    const id = contextId.slice(projectPrefix.length);
    target = projectState.projects.find((project) => project.id === id) ?? null;
  }

  return JSON.stringify({
    target,
    browseScope,
    viewMode,
    isSelectionMode,
    isActive,
    lockState,
  });
}

export function ProjectsView({ isActive = true }: ProjectsViewProps): React.JSX.Element {
  const { t } = useTranslation();
  const { activeContextId, closeContextMenu, openContextMenu } = useContextMenu();
  const projects = useProjectStore((state) => state.projects);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const createProject = useProjectStore((state) => state.createProject);
  const updateProject = useProjectStore((state) => state.updateProject);
  const deleteProject = useProjectStore((state) => state.deleteProject);
  const setActiveProjectId = useProjectStore((state) => state.setActiveProjectId);
  const reorderProjects = useProjectStore((state) => state.reorderProjects);

  const historyItems = useHistoryStore((state) => state.items);
  const isHistoryLoading = useHistoryStore((state) => state.isLoading);
  const loadHistoryItems = useHistoryStore((state) => state.loadItems);
  const refreshHistory = useHistoryStore((state) => state.refresh);
  const deleteHistoryItems = useHistoryStore((state) => state.deleteItems);

  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const segmentsLength = useTranscriptSessionStore((state) => state.segments.length);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const setMode = useTranscriptRuntimeStore((state) => state.setMode);

  const globalConfig = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);

  const viewMode = globalConfig.projectsViewMode || 'list';

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectDescription, setNewProjectDescription] = useState('');
  const [newProjectColor, setNewProjectColor] = useState('#64748b');
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedHistoryId, setSelectedHistoryId] = useState<string | null>(sourceHistoryId);
  const [renameTarget, setRenameTarget] = useState<RenameTarget | null>(null);
  const [tagAssignmentIds, setTagAssignmentIds] = useState<string[]>([]);
  const filterMenuRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const workspaceMenuSnapshotRef = useRef<WorkspaceMenuSnapshot | null>(null);
  const browseScopeRef = useRef('inbox');
  const activeHistoryItem = useMemo(
    () => historyItems.find((item) => item.id === sourceHistoryId) || null,
    [historyItems, sourceHistoryId],
  );
  const isLiveDraftSessionLocked = isRecording && !!activeHistoryItem && isLiveRecordDraftHistoryItem(activeHistoryItem);

  useEffect(() => {
    void loadHistoryItems();
  }, [loadHistoryItems]);

  const clearOpenedItem = useCallback(() => {
    setSelectedHistoryId(null);
    clearActiveTranscriptSession({ clearAudio: true });
  }, []);

  const handleOpenItem = useCallback(async (item: HistoryItemType) => {
    if (item.deletedAt != null) {
      return;
    }
    const initialItem = useHistoryStore.getState().items.find((candidate) => candidate.id === item.id);
    const initialLockState = getLiveDraftLockState();
    if (!initialItem || (initialLockState.isLocked && item.id !== initialLockState.sourceHistoryId)) {
      return;
    }

    try {
      let segments = await historyService.loadTranscript(item.id);
      const url = await historyService.getAudioUrl(item.id);

      const latestItem = useHistoryStore.getState().items.find((candidate) => candidate.id === item.id);
      const latestLockState = getLiveDraftLockState();
      if (!latestItem || (latestLockState.isLocked && item.id !== latestLockState.sourceHistoryId)) {
        return;
      }

      if (!segments && !url) {
        await showError({
          code: 'history.missing_files_deleted',
          messageKey: 'errors.history.missing_files_deleted',
          showCause: false,
        });

        const deletableItem = useHistoryStore.getState().items.find((candidate) => candidate.id === item.id);
        const deleteLockState = getLiveDraftLockState();
        if (!deletableItem || (deleteLockState.isLocked && item.id === deleteLockState.sourceHistoryId)) {
          return;
        }

        await useHistoryStore.getState().deleteItem(item.id);
        await useHistoryStore.getState().refresh();
        return;
      }

      if (!segments) {
        segments = [];
      }

      openTranscriptSession({
        segments,
        sourceHistoryId: item.id,
        title: latestItem.title,
        icon: latestItem.icon,
        audioUrl: url,
      });
      setSelectedHistoryId(item.id);
      await useProjectStore.getState().setActiveProjectId(getPrimaryTagId(latestItem));
    } catch (error) {
      await showError({
        code: 'history.load_failed',
        messageKey: 'errors.history.load_failed',
        cause: error,
      });
    }
  }, [showError]);

  const browseState = useWorkspaceBrowseState({
    activeProjectId,
    historyItems,
    projects,
    filterMenuRef,
    isSelectionMode,
    searchInputRef,
    t,
    onOpenItem: handleOpenItem,
  });

  const projectSettingsDraft = useProjectSettingsDraft({
    browseProject: browseState.browseProject,
    confirm,
    t,
  });

  const selectionState = useWorkspaceSelectionState({
    browseProjectId: browseState.browseProjectId,
    isAllItemsScope: browseState.isAllItemsScope,
    isSelectionMode,
    projects,
    setIsSelectionMode,
  });
  const clearSelection = selectionState.clearSelection;
  const workspaceSelectionMode = selectionState.isSelectionMode;
  const syncVisibleItems = selectionState.syncVisibleItems;

  useEffect(() => {
    browseScopeRef.current = browseState.browseScope;
  }, [browseState.browseScope]);

  useEffect(() => {
    if (!activeContextId?.startsWith('workspace:')) {
      workspaceMenuSnapshotRef.current = null;
      return;
    }

    const snapshot = workspaceMenuSnapshotRef.current;
    if (!snapshot || snapshot.contextId !== activeContextId) {
      closeContextMenu();
      return;
    }

    const latestRevision = createWorkspaceMenuRevision(
      activeContextId,
      browseState.browseScope,
      viewMode,
      selectionState.isSelectionMode,
      isActive,
    );
    if (latestRevision !== snapshot.revision) {
      closeContextMenu();
    }
  }, [
    activeContextId,
    browseState.browseScope,
    closeContextMenu,
    historyItems,
    isActive,
    isRecording,
    projects,
    selectionState.isSelectionMode,
    sourceHistoryId,
    viewMode,
  ]);

  useEffect(() => {
    if (!isActive) {
      return;
    }

    syncVisibleItems(browseState.filteredAndSortedItems);
  }, [browseState.filteredAndSortedItems, isActive, syncVisibleItems]);

  const itemMatchesBrowseScope = useCallback((item: HistoryItemType) => {
    if (browseState.isTrashScope) {
      return item.deletedAt != null;
    }
    if (item.deletedAt != null) {
      return false;
    }
    if (browseState.isAllItemsScope) {
      return true;
    }
    if (browseState.isInboxScope) {
      return (item.tagIds ?? (item.projectId ? [item.projectId] : [])).length === 0;
    }
    return (item.tagIds ?? (item.projectId ? [item.projectId] : []))
      .includes(browseState.browseProjectId || '');
  }, [
    browseState.browseProjectId,
    browseState.isAllItemsScope,
    browseState.isInboxScope,
    browseState.isTrashScope,
  ]);

  const scopedSourceHistoryId = useMemo(() => {
    if (!sourceHistoryId) {
      return null;
    }
    const sourceItem = historyItems.find((item) => item.id === sourceHistoryId);
    return sourceItem && itemMatchesBrowseScope(sourceItem) ? sourceHistoryId : null;
  }, [historyItems, itemMatchesBrowseScope, sourceHistoryId]);

  const effectiveSelectedHistoryId = useMemo(() => {
    if (isLiveDraftSessionLocked && sourceHistoryId) {
      return sourceHistoryId;
    }

    if (!selectedHistoryId) {
      return scopedSourceHistoryId;
    }

    const selectedItemStillVisible = historyItems.some(
      (item) => item.id === selectedHistoryId && itemMatchesBrowseScope(item),
    );
    if (!selectedItemStillVisible) {
      return null;
    }

    if (!sourceHistoryId && segmentsLength === 0) {
      return null;
    }

    return selectedHistoryId;
  }, [
    historyItems,
    isLiveDraftSessionLocked,
    itemMatchesBrowseScope,
    scopedSourceHistoryId,
    segmentsLength,
    selectedHistoryId,
    sourceHistoryId,
  ]);

  const selectedItem = useMemo(
    () => historyItems.find((item) => item.id === effectiveSelectedHistoryId) || null,
    [effectiveSelectedHistoryId, historyItems],
  );
  const tagAssignmentItems = useMemo(() => {
    const candidates = new Map<string, HistoryItemType>();
    historyItems.forEach((item) => candidates.set(item.id, item));
    browseState.filteredAndSortedItems.forEach((item) => candidates.set(item.id, item));
    return tagAssignmentIds
      .map((id) => candidates.get(id))
      .filter((item): item is HistoryItemType => !!item && item.deletedAt == null);
  }, [browseState.filteredAndSortedItems, historyItems, tagAssignmentIds]);

  useEffect(() => {
    if (effectiveSelectedHistoryId === null && selectedHistoryId) {
      queueMicrotask(() => {
        setSelectedHistoryId(null);
        clearActiveTranscriptSession({ clearAudio: true });
      });
      return;
    }

    if (effectiveSelectedHistoryId === null) {
      const sessionState = useTranscriptSessionStore.getState();
      const playbackState = useTranscriptPlaybackStore.getState();
      if (sessionState.sourceHistoryId || sessionState.segments.length > 0 || playbackState.audioUrl) {
        clearActiveTranscriptSession({ clearAudio: true });
      }
    }
  }, [effectiveSelectedHistoryId, selectedHistoryId]);

  useEffect(() => {
    if (!isLiveDraftSessionLocked) {
      return;
    }

    if (workspaceSelectionMode) {
      clearSelection();
    }
  }, [clearSelection, isLiveDraftSessionLocked, workspaceSelectionMode]);

  const handleSwitchBrowseScope = async (nextScope: string): Promise<boolean> => {
    const isProjectScope = nextScope !== 'all' && nextScope !== 'untagged' && nextScope !== 'trash';
    const initialLockState = getLiveDraftLockState();
    if (
      initialLockState.isLocked
      || (isProjectScope && !useProjectStore.getState().projects.some((project) => project.id === nextScope))
    ) {
      return false;
    }

    const shouldDiscard = await projectSettingsDraft.confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return false;
    }

    const latestLockState = getLiveDraftLockState();
    if (
      latestLockState.isLocked
      || (isProjectScope && !useProjectStore.getState().projects.some((project) => project.id === nextScope))
    ) {
      return false;
    }

    if (projectSettingsDraft.isSettingsOpen) {
      projectSettingsDraft.discardProjectSettingsDraft();
    }

    selectionState.clearSelection();
    browseState.setIsFilterMenuOpen(false);
    browseState.setBrowseScope(nextScope);

    // Auto-close active editor/session on switching browse scope
    clearOpenedItem();

    if (nextScope === 'all') {
      return true;
    }

    if (nextScope !== 'trash') {
      await useProjectStore.getState().setActiveProjectId(nextScope === 'untagged' ? null : nextScope);
    }
    return true;
  };

  const handleOpenProjectSettings = async (id: string) => {
    let project = useProjectStore.getState().projects.find((item) => item.id === id);
    const initialLockState = getLiveDraftLockState();
    const isCurrentProject = browseScopeRef.current === id;
    if (!project || (initialLockState.isLocked && !isCurrentProject)) {
      return;
    }

    if (!isCurrentProject) {
      const didSwitch = await handleSwitchBrowseScope(id);
      if (!didSwitch) {
        return;
      }
    }

    project = useProjectStore.getState().projects.find((item) => item.id === id);
    const latestLockState = getLiveDraftLockState();
    if (!project || (latestLockState.isLocked && !isCurrentProject)) {
      return;
    }

    projectSettingsDraft.resetProjectSettingsDraft(project);
    projectSettingsDraft.setIsSettingsOpen(true);
  };

  const handleOpenProjectContextMenu = (
    id: string,
    request: ContextMenuOpenRequest,
  ) => {
    const project = useProjectStore.getState().projects.find((item) => item.id === id);
    if (!project) {
      return;
    }

    const lockState = getLiveDraftLockState();
    const isCurrentProject = browseScopeRef.current === id;
    const isOtherProjectLocked = lockState.isLocked && !isCurrentProject;
    const contextId = `workspace:project:${id}`;

    workspaceMenuSnapshotRef.current = {
      contextId,
      revision: createWorkspaceMenuRevision(
        contextId,
        browseScopeRef.current,
        viewMode,
        selectionState.isSelectionMode,
        isActive,
      ),
    };

    openContextMenu({
      contextId,
      ariaLabel: t('common.actions_for', {
        item: project.name,
        defaultValue: 'Actions for {{item}}',
      }),
      actions: [
        {
          id: 'open',
          label: t('common.open', { defaultValue: 'Open' }),
          icon: <FolderOpen size={16} />,
          disabled: isCurrentProject || lockState.isLocked,
          onSelect: () => {
            void handleSwitchBrowseScope(id);
          },
        },
        {
          id: 'settings',
          label: t('projects.tag_settings', { defaultValue: 'Tag Settings' }),
          icon: <SettingsIcon size={16} />,
          disabled: isOtherProjectLocked,
          onSelect: () => {
            void handleOpenProjectSettings(id);
          },
        },
      ],
      ...request,
    });
  };

  const handleDeleteHistoryItem = async (id: string) => {
    const initialItem = useHistoryStore.getState().items.find((item) => item.id === id)
      ?? browseState.filteredAndSortedItems.find((item) => item.id === id);
    const initialLockState = getLiveDraftLockState();
    if (!initialItem || (initialLockState.isLocked && id === initialLockState.sourceHistoryId)) {
      return;
    }

    const isTrashItem = initialItem.deletedAt != null;
    const confirmed = await confirm(
      isTrashItem
        ? t('history.purge_confirm', { defaultValue: 'Permanently delete this item? This cannot be undone.' })
        : t('history.trash_confirm', { defaultValue: 'Move this item to Trash?' }), {
      title: isTrashItem
        ? t('history.purge_title', { defaultValue: 'Delete Permanently' })
        : t('history.trash_title', { defaultValue: 'Move to Trash' }),
      confirmLabel: isTrashItem
        ? t('history.delete_permanently', { defaultValue: 'Delete Permanently' })
        : t('history.move_to_trash', { defaultValue: 'Move to Trash' }),
      variant: 'error',
    });

    if (!confirmed) {
      return;
    }

    const latestItem = useHistoryStore.getState().items.find((item) => item.id === id)
      ?? browseState.filteredAndSortedItems.find((item) => item.id === id);
    const latestLockState = getLiveDraftLockState();
    if (!latestItem || (latestLockState.isLocked && id === latestLockState.sourceHistoryId)) {
      return;
    }

    if (isTrashItem) {
      await historyService.purgeRecordings([id]);
    } else {
      await useHistoryStore.getState().deleteItem(id);
    }
    await useHistoryStore.getState().refresh();
  };

  const handleRestoreHistoryItems = async (ids: string[]) => {
    await historyService.restoreRecordings(ids);
    await refreshHistory();
    selectionState.clearSelection();
  };

  const handleRenameHistoryItem = async (id: string) => {
    const item = useHistoryStore.getState().items.find((historyItem) => historyItem.id === id)
      ?? browseState.filteredAndSortedItems.find((historyItem) => historyItem.id === id);
    const lockState = getLiveDraftLockState();
    if (!item || (lockState.isLocked && id === lockState.sourceHistoryId)) {
      return;
    }

    setRenameTarget({ id, title: item.title, icon: item.icon, type: item.type });
  };

  const handleOpenHistoryContextMenu = (
    id: string,
    request: ContextMenuOpenRequest,
  ) => {
    const item = useHistoryStore.getState().items.find((historyItem) => historyItem.id === id);
    if (!item || selectionState.isSelectionMode) {
      return;
    }

    const lockState = getLiveDraftLockState();
    const isLockedLiveDraft = lockState.isLocked && id === lockState.sourceHistoryId;
    const isOpenDisabled = lockState.isLocked && id !== lockState.sourceHistoryId;
    const contextId = `workspace:history:${id}`;

    workspaceMenuSnapshotRef.current = {
      contextId,
      revision: createWorkspaceMenuRevision(
        contextId,
        browseScopeRef.current,
        viewMode,
        selectionState.isSelectionMode,
        isActive,
      ),
    };

    openContextMenu({
      contextId,
      ariaLabel: t('common.actions_for', {
        item: item.title,
        defaultValue: 'Actions for {{item}}',
      }),
      actions: item.deletedAt != null ? [
        {
          id: 'restore',
          label: t('history.restore', { defaultValue: 'Restore' }),
          icon: <RotateCcw size={16} />,
          onSelect: () => {
            void handleRestoreHistoryItems([id]);
          },
        },
        {
          id: 'purge',
          label: t('history.delete_permanently', { defaultValue: 'Delete Permanently' }),
          icon: <Trash2 size={16} />,
          tone: 'danger',
          dividerBefore: true,
          onSelect: () => {
            void handleDeleteHistoryItem(id);
          },
        },
      ] : [
        {
          id: 'tags',
          label: t('projects.edit_tags', { defaultValue: 'Edit Tags' }),
          icon: <Tags size={16} />,
          disabled: isLockedLiveDraft,
          onSelect: () => {
            setTagAssignmentIds([id]);
          },
        },
        {
          id: 'open',
          label: t('common.open', { defaultValue: 'Open' }),
          icon: <FolderOpen size={16} />,
          disabled: isOpenDisabled,
          onSelect: () => {
            const latestItem = useHistoryStore.getState().items.find((historyItem) => historyItem.id === id);
            if (latestItem) {
              void handleOpenItem(latestItem);
            }
          },
        },
        {
          id: 'rename',
          label: t('common.rename', { defaultValue: 'Rename' }),
          icon: <Pencil size={16} />,
          disabled: isLockedLiveDraft,
          onSelect: () => {
            void handleRenameHistoryItem(id);
          },
        },
        {
          id: 'delete',
          label: t('common.delete', { defaultValue: 'Delete' }),
          icon: <Trash2 size={16} />,
          disabled: isLockedLiveDraft,
          tone: 'danger',
          dividerBefore: true,
          onSelect: () => {
            void handleDeleteHistoryItem(id);
          },
        },
      ],
      ...request,
    });
  };

  const handlePerformRename = async (newTitle: string, newIcon?: string) => {
    if (!renameTarget) {
      return;
    }

    const targetId = renameTarget.id;
    const target = useHistoryStore.getState().items.find((item) => item.id === targetId);
    const lockState = getLiveDraftLockState();
    if (!target || (lockState.isLocked && lockState.sourceHistoryId === targetId)) {
      return;
    }

    const trimmedTitle = newTitle.trim();
    await useHistoryStore.getState().updateItemMeta(targetId, { title: trimmedTitle, icon: newIcon });
    await useHistoryStore.getState().refresh();

    const sessionState = useTranscriptSessionStore.getState();
    if (sessionState.sourceHistoryId === targetId) {
      sessionState.setTitle(trimmedTitle);
      sessionState.setIcon(newIcon || null);
    }

    setRenameTarget(null);
  };

  const handleCreateProject = async () => {
    if (!newProjectName.trim()) {
      return;
    }

    const project = await createProject(
      {
        name: newProjectName.trim(),
        description: newProjectDescription.trim(),
        color: newProjectColor,
      },
      globalConfig,
    );

    if (!project) {
      return;
    }

    setNewProjectName('');
    setNewProjectDescription('');
    setNewProjectColor('#64748b');
    setIsCreateModalOpen(false);
    browseState.setBrowseScope(project.id);
    await setActiveProjectId(project.id);
  };

  const handleSaveProject = async () => {
    if (!browseState.browseProject || !projectSettingsDraft.draftDefaults) {
      return;
    }

    await updateProject(browseState.browseProject.id, {
      name: projectSettingsDraft.draftName.trim() || browseState.browseProject.name,
      description: projectSettingsDraft.draftDescription,
      icon: projectSettingsDraft.draftIcon,
      color: projectSettingsDraft.draftColor,
      defaults: projectSettingsDraft.draftDefaults,
    });
    projectSettingsDraft.setIsSettingsOpen(false);
  };

  const handleDeleteProject = async () => {
    if (!browseState.browseProject) {
      return;
    }

    const shouldDiscard = await projectSettingsDraft.confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (projectSettingsDraft.isSettingsOpen) {
      projectSettingsDraft.discardProjectSettingsDraft(browseState.browseProject);
    }

    const confirmed = await confirm(
      t('projects.delete_tag_confirm', {
        tag: browseState.browseProject.name,
        defaultValue: `Delete ${browseState.browseProject.name}? Items keep their other tags.`,
      }),
      {
        title: t('projects.delete_tag_title', { defaultValue: 'Delete Tag' }),
        confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
        variant: 'error',
      },
    );

    if (!confirmed) {
      return;
    }

    clearOpenedItem();
    browseState.setBrowseScope('untagged');
    await deleteProject(browseState.browseProject.id);
    await refreshHistory();
  };

  const handleToggleSelectionMode = () => {
    if (isLiveDraftSessionLocked) {
      return;
    }
    browseState.setIsFilterMenuOpen(false);
    selectionState.toggleSelectionMode();
  };

  const handleDeleteSelected = async () => {
    if (selectionState.selectedIds.length === 0) {
      return;
    }

    const isTrashScope = browseState.isTrashScope;
    const confirmed = await confirm(
      isTrashScope
        ? t('history.purge_bulk_confirm', {
          count: selectionState.selectedIds.length,
          defaultValue: `Permanently delete ${selectionState.selectedIds.length} items? This cannot be undone.`,
        })
        : t('history.trash_bulk_confirm', {
        count: selectionState.selectedIds.length,
        defaultValue: `Move ${selectionState.selectedIds.length} items to Trash?`,
      }),
      {
        title: isTrashScope
          ? t('history.purge_title', { defaultValue: 'Delete Permanently' })
          : t('history.trash_title', { defaultValue: 'Move to Trash' }),
        confirmLabel: isTrashScope
          ? t('history.delete_permanently', { defaultValue: 'Delete Permanently' })
          : t('history.move_to_trash', { defaultValue: 'Move to Trash' }),
        variant: 'error',
      },
    );

    if (!confirmed) {
      return;
    }

    if (isTrashScope) {
      await historyService.purgeRecordings(selectionState.selectedIds);
    } else {
      await deleteHistoryItems(selectionState.selectedIds);
    }
    await refreshHistory();
    selectionState.clearSelection();
  };

  const handleEmptyTrash = async () => {
    const trashCount = browseState.itemCounts.get('trash') || 0;
    if (trashCount === 0) return;
    const confirmed = await confirm(
      t('history.empty_trash_confirm', {
        count: trashCount,
        defaultValue: `Permanently delete all ${trashCount} items in Trash? This cannot be undone.`,
      }),
      {
        title: t('history.empty_trash', { defaultValue: 'Empty Trash' }),
        confirmLabel: t('history.delete_permanently', { defaultValue: 'Delete Permanently' }),
        variant: 'error',
      },
    );
    if (!confirmed) return;

    const ids: string[] = [];
    let offset = 0;
    const limit = 200;
    while (true) {
      const page = await historyQueryWorkspace({
        scope: { kind: 'trash' },
        query: '',
        filterType: 'all',
        dateFilter: 'all',
        sortOrder: 'newest',
        limit,
        offset,
      });
      ids.push(...page.filteredItems.map((item) => item.id));
      if (!page.hasMore || page.filteredItems.length === 0) break;
      offset += page.filteredItems.length;
    }
    await historyService.purgeRecordings(ids);
    await refreshHistory();
  };

  if (!isActive) {
    return (
      <div
        className="projects-workbench"
        data-projects-inactive="true"
        hidden
        aria-hidden="true"
      />
    );
  }

  return (
    <div className={`projects-workbench ${selectedItem ? 'with-detail' : ''}`}>
      <ProjectsRail
        activeContextId={activeContextId}
        browseProjectId={browseState.browseProjectId}
        historyItemsCount={historyItems.length}
        inboxCount={browseState.itemCounts.get(null) || 0}
        isAllItemsScope={browseState.isAllItemsScope}
        isInboxScope={browseState.isInboxScope}
        isTrashScope={browseState.isTrashScope}
        trashCount={browseState.itemCounts.get('trash') || 0}
        itemCounts={browseState.itemCounts}
        onOpenCreateModal={() => setIsCreateModalOpen(true)}
        onReorderProjects={reorderProjects}
        onSwitchScope={handleSwitchBrowseScope}
        onOpenProjectContextMenu={handleOpenProjectContextMenu}
        projects={projects}
        t={t}
      />

      {!selectedItem && (
        <section className="projects-main">
          <ProjectsHeader
            browseProject={browseState.browseProject}
            headerDescription={browseState.headerDescription}
            headerIcon={browseState.headerIcon}
            headerTitle={browseState.headerTitle}
            isScrolled={browseState.isScrolled}
            onOpenBatchImport={() => setMode('batch')}
            onOpenProjectSettings={() => projectSettingsDraft.setIsSettingsOpen(true)}
            onStartLiveRecord={() => setMode('live')}
            showWorkflowActions={browseState.showWorkflowActions}
            isTrashScope={browseState.isTrashScope}
            onEmptyTrash={() => void handleEmptyTrash()}
            trashItemCount={browseState.itemCounts.get('trash') || 0}
            summaryChips={browseState.summaryChips}
            t={t}
          />

              <ProjectsToolbar
            activeFilterCount={browseState.activeFilterCount}
            currentSearchResultId={browseState.activeSearchResultId}
            dateFilter={browseState.dateFilter}
            dateFilterOptions={browseState.dateFilterOptions}
            filterMenuHint={browseState.filterPopoverHint}
            filterMenuRef={filterMenuRef}
            filterType={browseState.filterType}
            filterTypeOptions={browseState.filterTypeOptions}
            filteredResultsCount={browseState.filteredItemCount}
            hasActiveFilters={browseState.hasActiveFilters}
            isFilterMenuOpen={browseState.isFilterMenuOpen}
            isSelectionMode={selectionState.isSelectionMode}
            onClearSearch={() => browseState.setSearchQuery('')}
            onOpenHistoryFolder={() => historyService.openHistoryFolder()}
            onResetBrowseState={browseState.resetBrowseState}
            onSearchInputKeyDown={browseState.handleWorkspaceSearchInputKeyDown}
            onSearchQueryChange={browseState.setSearchQuery}
            onSetDateFilter={browseState.setDateFilter}
            onSetFilterMenuOpen={browseState.setIsFilterMenuOpen}
            onSetFilterType={browseState.setFilterType}
            onSetSortOrder={browseState.setSortOrder}
            onSetViewMode={(nextViewMode) => setConfig({ projectsViewMode: nextViewMode })}
                onToggleSelectionMode={handleToggleSelectionMode}
                disableSelectionModeToggle={isLiveDraftSessionLocked}
                scopedItemsCount={browseState.scopeItemCount}
            searchInputLabel={browseState.searchInputLabel}
            searchInputRef={searchInputRef}
            searchQuery={browseState.searchQuery}
            sortOptions={browseState.sortOptions}
            sortOrder={browseState.sortOrder}
            t={t}
            viewMode={viewMode}
          />

          {selectionState.isSelectionMode && (
            <ProjectsSelectionBar
              isTrashScope={browseState.isTrashScope}
              onCancel={handleToggleSelectionMode}
              onDeleteSelected={() => void handleDeleteSelected()}
              onEditTags={() => setTagAssignmentIds(selectionState.selectedIds)}
              onRestoreSelected={() => void handleRestoreHistoryItems(selectionState.selectedIds)}
              onToggleSelectAll={selectionState.handleToggleSelectAll}
              selectedIds={selectionState.selectedIds}
              totalVisibleItems={browseState.filteredAndSortedItems.length}
              t={t}
            />
          )}

          <ProjectsResults
            activeContextId={activeContextId}
            activeSearchResultId={browseState.activeSearchResultId}
            browseProject={browseState.browseProject}
            filteredAndSortedItems={browseState.filteredAndSortedItems}
            handleOpenItem={handleOpenItem}
            initialLoadError={browseState.initialLoadError}
            isAllItemsScope={browseState.isAllItemsScope}
            isHistoryLoading={isHistoryLoading}
            isInitialLoading={browseState.isInitialLoading}
            isLoadingMore={browseState.isLoadingMore}
            isSelectionMode={selectionState.isSelectionMode}
            isTrashScope={browseState.isTrashScope}
            loadMoreError={browseState.loadMoreError}
            lockedHistoryId={isLiveDraftSessionLocked ? sourceHistoryId : null}
            onDeleteHistoryItem={handleDeleteHistoryItem}
            onLoadMore={browseState.loadMore}
            onRenameHistoryItem={handleRenameHistoryItem}
            onOpenHistoryContextMenu={handleOpenHistoryContextMenu}
            onRetryInitialLoad={browseState.retryInitialLoad}
            onScroll={browseState.handleScroll}
            onToggleSelection={selectionState.toggleSelection}
            resetBrowseState={browseState.resetBrowseState}
            filteredItemCount={browseState.filteredItemCount}
            scopeItemCount={browseState.scopeItemCount}
            searchMatchByItemId={browseState.searchMatchByItemId}
            searchQuery={browseState.searchQuery}
            selectedHistoryId={effectiveSelectedHistoryId}
            selectedIds={selectionState.selectedIds}
            t={t}
            viewMode={viewMode}
          />
        </section>
      )}

      {selectedItem && (
        <aside
          className="projects-detail-pane"
          data-projects-detail-placeholder="true"
          aria-hidden="true"
        />
      )}

      <ProjectCreateModal
        isOpen={isCreateModalOpen}
        name={newProjectName}
        description={newProjectDescription}
        color={newProjectColor}
        onNameChange={setNewProjectName}
        onDescriptionChange={setNewProjectDescription}
        onColorChange={setNewProjectColor}
        onClose={() => setIsCreateModalOpen(false)}
        onCreate={handleCreateProject}
      />

      <ProjectSettingsModal
        isOpen={projectSettingsDraft.isSettingsOpen}
        project={browseState.browseProject}
        draftName={projectSettingsDraft.draftName}
        draftDescription={projectSettingsDraft.draftDescription}
        draftIcon={projectSettingsDraft.draftIcon}
        draftColor={projectSettingsDraft.draftColor}
        draftDefaults={projectSettingsDraft.draftDefaults}
        globalConfig={globalConfig}
        onClose={projectSettingsDraft.handleRequestCloseProjectSettings}
        onSave={handleSaveProject}
        onDelete={handleDeleteProject}
        onNameChange={projectSettingsDraft.setDraftName}
        onDescriptionChange={projectSettingsDraft.setDraftDescription}
        onIconChange={projectSettingsDraft.setDraftIcon}
        onColorChange={projectSettingsDraft.setDraftColor}
        onDefaultsChange={projectSettingsDraft.setDraftDefaults}
      />

      <RenameModal
        isOpen={!!renameTarget}
        onClose={() => setRenameTarget(null)}
        initialTitle={renameTarget?.title || ''}
        initialIcon={renameTarget?.icon}
        defaultType={renameTarget?.type}
        onRename={handlePerformRename}
        onAiAction={async () => {
          if (!renameTarget) {
            return '';
          }
          const item = historyItems.find((historyItem) => historyItem.id === renameTarget.id);
          if (!item) {
            return '';
          }
          const { generateAiTitleForHistoryItem } = await import('../services/aiRenameService');
          return await generateAiTitleForHistoryItem(item.id);
        }}
      />

      <TagAssignmentModal
        isOpen={tagAssignmentIds.length > 0}
        items={tagAssignmentItems}
        tags={projects}
        onClose={() => setTagAssignmentIds([])}
        onApply={async (addTagIds, removeTagIds) => {
          await historyService.updateTagAssignments(tagAssignmentIds, addTagIds, removeTagIds);
          await refreshHistory();
          selectionState.clearSelection();
        }}
      />
    </div>
  );
}

export default ProjectsView;
