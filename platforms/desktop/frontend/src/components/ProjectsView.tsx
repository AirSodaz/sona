import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { historyService } from '../services/historyService';
import { historyQueryWorkspace } from '../services/tauri/history';
import { storageOpenPath } from '../services/tauri/storage';
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
import type { ProjectRecord } from '../types/project';
import { logger } from '../utils/logger';
import { useContextMenu } from './context-menu/useContextMenu';
import { useHistoryItemContextMenu } from './projects/context-menu/useHistoryItemContextMenu';
import { useProjectRailContextMenu } from './projects/context-menu/useProjectRailContextMenu';
import { useProjectSettingsDraft } from './projects/hooks/useProjectSettingsDraft';
import { useWorkspaceBrowseState } from './projects/hooks/useWorkspaceBrowseState';
import { useWorkspaceSelectionState } from './projects/hooks/useWorkspaceSelectionState';
import { ProjectAssignmentModal } from './projects/ProjectAssignmentModal';
import { ProjectCreateModal } from './projects/ProjectCreateModal';
import { ProjectDeleteModal } from './projects/ProjectDeleteModal';
import { ProjectSettingsModal } from './projects/ProjectSettingsModal';
import { ProjectsHeader } from './projects/ProjectsHeader';
import { ProjectsRail } from './projects/ProjectsRail';
import { ProjectsResults } from './projects/ProjectsResults';
import { ProjectsSelectionBar } from './projects/ProjectsSelectionBar';
import { ProjectsToolbar } from './projects/ProjectsToolbar';
import type { RenameTarget } from './projects/types';
import { RenameModal } from './RenameModal';

