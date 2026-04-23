import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AudioPlayer } from './AudioPlayer';
import { Checkbox } from './Checkbox';
import { Dropdown } from './Dropdown';
import { ErrorBoundary } from './ErrorBoundary';
import { TranscriptEditor } from './TranscriptEditor';
import { HistoryItem } from './history/HistoryItem';
import {
  CloseIcon,
  FolderIcon,
  PlusCircleIcon,
  SettingsIcon,
  XIcon,
} from './Icons';
import { historyService } from '../services/historyService';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { HistoryItem as HistoryItemType } from '../types/history';
import type { ProjectDefaults, ProjectRecord } from '../types/project';

const LANGUAGE_OPTIONS = ['zh', 'en', 'ja', 'ko', 'fr', 'de', 'es'];
const SUMMARY_TEMPLATE_OPTIONS = ['general', 'meeting', 'lecture'] as const;
const POLISH_SCENARIO_OPTIONS = [
  'customer_service',
  'meeting',
  'interview',
  'lecture',
  'podcast',
  'custom',
] as const;

function sortRuleSetIds(ids: string[]): string[] {
  return [...ids].sort();
}

function buildComparableProjectSettings(
  project: ProjectRecord,
  draftName: string,
  draftDescription: string,
  draftDefaults: ProjectDefaults,
) {
  return {
    name: draftName.trim() || project.name,
    description: draftDescription,
    summaryTemplate: draftDefaults.summaryTemplate,
    translationLanguage: draftDefaults.translationLanguage,
    polishScenario: draftDefaults.polishScenario,
    polishContext: draftDefaults.polishContext,
    exportFileNamePrefix: draftDefaults.exportFileNamePrefix,
    enabledTextReplacementSetIds: sortRuleSetIds(draftDefaults.enabledTextReplacementSetIds),
    enabledHotwordSetIds: sortRuleSetIds(draftDefaults.enabledHotwordSetIds),
  };
}

function buildSavedProjectSettings(project: ProjectRecord) {
  return {
    name: project.name,
    description: project.description,
    summaryTemplate: project.defaults.summaryTemplate,
    translationLanguage: project.defaults.translationLanguage,
    polishScenario: project.defaults.polishScenario,
    polishContext: project.defaults.polishContext,
    exportFileNamePrefix: project.defaults.exportFileNamePrefix,
    enabledTextReplacementSetIds: sortRuleSetIds(project.defaults.enabledTextReplacementSetIds),
    enabledHotwordSetIds: sortRuleSetIds(project.defaults.enabledHotwordSetIds),
  };
}

interface ProjectCreateModalProps {
  isOpen: boolean;
  name: string;
  description: string;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onClose: () => void;
  onCreate: () => void;
}

function ProjectCreateModal({
  isOpen,
  name,
  description,
  onNameChange,
  onDescriptionChange,
  onClose,
  onCreate,
}: ProjectCreateModalProps): React.JSX.Element | null {
  const { t } = useTranslation();

  if (!isOpen) {
    return null;
  }

  return (
    <div className="projects-overlay" onClick={onClose}>
      <div
        className="projects-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-create-title"
      >
        <div className="projects-modal-header">
          <div>
            <div className="projects-modal-eyebrow">
              {t('projects.create_project', { defaultValue: 'Create Project' })}
            </div>
            <h3 id="project-create-title">
              {t('projects.new_project_title', { defaultValue: 'New Project' })}
            </h3>
          </div>
          <button
            type="button"
            className="btn btn-icon"
            onClick={onClose}
            aria-label={t('common.close')}
          >
            <XIcon />
          </button>
        </div>

        <div className="projects-modal-body">
          <div className="projects-field">
            <label htmlFor="project-create-name">
              {t('projects.project_name', { defaultValue: 'Project Name' })}
            </label>
            <input
              id="project-create-name"
              type="text"
              value={name}
              onChange={(event) => onNameChange(event.target.value)}
              placeholder={t('projects.new_project_name', { defaultValue: 'Project name' })}
            />
          </div>

          <div className="projects-field">
            <label htmlFor="project-create-description">
              {t('projects.project_description', { defaultValue: 'Description' })}
            </label>
            <textarea
              id="project-create-description"
              value={description}
              onChange={(event) => onDescriptionChange(event.target.value)}
              placeholder={t('projects.new_project_description', { defaultValue: 'Short description' })}
            />
          </div>
        </div>

        <div className="projects-modal-footer">
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void onCreate()}
            disabled={!name.trim()}
          >
            {t('projects.create_action', { defaultValue: 'Create Project' })}
          </button>
        </div>
      </div>
    </div>
  );
}

