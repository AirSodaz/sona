import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Pencil, Settings as SettingsIcon, Trash2 } from 'lucide-react';
import { RenameModal } from './RenameModal';
import { ProjectCreateModal } from './projects/ProjectCreateModal';
import { ProjectSettingsModal } from './projects/ProjectSettingsModal';
import { ProjectsHeader } from './projects/ProjectsHeader';
import { ProjectsRail } from './projects/ProjectsRail';
import { ProjectsResults } from './projects/ProjectsResults';
import { ProjectsSelectionBar } from './projects/ProjectsSelectionBar';
import { ProjectsToolbar } from './projects/ProjectsToolbar';
import { useProjectSettingsDraft } from './projects/hooks/useProjectSettingsDraft';
import { useWorkspaceBrowseState } from './projects/hooks/useWorkspaceBrowseState';
import { useWorkspaceSelectionState } from './projects/hooks/useWorkspaceSelectionState';
import type { RenameTarget } from './projects/types';
import { historyService } from '../services/historyService';
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
  const assignHistoryItems = useProjectStore((state) => state.assignHistoryItems);
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
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedHistoryId, setSelectedHistoryId] = useState<string | null>(sourceHistoryId);
  const [renameTarget, setRenameTarget] = useState<RenameTarget | null>(null);
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
      await useProjectStore.getState().setActiveProjectId(latestItem.projectId);
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
    if (browseState.isAllItemsScope) {
      return true;
    }
    if (browseState.isInboxScope) {
      return !item.projectId;
    }
    return item.projectId === browseState.browseProjectId;
  }, [browseState.browseProjectId, browseState.isAllItemsScope, browseState.isInboxScope]);

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
    const isProjectScope = nextScope !== 'all' && nextScope !== 'inbox';
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

    await useProjectStore.getState().setActiveProjectId(nextScope === 'inbox' ? null : nextScope);
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
          label: t('projects.project_settings', { defaultValue: 'Project Settings' }),
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
    const initialItem = useHistoryStore.getState().items.find((item) => item.id === id);
    const initialLockState = getLiveDraftLockState();
    if (!initialItem || (initialLockState.isLocked && id === initialLockState.sourceHistoryId)) {
      return;
    }

    const confirmed = await confirm(t('history.delete_confirm'), {
      title: t('history.delete_title', { defaultValue: 'Delete History' }),
      confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
      variant: 'error',
    });

    if (!confirmed) {
      return;
    }

    const latestItem = useHistoryStore.getState().items.find((item) => item.id === id);
    const latestLockState = getLiveDraftLockState();
    if (!latestItem || (latestLockState.isLocked && id === latestLockState.sourceHistoryId)) {
      return;
    }

    await useHistoryStore.getState().deleteItem(id);
    await useHistoryStore.getState().refresh();
  };

  const handleRenameHistoryItem = async (id: string) => {
    const item = useHistoryStore.getState().items.find((historyItem) => historyItem.id === id);
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
      actions: [
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
      },
      globalConfig,
    );

    if (!project) {
      return;
    }

    setNewProjectName('');
    setNewProjectDescription('');
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
      t('projects.delete_confirm', {
        defaultValue: `Delete ${browseState.browseProject.name} and move its items back to Inbox?`,
      }),
      {
        title: t('projects.delete_title', { defaultValue: 'Delete Project' }),
        confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
        variant: 'error',
      },
    );

    if (!confirmed) {
      return;
    }

    clearOpenedItem();
    browseState.setBrowseScope('inbox');
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

  const handleMoveSelected = async () => {
    if (selectionState.selectedIds.length === 0) {
      return;
    }

    const targetProjectId = selectionState.moveTarget === 'inbox' ? null : selectionState.moveTarget;
    await assignHistoryItems(selectionState.selectedIds, targetProjectId);
    await refreshHistory();

    const currentHistoryId = useTranscriptSessionStore.getState().sourceHistoryId;
    if (currentHistoryId && selectionState.selectedIds.includes(currentHistoryId)) {
      await setActiveProjectId(targetProjectId);
    }

    selectionState.clearSelection();
  };

  const handleDeleteSelected = async () => {
    if (selectionState.selectedIds.length === 0) {
      return;
    }

    const confirmed = await confirm(
      t('history.delete_bulk_confirm', {
        count: selectionState.selectedIds.length,
        defaultValue: `Are you sure you want to delete ${selectionState.selectedIds.length} items?`,
      }),
      {
        title: t('history.delete_title', { defaultValue: 'Delete History' }),
        confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
        variant: 'error',
      },
    );

    if (!confirmed) {
      return;
    }

    await deleteHistoryItems(selectionState.selectedIds);
    await refreshHistory();
    selectionState.clearSelection();
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
              currentScopeMoveTarget={selectionState.currentScopeMoveTarget}
              moveOptions={browseState.moveOptions}
              moveTarget={selectionState.moveTarget}
              onCancel={handleToggleSelectionMode}
              onDeleteSelected={() => void handleDeleteSelected()}
              onMoveSelected={() => void handleMoveSelected()}
              onMoveTargetChange={selectionState.setMoveTarget}
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
        onNameChange={setNewProjectName}
        onDescriptionChange={setNewProjectDescription}
        onClose={() => setIsCreateModalOpen(false)}
        onCreate={handleCreateProject}
      />

      <ProjectSettingsModal
        isOpen={projectSettingsDraft.isSettingsOpen}
        project={browseState.browseProject}
        draftName={projectSettingsDraft.draftName}
        draftDescription={projectSettingsDraft.draftDescription}
        draftIcon={projectSettingsDraft.draftIcon}
        draftDefaults={projectSettingsDraft.draftDefaults}
        globalConfig={globalConfig}
        onClose={projectSettingsDraft.handleRequestCloseProjectSettings}
        onSave={handleSaveProject}
        onDelete={handleDeleteProject}
        onNameChange={projectSettingsDraft.setDraftName}
        onDescriptionChange={projectSettingsDraft.setDraftDescription}
        onIconChange={projectSettingsDraft.setDraftIcon}
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
    </div>
  );
}

export default ProjectsView;