interface ProjectsViewProps {
  isActive?: boolean;
  onOpenAutomationSettings?: (tagId: string) => void;
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
  isActive: boolean
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
  const [newProjectIcon, setNewProjectIcon] = useState('');
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedHistoryId, setSelectedHistoryId] = useState<string | null>(sourceHistoryId);
  const [renameTarget, setRenameTarget] = useState<RenameTarget | null>(null);
  const [projectToDelete, setProjectToDelete] = useState<ProjectRecord | null>(null);
  const [projectAssignmentIds, setProjectAssignmentIds] = useState<string[]>([]);
  const filterMenuRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const workspaceMenuSnapshotRef = useRef<WorkspaceMenuSnapshot | null>(null);
  const browseScopeRef = useRef('inbox');
  const activeHistoryItem = useMemo(
    () => historyItems.find((item) => item.id === sourceHistoryId) || null,
    [historyItems, sourceHistoryId]
  );
  const isLiveDraftSessionLocked =
    isRecording && !!activeHistoryItem && isLiveRecordDraftHistoryItem(activeHistoryItem);

  useEffect(() => {
    void loadHistoryItems();
  }, [loadHistoryItems]);

  const clearOpenedItem = useCallback(() => {
    setSelectedHistoryId(null);
    clearActiveTranscriptSession({ clearAudio: true });
  }, []);

  const handleOpenItem = useCallback(
    async (item: HistoryItemType) => {
      if (item.deletedAt != null) {
        return;
      }
      const initialItem = useHistoryStore
        .getState()
        .items.find((candidate) => candidate.id === item.id);
      const initialLockState = getLiveDraftLockState();
      if (
        !initialItem ||
        (initialLockState.isLocked && item.id !== initialLockState.sourceHistoryId)
      ) {
        return;
      }

      try {
        let segments = await historyService.loadTranscript(item.id);
        const url = await historyService.getAudioUrl(item.id);

        const latestItem = useHistoryStore
          .getState()
          .items.find((candidate) => candidate.id === item.id);
        const latestLockState = getLiveDraftLockState();
        if (
          !latestItem ||
          (latestLockState.isLocked && item.id !== latestLockState.sourceHistoryId)
        ) {
          return;
        }

        if (!segments && !url) {
          await showError({
            code: 'history.missing_files_deleted',
            messageKey: 'errors.history.missing_files_deleted',
            showCause: false,
          });

          const deletableItem = useHistoryStore
            .getState()
            .items.find((candidate) => candidate.id === item.id);
          const deleteLockState = getLiveDraftLockState();
          if (
            !deletableItem ||
            (deleteLockState.isLocked && item.id === deleteLockState.sourceHistoryId)
          ) {
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
    },
    [showError]
  );

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
      isActive
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

  const itemMatchesBrowseScope = useCallback(
    (item: HistoryItemType) => {
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
      return (item.tagIds ?? (item.projectId ? [item.projectId] : [])).includes(
        browseState.browseProjectId || ''
      );
    },
    [
      browseState.browseProjectId,
      browseState.isAllItemsScope,
      browseState.isInboxScope,
      browseState.isTrashScope,
    ]
  );

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
      (item) => item.id === selectedHistoryId && itemMatchesBrowseScope(item)
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
    [effectiveSelectedHistoryId, historyItems]
  );
  const tagAssignmentItems = useMemo(() => {
    const candidates = new Map<string, HistoryItemType>();
    historyItems.forEach((item) => candidates.set(item.id, item));
    browseState.filteredAndSortedItems.forEach((item) => candidates.set(item.id, item));
    return projectAssignmentIds
      .map((id) => candidates.get(id))
      .filter((item): item is HistoryItemType => !!item && item.deletedAt == null);
  }, [browseState.filteredAndSortedItems, historyItems, projectAssignmentIds]);

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
      if (
        sessionState.sourceHistoryId ||
        sessionState.segments.length > 0 ||
        playbackState.audioUrl
      ) {
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
      initialLockState.isLocked ||
      (isProjectScope &&
        !useProjectStore.getState().projects.some((project) => project.id === nextScope))
    ) {
      return false;
    }

    const shouldDiscard = await projectSettingsDraft.confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return false;
    }

    const latestLockState = getLiveDraftLockState();
    if (
      latestLockState.isLocked ||
      (isProjectScope &&
        !useProjectStore.getState().projects.some((project) => project.id === nextScope))
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
      await useProjectStore
        .getState()
        .setActiveProjectId(nextScope === 'untagged' ? null : nextScope);
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

  const handleDeleteHistoryItem = async (id: string) => {
    const initialItem =
      useHistoryStore.getState().items.find((item) => item.id === id) ??
      browseState.filteredAndSortedItems.find((item) => item.id === id);
    const initialLockState = getLiveDraftLockState();
    if (!initialItem || (initialLockState.isLocked && id === initialLockState.sourceHistoryId)) {
      return;
    }

    const isTrashItem = initialItem.deletedAt != null;
    const confirmed = await confirm(
      isTrashItem
        ? t('history.purge_confirm', {
            defaultValue: 'Permanently delete this item? This cannot be undone.',
          })
        : t('history.trash_confirm', { defaultValue: 'Move this item to Trash?' }),
      {
        title: isTrashItem
          ? t('history.purge_title', { defaultValue: 'Delete Permanently' })
          : t('history.trash_title', { defaultValue: 'Move to Trash' }),
        confirmLabel: isTrashItem
          ? t('history.delete_permanently', { defaultValue: 'Delete Permanently' })
          : t('history.move_to_trash', { defaultValue: 'Move to Trash' }),
        variant: 'error',
      }
    );

    if (!confirmed) {
      return;
    }

    const latestItem =
      useHistoryStore.getState().items.find((item) => item.id === id) ??
      browseState.filteredAndSortedItems.find((item) => item.id === id);
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
    const item =
      useHistoryStore.getState().items.find((historyItem) => historyItem.id === id) ??
      browseState.filteredAndSortedItems.find((historyItem) => historyItem.id === id);
    const lockState = getLiveDraftLockState();
    if (!item || (lockState.isLocked && id === lockState.sourceHistoryId)) {
      return;
    }

    setRenameTarget({ id, title: item.title, icon: item.icon, type: item.type });
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
    await useHistoryStore
      .getState()
      .updateItemMeta(targetId, { title: trimmedTitle, icon: newIcon });
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

    const project = await createProject({
      name: newProjectName.trim(),
      description: newProjectDescription.trim(),
      color: newProjectColor,
      icon: newProjectIcon,
    });

    if (!project) {
      return;
    }

    setNewProjectName('');
    setNewProjectDescription('');
    setNewProjectColor('#64748b');
    setNewProjectIcon('');
    setIsCreateModalOpen(false);
    browseState.setBrowseScope(project.id);
    await setActiveProjectId(project.id);
  };

  const handleSaveProject = async () => {
    if (!browseState.browseProject) {
      return;
    }

    await updateProject(browseState.browseProject.id, {
      name: projectSettingsDraft.draftName.trim() || browseState.browseProject.name,
      description: projectSettingsDraft.draftDescription,
      icon: projectSettingsDraft.draftIcon,
      color: projectSettingsDraft.draftColor,
      pipeline: projectSettingsDraft.draftPipeline,
    });
    projectSettingsDraft.setIsSettingsOpen(false);
  };

  const handleDeleteProject = async () => {
    if (!browseState.browseProject) {
      return;
    }
    const project = browseState.browseProject;
    const shouldDiscard = await projectSettingsDraft.confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (projectSettingsDraft.isSettingsOpen) {
      projectSettingsDraft.discardProjectSettingsDraft(project);
    }
    setProjectToDelete(project);
  };

  const handleToggleSelectionMode = () => {
    if (isLiveDraftSessionLocked) {
      return;
    }
    browseState.setIsFilterMenuOpen(false);
    selectionState.toggleSelectionMode();
  };

  const handleDeleteHistoryItems = async (ids: string[]) => {
    if (ids.length === 0) {
      return;
    }

    const isTrashScope = browseState.isTrashScope;
    const confirmed = await confirm(
      isTrashScope
        ? t('history.purge_bulk_confirm', {
            count: ids.length,
            defaultValue: `Permanently delete ${ids.length} items? This cannot be undone.`,
          })
        : t('history.trash_bulk_confirm', {
            count: ids.length,
            defaultValue: `Move ${ids.length} items to Trash?`,
          }),
      {
        title: isTrashScope
          ? t('history.purge_title', { defaultValue: 'Delete Permanently' })
          : t('history.trash_title', { defaultValue: 'Move to Trash' }),
        confirmLabel: isTrashScope
          ? t('history.delete_permanently', { defaultValue: 'Delete Permanently' })
          : t('history.move_to_trash', { defaultValue: 'Move to Trash' }),
        variant: 'error',
      }
    );

    if (!confirmed) {
      return;
    }

    if (isTrashScope) {
      await historyService.purgeRecordings(ids);
    } else {
      await deleteHistoryItems(ids);
    }
    await refreshHistory();
    selectionState.clearSelection();
  };

  const handleDeleteSelected = async () => {
    await handleDeleteHistoryItems(selectionState.selectedIds);
  };

  const handleCopyTranscript = async (item: HistoryItemType) => {
    try {
      const segments = await historyService.loadTranscript(item.id);
      const text =
        segments
          ?.map((s) => s.text)
          .join('\n')
          .trim() ||
        item.previewText ||
        item.title;
      if (text) {
        await navigator.clipboard.writeText(text);
      }
    } catch (error) {
      logger.error('Failed to copy transcript:', error);
    }
  };

  const handleCopyTranscripts = async (items: HistoryItemType[]) => {
    try {
      const texts: string[] = [];
      for (const item of items) {
        const segments = await historyService.loadTranscript(item.id);
        const text =
          segments
            ?.map((s) => s.text)
            .join('\n')
            .trim() ||
          item.previewText ||
          item.title;
        if (text) {
          texts.push(`=== ${item.title} ===\n${text}`);
        }
      }
      if (texts.length > 0) {
        await navigator.clipboard.writeText(texts.join('\n\n'));
      }
    } catch (error) {
      logger.error('Failed to copy transcripts:', error);
    }
  };

  const handleCopyTitle = async (item: HistoryItemType) => {
    try {
      await navigator.clipboard.writeText(item.title);
    } catch (error) {
      logger.error('Failed to copy title:', error);
    }
  };

  const handleShowInFolder = async (item: HistoryItemType) => {
    try {
      const fullPath = await historyService.getAudioAbsolutePath(item.id);
      if (fullPath) {
        await storageOpenPath(fullPath);
      } else {
        await historyService.openHistoryFolder();
      }
    } catch (error) {
      logger.error('Failed to reveal file:', error);
      await historyService.openHistoryFolder();
    }
  };

  const handleMoveToInbox = async (ids: string[]) => {
    await useProjectStore.getState().moveItemsToProject(ids, null);
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
      }
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
  const { openRailContextMenu } = useProjectRailContextMenu({
    activeProjectId,
    browseScope: browseState.browseScope,
    isLockedLiveDraft: isLiveDraftSessionLocked,
    onDeleteProject: (project) => {
      setProjectToDelete(project);
    },
    onEmptyTrash: handleEmptyTrash,
    onOpenCreateModal: () => setIsCreateModalOpen(true),
    onOpenProjectSettings: (id) => void handleOpenProjectSettings(id),
    onSetActiveProjectId: (id) => void setActiveProjectId(id),
    onSwitchScope: handleSwitchBrowseScope,
    openContextMenu,
    projects,
    t,
    trashCount: browseState.itemCounts.get('trash') || 0,
    onMenuOpened: (contextId) => {
      workspaceMenuSnapshotRef.current = {
        contextId,
        revision: createWorkspaceMenuRevision(
          contextId,
          browseScopeRef.current,
          viewMode,
          selectionState.isSelectionMode,
          isActive
        ),
      };
    },
  });

  const { openHistoryContextMenu } = useHistoryItemContextMenu({
    getItemById: (id) =>
      useHistoryStore.getState().items.find((historyItem) => historyItem.id === id) ??
      browseState.filteredAndSortedItems.find((historyItem) => historyItem.id === id),
    isAllSelected: selectionState.isAllSelected,
    isLockedLiveDraft: (id) => {
      const lockState = getLiveDraftLockState();
      return lockState.isLocked && id === lockState.sourceHistoryId;
    },
    isOpenDisabled: (id) => {
      const lockState = getLiveDraftLockState();
      return lockState.isLocked && id !== lockState.sourceHistoryId;
    },
    isTrashScope: browseState.isTrashScope,
    onAssignProject: (ids) => {
      setProjectAssignmentIds(ids);
    },
    onClearSelection: selectionState.clearSelection,
    onCopyTitle: handleCopyTitle,
    onCopyTranscript: handleCopyTranscript,
    onCopyTranscripts: handleCopyTranscripts,
    onDeleteHistoryItem: (id) => {
      void handleDeleteHistoryItem(id);
    },
    onDeleteHistoryItems: (ids) => {
      void handleDeleteHistoryItems(ids);
    },
    onMoveToInbox: (ids) => {
      void handleMoveToInbox(ids);
    },
    onOpenItem: (item) => {
      void handleOpenItem(item);
    },
    onRenameHistoryItem: (id) => {
      void handleRenameHistoryItem(id);
    },
    onRestoreHistoryItems: (ids) => {
      void handleRestoreHistoryItems(ids);
    },
    onSelectAllVisible: selectionState.handleToggleSelectAll,
    onShowInFolder: (item) => {
      void handleShowInFolder(item);
    },
    openContextMenu,
    selectedIds: selectionState.selectedIds,
    t,
    onMenuOpened: (contextId) => {
      workspaceMenuSnapshotRef.current = {
        contextId,
        revision: createWorkspaceMenuRevision(
          contextId,
          browseScopeRef.current,
          viewMode,
          selectionState.isSelectionMode,
          isActive
        ),
      };
    },
  });

  if (!isActive) {
    return (
      <div className="projects-workbench" data-projects-inactive="true" hidden aria-hidden="true" />
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
        onOpenProjectContextMenu={openRailContextMenu}
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
              onAssignProject={() => setProjectAssignmentIds(selectionState.selectedIds)}
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
            onOpenHistoryContextMenu={openHistoryContextMenu}
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
        icon={newProjectIcon}
        onNameChange={setNewProjectName}
        onDescriptionChange={setNewProjectDescription}
        onColorChange={setNewProjectColor}
        onIconChange={setNewProjectIcon}
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
        draftPipeline={projectSettingsDraft.draftPipeline}
        onPipelineChange={projectSettingsDraft.setDraftPipeline}
        onClose={projectSettingsDraft.handleRequestCloseProjectSettings}
        onSave={handleSaveProject}
        onDelete={handleDeleteProject}
        onNameChange={projectSettingsDraft.setDraftName}
        onDescriptionChange={projectSettingsDraft.setDraftDescription}
        onIconChange={projectSettingsDraft.setDraftIcon}
        onColorChange={projectSettingsDraft.setDraftColor}
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

      <ProjectAssignmentModal
        isOpen={projectAssignmentIds.length > 0}
        items={tagAssignmentItems}
        projects={projects}
        onClose={() => setProjectAssignmentIds([])}
        onApply={async (projectId) => {
          await useProjectStore.getState().moveItemsToProject(projectAssignmentIds, projectId);
          await refreshHistory();
          selectionState.clearSelection();
        }}
      />

      <ProjectDeleteModal
        isOpen={!!projectToDelete}
        project={projectToDelete}
        itemCount={
          projectToDelete
            ? historyItems.filter(
                (item) => !item.deletedAt && item.projectId === projectToDelete.id
              ).length
            : 0
        }
        onClose={() => setProjectToDelete(null)}
        onConfirm={async (cascadeAction) => {
          if (!projectToDelete) return;
          const pid = projectToDelete.id;
          clearOpenedItem();
          browseState.setBrowseScope('untagged');
          await deleteProject(pid, cascadeAction);
          await refreshHistory();
          setProjectToDelete(null);
        }}
      />
    </div>
  );
}

export default ProjectsView;