interface ProjectSettingsDrawerProps {
  isOpen: boolean;
  project: ProjectRecord | null;
  draftName: string;
  draftDescription: string;
  draftDefaults: ProjectDefaults | null;
  globalConfig: ReturnType<typeof useConfigStore.getState>['config'];
  onClose: () => void;
  onSave: () => void;
  onDelete: () => void;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onDefaultsChange: (defaults: ProjectDefaults) => void;
}

function ProjectSettingsDrawer({
  isOpen,
  project,
  draftName,
  draftDescription,
  draftDefaults,
  globalConfig,
  onClose,
  onSave,
  onDelete,
  onNameChange,
  onDescriptionChange,
  onDefaultsChange,
}: ProjectSettingsDrawerProps): React.JSX.Element | null {
  const { t } = useTranslation();

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') {
        return;
      }

      event.preventDefault();
      void onClose();
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen || !project || !draftDefaults) {
    return null;
  }

  const summaryTemplateOptions = SUMMARY_TEMPLATE_OPTIONS.map((template) => ({
    value: template,
    label: t(`summary.templates.${template}`),
  }));

  const languageOptions = LANGUAGE_OPTIONS.map((language) => ({
    value: language,
    label: t(`translation.languages.${language}`),
  }));

  const polishScenarioOptions = POLISH_SCENARIO_OPTIONS.map((scenario) => ({
    value: scenario,
    label: t(`polish.scenarios.${scenario}`),
  }));

  const toggleRuleSetId = (
    key: 'enabledTextReplacementSetIds' | 'enabledHotwordSetIds',
    id: string,
  ) => {
    const current = new Set(draftDefaults[key]);
    if (current.has(id)) {
      current.delete(id);
    } else {
      current.add(id);
    }

    onDefaultsChange({
      ...draftDefaults,
      [key]: Array.from(current),
    });
  };

  return (
    <div className="projects-drawer-shell">
      <div className="projects-drawer-backdrop" onClick={onClose} />

      <aside
        className="projects-settings-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-settings-title"
      >
        <div className="projects-settings-header">
          <div>
            <div className="projects-modal-eyebrow">
              {t('projects.project_settings', { defaultValue: 'Project Settings' })}
            </div>
            <h3 id="project-settings-title">
              {t('projects.project_settings_title', { defaultValue: 'Edit Project Defaults' })}
            </h3>
            <p>
              {t('projects.project_settings_hint', {
                defaultValue: 'These defaults apply whenever you work inside this project.',
              })}
            </p>
          </div>

          <button
            type="button"
            className="btn btn-icon"
            onClick={onClose}
            aria-label={t('common.close')}
          >
            <XIcon />
          </button>
        </div>

        <div className="projects-settings-body">
          <div className="projects-field">
            <label htmlFor="project-settings-name">
              {t('projects.project_name', { defaultValue: 'Project Name' })}
            </label>
            <input
              id="project-settings-name"
              value={draftName}
              onChange={(event) => onNameChange(event.target.value)}
            />
          </div>

          <div className="projects-field">
            <label htmlFor="project-settings-description">
              {t('projects.project_description', { defaultValue: 'Description' })}
            </label>
            <textarea
              id="project-settings-description"
              value={draftDescription}
              onChange={(event) => onDescriptionChange(event.target.value)}
            />
          </div>

          <div className="projects-settings-grid">
            <div className="projects-field">
              <label>
                {t('projects.summary_template', { defaultValue: 'Default Summary Template' })}
              </label>
              <Dropdown
                value={draftDefaults.summaryTemplate}
                onChange={(value) => onDefaultsChange({
                  ...draftDefaults,
                  summaryTemplate: value as ProjectDefaults['summaryTemplate'],
                })}
                options={summaryTemplateOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div className="projects-field">
              <label>
                {t('projects.translation_language', { defaultValue: 'Default Translation Language' })}
              </label>
              <Dropdown
                value={draftDefaults.translationLanguage}
                onChange={(value) => onDefaultsChange({
                  ...draftDefaults,
                  translationLanguage: value,
                })}
                options={languageOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div className="projects-field">
              <label>
                {t('projects.polish_scenario', { defaultValue: 'Default Polish Scenario' })}
              </label>
              <Dropdown
                value={draftDefaults.polishScenario}
                onChange={(value) => onDefaultsChange({
                  ...draftDefaults,
                  polishScenario: value,
                })}
                options={polishScenarioOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div className="projects-field">
              <label>
                {t('projects.export_prefix', { defaultValue: 'Export Filename Prefix' })}
              </label>
              <input
                value={draftDefaults.exportFileNamePrefix}
                onChange={(event) => onDefaultsChange({
                  ...draftDefaults,
                  exportFileNamePrefix: event.target.value,
                })}
              />
            </div>
          </div>

          {(draftDefaults.polishScenario === 'custom' || !draftDefaults.polishScenario) && (
            <div className="projects-field">
              <label htmlFor="project-settings-polish-context">
                {t('projects.polish_context', { defaultValue: 'Default Polish Context' })}
              </label>
              <textarea
                id="project-settings-polish-context"
                value={draftDefaults.polishContext}
                onChange={(event) => onDefaultsChange({
                  ...draftDefaults,
                  polishContext: event.target.value,
                })}
              />
            </div>
          )}

          <div className="projects-settings-grid">
            <div className="projects-settings-card">
              <div className="projects-settings-card-title">
                {t('projects.text_replacement_sets', { defaultValue: 'Enabled Text Replacement Sets' })}
              </div>
              <div className="projects-settings-card-list">
                {(globalConfig.textReplacementSets || []).length === 0 ? (
                  <span className="projects-settings-empty-copy">
                    {t('projects.no_text_replacement_sets', {
                      defaultValue: 'No global text replacement sets yet.',
                    })}
                  </span>
                ) : (
                  (globalConfig.textReplacementSets || []).map((set) => (
                    <Checkbox
                      key={set.id}
                      checked={draftDefaults.enabledTextReplacementSetIds.includes(set.id)}
                      onChange={() => toggleRuleSetId('enabledTextReplacementSetIds', set.id)}
                      label={set.name}
                    />
                  ))
                )}
              </div>
            </div>

            <div className="projects-settings-card">
              <div className="projects-settings-card-title">
                {t('projects.hotword_sets', { defaultValue: 'Enabled Hotword Sets' })}
              </div>
              <div className="projects-settings-card-list">
                {(globalConfig.hotwordSets || []).length === 0 ? (
                  <span className="projects-settings-empty-copy">
                    {t('projects.no_hotword_sets', {
                      defaultValue: 'No global hotword sets yet.',
                    })}
                  </span>
                ) : (
                  (globalConfig.hotwordSets || []).map((set) => (
                    <Checkbox
                      key={set.id}
                      checked={draftDefaults.enabledHotwordSetIds.includes(set.id)}
                      onChange={() => toggleRuleSetId('enabledHotwordSetIds', set.id)}
                      label={set.name}
                    />
                  ))
                )}
              </div>
            </div>
          </div>
        </div>

        <div className="projects-settings-footer">
          <button type="button" className="btn btn-danger" onClick={() => void onDelete()}>
            {t('projects.delete_project', { defaultValue: 'Delete Project' })}
          </button>

          <div className="projects-settings-footer-actions">
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button type="button" className="btn btn-primary" onClick={() => void onSave()}>
              {t('common.save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      </aside>
    </div>
  );
}

export function ProjectsView(): React.JSX.Element {
  const { t } = useTranslation();
  const projects = useProjectStore((state) => state.projects);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => (
    state.projects.find((item) => item.id === state.activeProjectId) || null
  ));
  const createProject = useProjectStore((state) => state.createProject);
  const updateProject = useProjectStore((state) => state.updateProject);
  const deleteProject = useProjectStore((state) => state.deleteProject);
  const setActiveProjectId = useProjectStore((state) => state.setActiveProjectId);
  const assignHistoryItems = useProjectStore((state) => state.assignHistoryItems);

  const historyItems = useHistoryStore((state) => state.items);
  const isHistoryLoading = useHistoryStore((state) => state.isLoading);
  const loadHistoryItems = useHistoryStore((state) => state.loadItems);
  const refreshHistory = useHistoryStore((state) => state.refresh);
  const deleteHistoryItem = useHistoryStore((state) => state.deleteItem);

  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const audioUrl = useTranscriptStore((state) => state.audioUrl);
  const clearSegments = useTranscriptStore((state) => state.clearSegments);
  const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
  const setMode = useTranscriptStore((state) => state.setMode);

  const globalConfig = useConfigStore((state) => state.config);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectDescription, setNewProjectDescription] = useState('');
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [draftDefaults, setDraftDefaults] = useState<ProjectDefaults | null>(null);
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [moveTarget, setMoveTarget] = useState('inbox');
  const [selectedHistoryId, setSelectedHistoryId] = useState<string | null>(sourceHistoryId);

  const resetProjectSettingsDraft = useCallback((project: ProjectRecord | null = activeProject) => {
    if (!project) {
      setDraftName('');
      setDraftDescription('');
      setDraftDefaults(null);
      return;
    }

    setDraftName(project.name);
    setDraftDescription(project.description);
    setDraftDefaults(project.defaults);
  }, [activeProject]);

  useEffect(() => {
    void loadHistoryItems();
  }, [loadHistoryItems]);

  useEffect(() => {
    if (!activeProject) {
      resetProjectSettingsDraft(null);
      setIsSettingsOpen(false);
      return;
    }

    resetProjectSettingsDraft(activeProject);
  }, [activeProject, resetProjectSettingsDraft]);

  useEffect(() => {
    if (activeProjectId) {
      setMoveTarget('inbox');
      return;
    }

    setMoveTarget(projects[0]?.id || 'inbox');
  }, [activeProjectId, projects]);

  const clearOpenedItem = useCallback(() => {
    setSelectedHistoryId(null);
    clearSegments();
    setAudioUrl(null);
  }, [clearSegments, setAudioUrl]);

  const scopedItems = useMemo(
    () => historyItems.filter((item) => (activeProjectId ? item.projectId === activeProjectId : item.projectId === null)),
    [historyItems, activeProjectId],
  );

  const selectedItem = useMemo(
    () => scopedItems.find((item) => item.id === selectedHistoryId) || null,
    [scopedItems, selectedHistoryId],
  );

  const itemCounts = useMemo(() => {
    const counts = new Map<string | null, number>();
    historyItems.forEach((item) => {
      const key = item.projectId ?? null;
      counts.set(key, (counts.get(key) || 0) + 1);
    });
    return counts;
  }, [historyItems]);

  useEffect(() => {
    if (selectedHistoryId && !selectedItem) {
      clearOpenedItem();
      return;
    }

    if (!selectedHistoryId && sourceHistoryId && scopedItems.some((item) => item.id === sourceHistoryId)) {
      setSelectedHistoryId(sourceHistoryId);
      return;
    }

    if (!selectedHistoryId) {
      const transcriptState = useTranscriptStore.getState();
      if (transcriptState.sourceHistoryId || transcriptState.segments.length > 0 || transcriptState.audioUrl) {
        transcriptState.clearSegments();
        transcriptState.setAudioUrl(null);
      }
    }
  }, [clearOpenedItem, scopedItems, selectedHistoryId, selectedItem, sourceHistoryId]);

  const moveOptions = useMemo(
    () => [
      { value: 'inbox', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
      ...projects.map((project) => ({ value: project.id, label: project.name })),
    ],
    [projects, t],
  );

  const isProjectSettingsDirty = useMemo(() => {
    if (!activeProject || !draftDefaults) {
      return false;
    }

    const currentDraft = buildComparableProjectSettings(
      activeProject,
      draftName,
      draftDescription,
      draftDefaults,
    );
    const savedProject = buildSavedProjectSettings(activeProject);

    return JSON.stringify(currentDraft) !== JSON.stringify(savedProject);
  }, [activeProject, draftDefaults, draftDescription, draftName]);

  const confirmDiscardProjectSettingsChanges = useCallback(async () => {
    if (!isSettingsOpen || !isProjectSettingsDirty) {
      return true;
    }

    return confirm(
      t('projects.discard_changes_confirm', {
        defaultValue: 'You have unsaved project settings changes. Discard them?',
      }),
      {
        title: t('projects.discard_changes_title', {
          defaultValue: 'Discard project changes?',
        }),
        confirmLabel: t('projects.discard_changes_action', {
          defaultValue: 'Discard',
        }),
        cancelLabel: t('projects.keep_editing_action', {
          defaultValue: 'Keep editing',
        }),
        variant: 'warning',
      },
    );
  }, [confirm, isProjectSettingsDirty, isSettingsOpen, t]);

  const discardProjectSettingsDraft = useCallback((project: ProjectRecord | null = activeProject) => {
    resetProjectSettingsDraft(project);
    setIsSettingsOpen(false);
  }, [activeProject, resetProjectSettingsDraft]);

  const handleRequestCloseProjectSettings = useCallback(async () => {
    const shouldDiscard = await confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    discardProjectSettingsDraft();
  }, [confirmDiscardProjectSettingsChanges, discardProjectSettingsDraft]);

  const handleSwitchProject = async (projectId: string | null) => {
    const shouldDiscard = await confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (isSettingsOpen) {
      discardProjectSettingsDraft();
    }

    setIsSelectionMode(false);
    setSelectedIds([]);
    await setActiveProjectId(projectId);
  };

  const handleOpenItem = async (item: HistoryItemType) => {
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

      useTranscriptStore.getState().loadTranscript(segments, item.id);
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
  };

  const handleDeleteHistoryItem = async (event: React.MouseEvent, id: string) => {
    event.stopPropagation();

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
    await setActiveProjectId(project.id);
  };

  const handleSaveProject = async () => {
    if (!activeProject || !draftDefaults) {
      return;
    }

    await updateProject(activeProject.id, {
      name: draftName.trim() || activeProject.name,
      description: draftDescription,
      defaults: draftDefaults,
    });
    setIsSettingsOpen(false);
  };

  const handleDeleteProject = async () => {
    if (!activeProject) {
      return;
    }

    const shouldDiscard = await confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (isSettingsOpen) {
      discardProjectSettingsDraft(activeProject);
    }

    const confirmed = await confirm(
      t('projects.delete_confirm', {
        defaultValue: `Delete ${activeProject.name} and move its items back to Inbox?`,
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
    await deleteProject(activeProject.id);
    await refreshHistory();
  };

  const toggleSelection = (id: string) => {
    setSelectedIds((current) => (
      current.includes(id) ? current.filter((itemId) => itemId !== id) : [...current, id]
    ));
  };

  const handleMoveSelected = async () => {
    if (selectedIds.length === 0) {
      return;
    }

    const targetProjectId = moveTarget === 'inbox' ? null : moveTarget;
    await assignHistoryItems(selectedIds, targetProjectId);
    await refreshHistory();

    const currentHistoryId = useTranscriptStore.getState().sourceHistoryId;
    if (currentHistoryId && selectedIds.includes(currentHistoryId)) {
      await setActiveProjectId(targetProjectId);
    }

    setSelectedIds([]);
    setIsSelectionMode(false);
  };

  return (
    <div className={`projects-workbench ${selectedItem ? 'with-detail' : ''}`}>
      <aside className="projects-rail">
        <div className="projects-rail-header">
          <div>
            <div className="projects-rail-eyebrow">
              {t('panel.projects', { defaultValue: 'Projects' })}
            </div>
            <h2>{t('projects.workspace_label', { defaultValue: 'Workspace' })}</h2>
          </div>

          <button
            type="button"
            className="btn btn-icon"
            onClick={() => setIsCreateModalOpen(true)}
            aria-label={t('projects.new_project_button', { defaultValue: 'New Project' })}
          >
            <PlusCircleIcon width={20} height={20} />
          </button>
        </div>

        <button
          type="button"
          className={`projects-rail-item ${activeProjectId === null ? 'active' : ''}`}
          onClick={() => void handleSwitchProject(null)}
        >
          <div>
            <strong>{t('projects.inbox', { defaultValue: 'Inbox' })}</strong>
            <span>
              {t('projects.inbox_description', {
                defaultValue: 'Inbox collects unassigned recordings and imports.',
              })}
            </span>
          </div>
          <span className="projects-rail-count">{itemCounts.get(null) || 0}</span>
        </button>

        <div className="projects-rail-list">
          {projects.length === 0 && (
            <div className="projects-rail-empty">
              {t('projects.no_projects', { defaultValue: 'No projects yet.' })}
            </div>
          )}

          {projects.map((project) => (
            <button
              key={project.id}
              type="button"
              className={`projects-rail-item ${activeProjectId === project.id ? 'active' : ''}`}
              onClick={() => void handleSwitchProject(project.id)}
            >
              <div>
                <strong>{project.name}</strong>
                <span>{project.description || t('projects.project_description', { defaultValue: 'Description' })}</span>
              </div>
              <span className="projects-rail-count">{itemCounts.get(project.id) || 0}</span>
            </button>
          ))}
        </div>

        <button
          type="button"
          className="btn btn-secondary projects-rail-create"
          onClick={() => setIsCreateModalOpen(true)}
        >
          <PlusCircleIcon width={18} height={18} />
          <span>{t('projects.new_project_button', { defaultValue: 'New Project' })}</span>
        </button>
      </aside>

      <section className="projects-main">
        <div className="projects-main-header">
          <div className="projects-main-heading">
            <div className="projects-main-eyebrow">
              {t('projects.workspace_label', { defaultValue: 'Workspace' })}
            </div>
            <div className="projects-main-title-row">
              <h3>{activeProject?.name || t('projects.inbox', { defaultValue: 'Inbox' })}</h3>
              <span className="projects-main-count">
                {t('projects.items_title', {
                  count: scopedItems.length,
                  defaultValue: `${scopedItems.length} items`,
                })}
              </span>
            </div>
            <p>
              {activeProject?.description || t('projects.inbox_description', {
                defaultValue: 'Inbox collects unassigned recordings and imports.',
              })}
            </p>
          </div>

          <div className="projects-main-actions">
            <button type="button" className="btn btn-secondary" onClick={() => setMode('live')}>
              {t('projects.start_live_record', { defaultValue: 'Start Live Record' })}
            </button>
            <button type="button" className="btn btn-secondary" onClick={() => setMode('batch')}>
              {t('projects.open_batch_import', { defaultValue: 'Open Batch Import' })}
            </button>
            {activeProject && (
              <>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setIsSettingsOpen(true)}
                >
                  <SettingsIcon width={16} height={16} />
                  <span>{t('projects.project_settings', { defaultValue: 'Project Settings' })}</span>
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleSwitchProject(null)}
                >
                  {t('projects.exit_to_inbox', { defaultValue: 'Exit to Inbox' })}
                </button>
              </>
            )}
          </div>
        </div>

        <div className="projects-toolbar">
          <div className="projects-toolbar-copy">
            <strong>{t('projects.items_title', {
              count: scopedItems.length,
              defaultValue: `${scopedItems.length} items`,
            })}</strong>
            <span>
              {selectedItem
                ? t('projects.detail_hint', {
                  defaultValue: 'Editing stays inside Projects until you close this detail pane.',
                })
                : t('projects.select_item_hint', {
                  defaultValue: 'Select an item to open it in the built-in editor pane.',
                })}
            </span>
          </div>

          <div className="projects-toolbar-actions">
            <button
              type="button"
              className={`btn ${isSelectionMode ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => {
                setIsSelectionMode((value) => !value);
                setSelectedIds([]);
              }}
            >
              {isSelectionMode
                ? t('common.cancel', { defaultValue: 'Cancel' })
                : t('common.select', { defaultValue: 'Select' })}
            </button>

            {isSelectionMode && (
              <>
                <Dropdown
                  value={moveTarget}
                  onChange={setMoveTarget}
                  options={moveOptions}
                  style={{ width: '220px' }}
                  aria-label={t('projects.move_target', { defaultValue: 'Move target' })}
                />
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleMoveSelected()}
                  disabled={selectedIds.length === 0 || moveTarget === (activeProjectId || 'inbox')}
                >
                  {t('projects.move_selected', { defaultValue: 'Move Selected' })}
                </button>
              </>
            )}
          </div>
        </div>

        <div className="projects-main-scroll">
          {!selectedItem && scopedItems.length === 0 && !isHistoryLoading && (
            <div className="projects-overview-card">
              <PlusCircleIcon />
              <h4>{t('projects.empty_state', { defaultValue: 'No items in this workspace yet.' })}</h4>
              <p>
                {activeProject
                  ? t('projects.empty_project_hint', {
                    defaultValue: 'Start a live recording or import files to begin building this project.',
                  })
                  : t('projects.empty_inbox_hint', {
                    defaultValue: 'New recordings and imports will arrive here until you move them into a project.',
                  })}
              </p>
            </div>
          )}

          {!selectedItem && scopedItems.length > 0 && (
            <div className="projects-overview-card compact">
              <FolderIcon width={32} height={32} />
              <div>
                <h4>{t('projects.select_item_title', { defaultValue: 'Pick an item to continue' })}</h4>
                <p>
                  {t('projects.select_item_hint', {
                    defaultValue: 'Select an item to open it in the built-in editor pane.',
                  })}
                </p>
              </div>
            </div>
          )}

          {isHistoryLoading && (
            <div className="projects-list-empty">
              {t('history.loading')}
            </div>
          )}

          {!isHistoryLoading && scopedItems.length > 0 && (
            <div className="projects-list">
              {scopedItems.map((item) => (
                <HistoryItem
                  key={item.id}
                  item={item}
                  onLoad={handleOpenItem}
                  onDelete={handleDeleteHistoryItem}
                  isSelectionMode={isSelectionMode}
                  isSelected={isSelectionMode ? selectedIds.includes(item.id) : selectedHistoryId === item.id}
                  onToggleSelection={toggleSelection}
                />
              ))}
            </div>
          )}
        </div>
      </section>

      {selectedItem && (
        <>
          <button
            type="button"
            className="projects-detail-backdrop"
            onClick={clearOpenedItem}
            aria-label={t('projects.close_detail', { defaultValue: 'Close detail' })}
          />

          <aside className="projects-detail-pane">
            <div className="projects-detail-header">
              <div>
                <div className="projects-main-eyebrow">
                  {t('projects.detail_label', { defaultValue: 'Selected Item' })}
                </div>
                <h4>{selectedItem.title}</h4>
              </div>

              <button
                type="button"
                className="btn btn-icon"
                onClick={clearOpenedItem}
                aria-label={t('projects.close_detail', { defaultValue: 'Close detail' })}
              >
                <CloseIcon />
              </button>
            </div>

            <div className="projects-detail-body">
              <ErrorBoundary>
                <TranscriptEditor />
              </ErrorBoundary>
            </div>

            {audioUrl && <AudioPlayer />}
          </aside>
        </>
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

      <ProjectSettingsDrawer
        isOpen={isSettingsOpen}
        project={activeProject}
        draftName={draftName}
        draftDescription={draftDescription}
        draftDefaults={draftDefaults}
        globalConfig={globalConfig}
        onClose={handleRequestCloseProjectSettings}
        onSave={handleSaveProject}
        onDelete={handleDeleteProject}
        onNameChange={setDraftName}
        onDescriptionChange={setDraftDescription}
        onDefaultsChange={setDraftDefaults}
      />
    </div>
  );
}

export default ProjectsView;
