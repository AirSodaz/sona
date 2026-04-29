import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RenameModal } from './RenameModal';
import { TranscriptWorkbench } from './TranscriptWorkbench';
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
import { generateAiTitleForHistoryItem } from '../services/aiRenameService';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { HistoryItem as HistoryItemType } from '../types/history';
import { isLiveRecordDraftHistoryItem } from '../types/history';

export function ProjectsView(): React.JSX.Element {
  const { t } = useTranslation();
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
  const deleteHistoryItem = useHistoryStore((state) => state.deleteItem);
  const deleteHistoryItems = useHistoryStore((state) => state.deleteItems);
  const updateHistoryItemMeta = useHistoryStore((state) => state.updateItemMeta);

  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const segmentsLength = useTranscriptStore((state) => state.segments.length);
  const isRecording = useTranscriptStore((state) => state.isRecording);
  const clearSegments = useTranscriptStore((state) => state.clearSegments);
  const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
  const setMode = useTranscriptStore((state) => state.setMode);
  const setTitle = useTranscriptStore((state) => state.setTitle);

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
    clearSegments();
    setAudioUrl(null);
  }, [clearSegments, setAudioUrl]);

  const handleOpenItem = useCallback(async (item: HistoryItemType) => {
    if (isLiveDraftSessionLocked && item.id !== sourceHistoryId) {
      return;
    }

    try {
      let segments = await historyService.loadTranscript(item.transcriptPath);
      const url = await historyService.getAudioUrl(item.audioPath);

      if (!segments && !url) {
        await showError({
          code: 'history.missing_files_deleted',
          messageKey: 'errors.history.missing_files_deleted',
          showCause: false,
        });
        await deleteHistoryItem(item.id);
        await refreshHistory();
        return;
      }

      if (!segments) {
        segments = [];
      }

      useTranscriptStore.getState().loadTranscript(segments, item.id, item.title, item.icon);
      setAudioUrl(url);
      setSelectedHistoryId(item.id);
      await setActiveProjectId(item.projectId);
    } catch (error) {
      await showError({
        code: 'history.load_failed',
        messageKey: 'errors.history.load_failed',
        cause: error,
      });
    }
  }, [deleteHistoryItem, isLiveDraftSessionLocked, refreshHistory, setActiveProjectId, setAudioUrl, showError, sourceHistoryId]);

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
    syncVisibleItems(browseState.filteredAndSortedItems);
  }, [browseState.filteredAndSortedItems, syncVisibleItems]);

  const scopedSourceHistoryId = useMemo(
    () => (
      sourceHistoryId && browseState.scopedItems.some((item) => item.id === sourceHistoryId)
        ? sourceHistoryId
        : null
    ),
    [browseState.scopedItems, sourceHistoryId],
  );

  const effectiveSelectedHistoryId = useMemo(() => {
    if (isLiveDraftSessionLocked && sourceHistoryId) {
      return sourceHistoryId;
    }

    if (!selectedHistoryId) {
      return scopedSourceHistoryId;
    }

    const selectedItemStillVisible = browseState.scopedItems.some((item) => item.id === selectedHistoryId);
    if (!selectedItemStillVisible) {
      return null;
    }

    if (!sourceHistoryId && segmentsLength === 0) {
      return null;
    }

    return selectedHistoryId;
  }, [
    browseState.scopedItems,
    isLiveDraftSessionLocked,
    scopedSourceHistoryId,
    segmentsLength,
    selectedHistoryId,
    sourceHistoryId,
  ]);

  const selectedItem = useMemo(
    () => browseState.scopedItems.find((item) => item.id === effectiveSelectedHistoryId) || null,
    [browseState.scopedItems, effectiveSelectedHistoryId],
  );

  useEffect(() => {
    if (effectiveSelectedHistoryId === null && selectedHistoryId) {
      queueMicrotask(() => {
        setSelectedHistoryId(null);
        clearSegments();
        setAudioUrl(null);
      });
      return;
    }

    if (effectiveSelectedHistoryId === null) {
      const transcriptState = useTranscriptStore.getState();
      if (transcriptState.sourceHistoryId || transcriptState.segments.length > 0 || transcriptState.audioUrl) {
        transcriptState.clearSegments();
        transcriptState.setAudioUrl(null);
      }
    }
  }, [clearSegments, effectiveSelectedHistoryId, selectedHistoryId, setAudioUrl]);

  useEffect(() => {
    if (!isLiveDraftSessionLocked) {
      return;
    }

    if (workspaceSelectionMode) {
      clearSelection();
    }
  }, [clearSelection, isLiveDraftSessionLocked, workspaceSelectionMode]);

  const handleSwitchBrowseScope = async (nextScope: string) => {
    if (isLiveDraftSessionLocked) {
      return;
    }

    const shouldDiscard = await projectSettingsDraft.confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (projectSettingsDraft.isSettingsOpen) {
      projectSettingsDraft.discardProjectSettingsDraft();
    }

    selectionState.clearSelection();
    browseState.setIsFilterMenuOpen(false);
    browseState.setBrowseScope(nextScope);

    if (nextScope === 'all') {
      return;
    }

    await setActiveProjectId(nextScope === 'inbox' ? null : nextScope);
  };

  const handleDeleteHistoryItem = async (event: React.MouseEvent, id: string) => {
    event.stopPropagation();
    if (isLiveDraftSessionLocked && id === sourceHistoryId) {
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

    await deleteHistoryItem(id);
    await refreshHistory();
  };

  const handleRenameHistoryItem = async (event: React.MouseEvent, id: string) => {
    event.stopPropagation();
    if (isLiveDraftSessionLocked && id === sourceHistoryId) {
      return;
    }
    const item = historyItems.find((historyItem) => historyItem.id === id);
    if (!item) {
      return;
    }

    setRenameTarget({ id, title: item.title, icon: item.icon, type: item.type });
  };

  const handlePerformRename = async (newTitle: string, newIcon?: string) => {
    if (!renameTarget) {
      return;
    }

    await updateHistoryItemMeta(renameTarget.id, { title: newTitle.trim(), icon: newIcon });
    await refreshHistory();

    if (sourceHistoryId === renameTarget.id) {
      setTitle(newTitle.trim());
      useTranscriptStore.getState().setIcon(newIcon || null);
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

    const currentHistoryId = useTranscriptStore.getState().sourceHistoryId;
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

  return (
    <div className={`projects-workbench ${selectedItem ? 'with-detail' : ''}`}>
      <ProjectsRail
        browseProjectId={browseState.browseProjectId}
        historyItemsCount={historyItems.length}
        inboxCount={browseState.itemCounts.get(null) || 0}
        isAllItemsScope={browseState.isAllItemsScope}
        isInboxScope={browseState.isInboxScope}
        itemCounts={browseState.itemCounts}
        onOpenCreateModal={() => setIsCreateModalOpen(true)}
        onReorderProjects={reorderProjects}
        onSwitchScope={handleSwitchBrowseScope}
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
            filteredResultsCount={browseState.filteredAndSortedItems.length}
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
                scopedItemsCount={browseState.scopedItems.length}
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

          <div className="projects-main-scroll" onScroll={browseState.handleScroll}>
            <ProjectsResults
              activeSearchResultId={browseState.activeSearchResultId}
              browseProject={browseState.browseProject}
              filteredAndSortedItems={browseState.filteredAndSortedItems}
              handleOpenItem={handleOpenItem}
              isHistoryInteractionLocked={isLiveDraftSessionLocked}
              isAllItemsScope={browseState.isAllItemsScope}
              isHistoryLoading={isHistoryLoading}
              isSelectionMode={selectionState.isSelectionMode}
              onDeleteHistoryItem={handleDeleteHistoryItem}
              onRenameHistoryItem={handleRenameHistoryItem}
              onToggleSelection={selectionState.toggleSelection}
              resetBrowseState={browseState.resetBrowseState}
              scopedItems={browseState.scopedItems}
              searchMatchByItemId={browseState.searchMatchByItemId}
              searchQuery={browseState.searchQuery}
              selectedHistoryId={effectiveSelectedHistoryId}
              selectedIds={selectionState.selectedIds}
              t={t}
              viewMode={viewMode}
            />
          </div>
        </section>
      )}

      {selectedItem && (
        <aside className="projects-detail-pane">
          <TranscriptWorkbench
            onClose={clearOpenedItem}
            title={selectedItem.title}
            defaultIconType={selectedItem.type}
          />
        </aside>
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
          return await generateAiTitleForHistoryItem(item.transcriptPath);
        }}
      />
    </div>
  );
}

export default ProjectsView;
