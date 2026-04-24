import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Search, SlidersHorizontal, LayoutGrid, List, LayoutList, CheckSquare, ArrowRight, Trash2, X, ListChecks } from 'lucide-react';
import {
  DndContext,
  closestCorners,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
  DragStartEvent,
  DragOverlay,
  defaultDropAnimationSideEffects,
} from '@dnd-kit/core';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
  useSortable,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { restrictToVerticalAxis, restrictToWindowEdges } from '@dnd-kit/modifiers';
import { HistoryItem } from './history/HistoryItem';
import { TranscriptWorkbench } from './TranscriptWorkbench';
import { Checkbox } from './Checkbox';
import { Dropdown } from './Dropdown';
import {
  FolderIcon,
  PlusCircleIcon,
  SettingsIcon,
  XIcon,
  MicIcon,
  FileTextIcon,
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
const DEFAULT_FILTER_TYPE = 'all';
const DEFAULT_DATE_FILTER = 'all';
const DEFAULT_SORT_ORDER = 'newest';
const ALL_ITEMS_SCOPE = 'all';
const INBOX_SCOPE = 'inbox';
const POLISH_SCENARIO_OPTIONS = [
  'customer_service',
  'meeting',
  'interview',
  'lecture',
  'podcast',
  'custom',
] as const;
type ProjectFilterType = 'all' | 'recording' | 'batch';
type ProjectDateFilter = 'all' | 'today' | 'week' | 'month';
type ProjectSortOrder = 'newest' | 'oldest' | 'duration_desc' | 'duration_asc' | 'title_asc';
type ProjectBrowseScope = typeof ALL_ITEMS_SCOPE | typeof INBOX_SCOPE | string;

function sortRuleSetIds(ids: string[]): string[] {
  return [...ids].sort();
}

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return `${date.toLocaleDateString()} ${date.toLocaleTimeString()}`;
}

function formatSummaryDuration(
  durationInSeconds: number,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  const totalMinutes = Math.max(0, Math.round(durationInSeconds / 60));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours > 0) {
    return t('projects.summary_duration_hours', {
      hours,
      minutes,
      defaultValue: `${hours}h ${minutes}m`,
    });
  }

  return t('projects.summary_duration_minutes', {
    minutes: totalMinutes,
    defaultValue: `${totalMinutes}m`,
  });
}

function matchesDateFilter(item: HistoryItemType, dateFilter: ProjectDateFilter): boolean {
  if (dateFilter === 'all') {
    return true;
  }

  const itemDate = new Date(item.timestamp);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  if (dateFilter === 'today') {
    return itemDate >= today;
  }

  if (dateFilter === 'week') {
    const weekAgo = new Date(today);
    weekAgo.setDate(weekAgo.getDate() - 7);
    return itemDate >= weekAgo;
  }

  const monthAgo = new Date(today);
  monthAgo.setMonth(monthAgo.getMonth() - 1);
  return itemDate >= monthAgo;
}

function compareProjectItems(a: HistoryItemType, b: HistoryItemType, sortOrder: ProjectSortOrder): number {
  switch (sortOrder) {
    case 'oldest':
      return a.timestamp - b.timestamp;
    case 'duration_desc':
      return b.duration - a.duration || b.timestamp - a.timestamp;
    case 'duration_asc':
      return a.duration - b.duration || b.timestamp - a.timestamp;
    case 'title_asc':
      return a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }) || b.timestamp - a.timestamp;
    case 'newest':
    default:
      return b.timestamp - a.timestamp;
  }
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

interface ProjectSettingsModalProps {
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

function ProjectSettingsModal({
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
}: ProjectSettingsModalProps): React.JSX.Element | null {
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
    <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
      <div
        className="dialog-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-settings-title"
        style={{
          background: 'var(--color-bg-elevated)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: 'var(--shadow-xl)',
          width: '650px',
          maxWidth: '95vw',
          maxHeight: '90vh',
          display: 'flex',
          flexDirection: 'column',
          border: '1px solid var(--color-border)',
          overflow: 'hidden'
        }}
      >
        {/* Header - Fixed */}
        <div style={{ 
          display: 'flex', 
          alignItems: 'center', 
          justifyContent: 'space-between', 
          padding: 'var(--spacing-lg) var(--spacing-lg) var(--spacing-md)',
          borderBottom: '1px solid var(--color-border)'
        }}>
          <div>
            <h3 id="project-settings-title" style={{ fontSize: '1.125rem', fontWeight: 600, color: 'var(--color-text-primary)', margin: 0 }}>
              {t('projects.project_settings_title', { defaultValue: 'Edit Project Defaults' })}
            </h3>
            <p style={{ fontSize: '0.8125rem', color: 'var(--color-text-secondary)', margin: '4px 0 0 0' }}>
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

        {/* Content - Scrollable */}
        <div className="settings-content-scroll" style={{ 
          padding: 'var(--spacing-lg)',
          flex: 1,
          overflowY: 'auto',
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--spacing-lg)'
        }}>
          <div className="projects-field">
            <label htmlFor="project-settings-name">
              {t('projects.project_name', { defaultValue: 'Project Name' })}
            </label>
            <input
              id="project-settings-name"
              type="text"
              className="settings-input"
              value={draftName}
              onChange={(event) => onNameChange(event.target.value)}
              placeholder={t('projects.new_project_name', { defaultValue: 'Project name' })}
            />
          </div>

          <div className="projects-field">
            <label htmlFor="project-settings-description">
              {t('projects.project_description', { defaultValue: 'Description' })}
            </label>
            <textarea
              id="project-settings-description"
              className="settings-input"
              value={draftDescription}
              onChange={(event) => onDescriptionChange(event.target.value)}
              style={{ minHeight: '80px' }}
            />
          </div>

          <div className="projects-settings-grid">
            <div className="projects-field">
              <label>
                {t('projects.summary_template', { defaultValue: 'Default Summary Template' })}
              </label>
              <Dropdown
                value={draftDefaults.summaryTemplate}
                onChange={(value: string) => onDefaultsChange({
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
                onChange={(value: string) => onDefaultsChange({
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
                onChange={(value: string) => onDefaultsChange({
                  ...draftDefaults,
                  polishScenario: value,
                })}
                options={polishScenarioOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div className="projects-field">
              <label htmlFor="project-settings-export-prefix">
                {t('projects.export_prefix', { defaultValue: 'Export Filename Prefix' })}
              </label>
              <input
                id="project-settings-export-prefix"
                type="text"
                className="settings-input"
                value={draftDefaults.exportFileNamePrefix}
                onChange={(event) => onDefaultsChange({
                  ...draftDefaults,
                  exportFileNamePrefix: event.target.value,
                })}
                placeholder={t('projects.export_prefix_placeholder', { defaultValue: 'e.g. SONA_' })}
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
                className="settings-input"
                value={draftDefaults.polishContext}
                onChange={(event) => onDefaultsChange({
                  ...draftDefaults,
                  polishContext: event.target.value,
                })}
                style={{ minHeight: '80px' }}
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

        {/* Footer - Fixed */}
        <div style={{ 
          display: 'flex', 
          alignItems: 'center', 
          justifyContent: 'space-between', 
          padding: 'var(--spacing-md) var(--spacing-lg)', 
          borderTop: '1px solid var(--color-border)',
          background: 'var(--color-bg-elevated)'
        }}>
          <button type="button" className="btn btn-danger" onClick={() => void onDelete()}>
            {t('projects.delete_project', { defaultValue: 'Delete Project' })}
          </button>

          <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button type="button" className="btn btn-primary" onClick={() => void onSave()}>
              {t('common.save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

interface SortableProjectItemProps {
  project: ProjectRecord;
  projectCount: number;
  isActive: boolean;
  onSwitchScope: (id: string) => Promise<void>;
  t: (key: string, options?: any) => string;
}

function SortableProjectItem({
  project,
  projectCount,
  isActive,
  onSwitchScope,
  t,
}: SortableProjectItemProps) {
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
        <div className="projects-rail-item-copy">
          <strong>{project.name}</strong>
          <span>
            {project.description || t('projects.items_title', {
              count: projectCount,
              defaultValue: `${projectCount} items`,
            })}
          </span>
        </div>
        <span className="projects-rail-count">{projectCount}</span>
      </button>
    </div>
  );
}

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

  const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
  const clearSegments = useTranscriptStore((state) => state.clearSegments);
  const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
  const setMode = useTranscriptStore((state) => state.setMode);

  const globalConfig = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);

  const viewMode = globalConfig.projectsViewMode || 'list';

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectDescription, setNewProjectDescription] = useState('');
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [draftDefaults, setDraftDefaults] = useState<ProjectDefaults | null>(null);
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [moveTarget, setMoveTarget] = useState(INBOX_SCOPE);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [selectedHistoryId, setSelectedHistoryId] = useState<string | null>(sourceHistoryId);
  const [browseScope, setBrowseScope] = useState<ProjectBrowseScope>(() => activeProjectId || INBOX_SCOPE);
  const [isFilterMenuOpen, setIsFilterMenuOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<ProjectFilterType>(DEFAULT_FILTER_TYPE);
  const [dateFilter, setDateFilter] = useState<ProjectDateFilter>(DEFAULT_DATE_FILTER);
  const [sortOrder, setSortOrder] = useState<ProjectSortOrder>(DEFAULT_SORT_ORDER);
  const filterMenuRef = useRef<HTMLDivElement>(null);

  const isAllItemsScope = browseScope === ALL_ITEMS_SCOPE;
  const isInboxScope = browseScope === INBOX_SCOPE;
  const browseProjectId = !isAllItemsScope && !isInboxScope ? browseScope : null;
  const browseProject = useMemo(
    () => projects.find((item) => item.id === browseProjectId) || null,
    [browseProjectId, projects],
  );

  const resetProjectSettingsDraft = useCallback((project: ProjectRecord | null = browseProject) => {
    if (!project) {
      setDraftName('');
      setDraftDescription('');
      setDraftDefaults(null);
      return;
    }

    setDraftName(project.name);
    setDraftDescription(project.description);
    setDraftDefaults(project.defaults);
  }, [browseProject]);

  useEffect(() => {
    void loadHistoryItems();
  }, [loadHistoryItems]);

  useEffect(() => {
    if (!browseProject) {
      resetProjectSettingsDraft(null);
      setIsSettingsOpen(false);
      return;
    }

    resetProjectSettingsDraft(browseProject);
  }, [browseProject, resetProjectSettingsDraft]);

  useEffect(() => {
    if (browseProjectId) {
      setMoveTarget(INBOX_SCOPE);
      return;
    }

    setMoveTarget(projects[0]?.id || INBOX_SCOPE);
  }, [browseProjectId, projects]);

  useEffect(() => {
    setSearchQuery('');
    setFilterType(DEFAULT_FILTER_TYPE);
    setDateFilter(DEFAULT_DATE_FILTER);
    setIsFilterMenuOpen(false);
  }, [browseScope]);

  useEffect(() => {
    if (!isFilterMenuOpen) {
      return undefined;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (filterMenuRef.current?.contains(event.target as Node)) {
        return;
      }

      setIsFilterMenuOpen(false);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') {
        return;
      }

      setIsFilterMenuOpen(false);
    };

    document.addEventListener('mousedown', handlePointerDown);
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isFilterMenuOpen]);

  useEffect(() => {
    if (browseScope === ALL_ITEMS_SCOPE || browseScope === INBOX_SCOPE) {
      return;
    }

    if (projects.some((item) => item.id === browseScope)) {
      return;
    }

    setBrowseScope(activeProjectId || INBOX_SCOPE);
  }, [activeProjectId, browseScope, projects]);

  const clearOpenedItem = useCallback(() => {
    setSelectedHistoryId(null);
    clearSegments();
    setAudioUrl(null);
  }, [clearSegments, setAudioUrl]);

  const scopedItems = useMemo(
    () => historyItems.filter((item) => {
      if (isAllItemsScope) {
        return true;
      }

      if (isInboxScope) {
        return item.projectId === null;
      }

      return item.projectId === browseProjectId;
    }),
    [browseProjectId, historyItems, isAllItemsScope, isInboxScope],
  );

  const selectedItem = useMemo(
    () => scopedItems.find((item) => item.id === selectedHistoryId) || null,
    [scopedItems, selectedHistoryId],
  );

  const filteredItems = useMemo(() => {
    const normalizedQuery = searchQuery.trim().toLowerCase();

    return scopedItems.filter((item) => {
      if (normalizedQuery) {
        const haystack = [item.title, item.searchContent || item.previewText || '']
          .join(' ')
          .toLowerCase();

        if (!haystack.includes(normalizedQuery)) {
          return false;
        }
      }

      if (filterType !== 'all' && (item.type || 'recording') !== filterType) {
        return false;
      }

      return matchesDateFilter(item, dateFilter);
    });
  }, [dateFilter, filterType, scopedItems, searchQuery]);

  const filteredAndSortedItems = useMemo(
    () => [...filteredItems].sort((a, b) => compareProjectItems(a, b, sortOrder)),
    [filteredItems, sortOrder],
  );

  const projectSummary = useMemo(() => {
    let totalDuration = 0;
    let recordingCount = 0;
    let batchCount = 0;
    let latestTimestamp: number | null = null;

    scopedItems.forEach((item) => {
      totalDuration += item.duration || 0;
      latestTimestamp = latestTimestamp === null ? item.timestamp : Math.max(latestTimestamp, item.timestamp);

      if ((item.type || 'recording') === 'batch') {
        batchCount += 1;
        return;
      }

      recordingCount += 1;
    });

    return {
      totalItems: scopedItems.length,
      totalDuration,
      latestTimestamp,
      recordingCount,
      batchCount,
    };
  }, [scopedItems]);

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

  useEffect(() => {
    const visibleIds = new Set(filteredAndSortedItems.map((item) => item.id));
    setSelectedIds((current) => {
      const next = current.filter((id) => visibleIds.has(id));
      return next.length === current.length ? current : next;
    });
  }, [filteredAndSortedItems]);

  const moveOptions = useMemo(
    () => [
      { value: INBOX_SCOPE, label: t('projects.inbox', { defaultValue: 'Inbox' }) },
      ...projects.map((project) => ({ value: project.id, label: project.name })),
    ],
    [projects, t],
  );

  const filterTypeOptions = useMemo(
    () => [
      { value: 'all', label: t('projects.filter_all_types', { defaultValue: 'All types' }) },
      { value: 'recording', label: t('projects.filter_recordings', { defaultValue: 'Recordings' }) },
      { value: 'batch', label: t('projects.filter_batch', { defaultValue: 'Batch imports' }) },
    ],
    [t],
  );

  const dateFilterOptions = useMemo(
    () => [
      { value: 'all', label: t('projects.date_all', { defaultValue: 'Any time' }) },
      { value: 'today', label: t('projects.date_today', { defaultValue: 'Today' }) },
      { value: 'week', label: t('projects.date_week', { defaultValue: 'Last 7 days' }) },
      { value: 'month', label: t('projects.date_month', { defaultValue: 'Last 30 days' }) },
    ],
    [t],
  );

  const sortOptions = useMemo(
    () => [
      { value: 'newest', label: t('projects.sort_newest', { defaultValue: 'Newest first' }) },
      { value: 'oldest', label: t('projects.sort_oldest', { defaultValue: 'Oldest first' }) },
      { value: 'duration_desc', label: t('projects.sort_duration_desc', { defaultValue: 'Longest first' }) },
      { value: 'duration_asc', label: t('projects.sort_duration_asc', { defaultValue: 'Shortest first' }) },
      { value: 'title_asc', label: t('projects.sort_title_asc', { defaultValue: 'Title A-Z' }) },
    ],
    [t],
  );

  const activeFilterLabels = useMemo(() => {
    const labels: string[] = [];

    if (filterType !== DEFAULT_FILTER_TYPE) {
      const typeLabel = filterTypeOptions.find((option) => option.value === filterType)?.label;
      if (typeLabel) {
        labels.push(typeLabel);
      }
    }

    if (dateFilter !== DEFAULT_DATE_FILTER) {
      const dateLabel = dateFilterOptions.find((option) => option.value === dateFilter)?.label;
      if (dateLabel) {
        labels.push(dateLabel);
      }
    }

    return labels;
  }, [dateFilter, dateFilterOptions, filterType, filterTypeOptions]);

  const activeFilterCount = activeFilterLabels.length;
  const hasActiveFilters = activeFilterCount > 0;
  const filterPopoverHint = hasActiveFilters
    ? activeFilterLabels.join(' · ')
    : t('projects.filter_menu_hint', {
      defaultValue: 'Refine the current workspace view by type or time.',
    });

  const isProjectSettingsDirty = useMemo(() => {
    if (!browseProject || !draftDefaults) {
      return false;
    }

    const currentDraft = buildComparableProjectSettings(
      browseProject,
      draftName,
      draftDescription,
      draftDefaults,
    );
    const savedProject = buildSavedProjectSettings(browseProject);

    return JSON.stringify(currentDraft) !== JSON.stringify(savedProject);
  }, [browseProject, draftDefaults, draftDescription, draftName]);

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

  const discardProjectSettingsDraft = useCallback((project: ProjectRecord | null = browseProject) => {
    resetProjectSettingsDraft(project);
    setIsSettingsOpen(false);
  }, [browseProject, resetProjectSettingsDraft]);

  const handleRequestCloseProjectSettings = useCallback(async () => {
    const shouldDiscard = await confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    discardProjectSettingsDraft();
  }, [confirmDiscardProjectSettingsChanges, discardProjectSettingsDraft]);

  const handleSwitchBrowseScope = async (nextScope: ProjectBrowseScope) => {
    const shouldDiscard = await confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (isSettingsOpen) {
      discardProjectSettingsDraft();
    }

    setIsSelectionMode(false);
    setSelectedIds([]);
    setIsFilterMenuOpen(false);
    setBrowseScope(nextScope);

    if (nextScope === ALL_ITEMS_SCOPE) {
      return;
    }

    await setActiveProjectId(nextScope === INBOX_SCOPE ? null : nextScope);
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

      useTranscriptStore.getState().loadTranscript(segments, item.id, item.title);
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
    setBrowseScope(project.id);
    await setActiveProjectId(project.id);
  };

  const handleSaveProject = async () => {
    if (!browseProject || !draftDefaults) {
      return;
    }

    await updateProject(browseProject.id, {
      name: draftName.trim() || browseProject.name,
      description: draftDescription,
      defaults: draftDefaults,
    });
    setIsSettingsOpen(false);
  };

  const handleDeleteProject = async () => {
    if (!browseProject) {
      return;
    }

    const shouldDiscard = await confirmDiscardProjectSettingsChanges();
    if (!shouldDiscard) {
      return;
    }

    if (isSettingsOpen) {
      discardProjectSettingsDraft(browseProject);
    }

    const confirmed = await confirm(
      t('projects.delete_confirm', {
        defaultValue: `Delete ${browseProject.name} and move its items back to Inbox?`,
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
    setBrowseScope(INBOX_SCOPE);
    await deleteProject(browseProject.id);
    await refreshHistory();
  };

  const toggleSelection = (id: string) => {
    setSelectedIds((current) => (
      current.includes(id) ? current.filter((itemId) => itemId !== id) : [...current, id]
    ));
  };

  const toggleSelectionMode = () => {
    setIsFilterMenuOpen(false);
    setIsSelectionMode((value) => !value);
    setSelectedIds([]);
  };

  const handleToggleSelectAll = () => {
    if (selectedIds.length === filteredAndSortedItems.length && filteredAndSortedItems.length > 0) {
      setSelectedIds([]);
    } else {
      setSelectedIds(filteredAndSortedItems.map((item) => item.id));
    }
  };

  const handleMoveSelected = async () => {
    if (selectedIds.length === 0) {
      return;
    }

    const targetProjectId = moveTarget === INBOX_SCOPE ? null : moveTarget;
    await assignHistoryItems(selectedIds, targetProjectId);
    await refreshHistory();

    const currentHistoryId = useTranscriptStore.getState().sourceHistoryId;
    if (currentHistoryId && selectedIds.includes(currentHistoryId)) {
      await setActiveProjectId(targetProjectId);
    }

    setSelectedIds([]);
    setIsSelectionMode(false);
  };

  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(event.active.id as string);
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveId(null);

    if (over && active.id !== over.id) {
      const oldIndex = projects.findIndex((p) => p.id === active.id);
      const newIndex = projects.findIndex((p) => p.id === over.id);

      const newOrder = arrayMove(projects, oldIndex, newIndex);
      await reorderProjects(newOrder.map((p) => p.id));
    }
  };

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

  const handleDeleteSelected = async () => {
    if (selectedIds.length === 0) {
      return;
    }

    const confirmed = await confirm(
      t('history.delete_bulk_confirm', {
        count: selectedIds.length,
        defaultValue: `Are you sure you want to delete ${selectedIds.length} items?`,
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

    await deleteHistoryItems(selectedIds);
    await refreshHistory();
    setSelectedIds([]);
    setIsSelectionMode(false);
  };

  const resetBrowseState = useCallback(() => {
    setSearchQuery('');
    setFilterType(DEFAULT_FILTER_TYPE);
    setDateFilter(DEFAULT_DATE_FILTER);
  }, []);

  const headerTitle = isAllItemsScope
    ? t('projects.all_items', { defaultValue: 'All Items' })
    : browseProject?.name || t('projects.inbox', { defaultValue: 'Inbox' });
  const headerDescription = isAllItemsScope
    ? t('projects.all_items_description', {
      defaultValue: 'Browse everything across Inbox and your projects.',
    })
    : browseProject
    ? browseProject.description
    : t('projects.inbox_description', {
      defaultValue: 'Inbox collects unassigned recordings and imports.',
    });
  const showWorkflowActions = !isAllItemsScope;
  const currentScopeMoveTarget = isAllItemsScope ? null : browseProjectId || INBOX_SCOPE;
  const summaryChips = [
    {
      key: 'items',
      label: t('projects.summary_items', { defaultValue: 'Items' }),
      value: String(projectSummary.totalItems),
      testId: 'projects-summary-total-items',
    },
    {
      key: 'duration',
      label: t('projects.summary_duration', { defaultValue: 'Total duration' }),
      value: formatSummaryDuration(projectSummary.totalDuration, t),
      testId: 'projects-summary-total-duration',
    },
    {
      key: 'latest',
      label: t('projects.summary_latest_activity', { defaultValue: 'Latest activity' }),
      value: projectSummary.latestTimestamp
        ? formatTimestamp(projectSummary.latestTimestamp)
        : t('projects.summary_no_activity', { defaultValue: 'No activity yet' }),
      testId: 'projects-summary-latest-activity',
    },
    {
      key: 'type-split',
      label: t('projects.summary_type_split', { defaultValue: 'Type split' }),
      value: t('projects.summary_type_split_value', {
        recordings: projectSummary.recordingCount,
        imports: projectSummary.batchCount,
        defaultValue: `${projectSummary.recordingCount} recordings / ${projectSummary.batchCount} imports`,
      }),
      testId: 'projects-summary-type-split',
    },
  ];

  return (
    <div className={`projects-workbench ${selectedItem ? 'with-detail' : ''}`}>
      <aside className="projects-rail">
        <div className="projects-rail-header">
          <div className="projects-rail-title-row">
            <div className="projects-rail-eyebrow">
              {t('panel.projects', { defaultValue: 'Workspace' })}
            </div>
            <button
              type="button"
              className="btn btn-icon projects-rail-create"
              onClick={() => setIsCreateModalOpen(true)}
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
            onClick={() => void handleSwitchBrowseScope(ALL_ITEMS_SCOPE)}
            aria-pressed={isAllItemsScope}
          >
            <div className="projects-rail-item-copy">
              <strong>{t('projects.all_items', { defaultValue: 'All Items' })}</strong>
            </div>
            <span className="projects-rail-count">{historyItems.length}</span>
          </button>

          <button
            type="button"
            className={`projects-rail-item ${isInboxScope ? 'active' : ''}`}
            onClick={() => void handleSwitchBrowseScope(INBOX_SCOPE)}
            aria-pressed={isInboxScope}
          >
            <div className="projects-rail-item-copy">
              <strong>{t('projects.inbox', { defaultValue: 'Inbox' })}</strong>
            </div>
            <span className="projects-rail-count">{itemCounts.get(null) || 0}</span>
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
                items={projects.map((p) => p.id)}
                strategy={verticalListSortingStrategy}
              >
                {projects.map((project) => (
                  <SortableProjectItem
                    key={project.id}
                    project={project}
                    projectCount={itemCounts.get(project.id) || 0}
                    isActive={browseProjectId === project.id}
                    onSwitchScope={handleSwitchBrowseScope}
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
                      <div className="projects-rail-item-copy">
                        <strong>{projects.find((p) => p.id === activeId)?.name}</strong>
                        <span>
                          {projects.find((p) => p.id === activeId)?.description || t('projects.items_title', {
                            count: itemCounts.get(activeId) || 0,
                            defaultValue: `${itemCounts.get(activeId) || 0} items`,
                          })}
                        </span>
                      </div>
                      <span className="projects-rail-count">{itemCounts.get(activeId) || 0}</span>
                    </button>
                  </div>
                ) : null}
              </DragOverlay>
            </DndContext>
          </div>
        </div>
      </aside>

      {!selectedItem && (
      <section className="projects-main">
        <div className="projects-main-header">
          <div className="projects-main-header-top">
            <div className="projects-main-heading">
              <div className="projects-main-title-row">
                <h3>{headerTitle}</h3>
              </div>
              <p>{headerDescription}</p>
            </div>
            <div className="projects-main-entry-actions" data-testid="projects-main-entry-actions">
              {showWorkflowActions && (
                <>
                  <button
                    type="button"
                    className="btn btn-icon projects-header-icon"
                    onClick={() => setMode('live')}
                    aria-label={t('projects.start_live_record', { defaultValue: 'Start Live Record' })}
                    data-tooltip={t('projects.start_live_record', { defaultValue: 'Start Live Record' })}
                    data-tooltip-pos="bottom"
                  >
                    <MicIcon width={16} height={16} />
                  </button>
                  <button
                    type="button"
                    className="btn btn-icon projects-header-icon"
                    onClick={() => setMode('batch')}
                    aria-label={t('projects.open_batch_import', { defaultValue: 'Open Batch Import' })}
                    data-tooltip={t('projects.open_batch_import', { defaultValue: 'Open Batch Import' })}
                    data-tooltip-pos="bottom"
                  >
                    <FileTextIcon width={16} height={16} />
                  </button>
                </>
              )}
              {browseProject && (
                <button
                  type="button"
                  className="btn btn-icon projects-header-icon"
                  onClick={() => setIsSettingsOpen(true)}
                  aria-label={t('projects.project_settings', { defaultValue: 'Project Settings' })}
                  data-tooltip={t('projects.project_settings', { defaultValue: 'Project Settings' })}
                  data-tooltip-pos="bottom-left"
                >
                  <SettingsIcon width={16} height={16} />
                </button>
              )}
            </div>
          </div>

          <div className="projects-stats-row" data-testid="projects-summary-chips">
            {summaryChips.map((chip) => (
              <div key={chip.key} className="projects-stat-card">
                <span className="projects-stat-label">{chip.label}</span>
                <strong className="projects-stat-value" data-testid={chip.testId}>
                  {chip.value}
                </strong>
              </div>
            ))}
          </div>
        </div>

        <div className="projects-toolbar">
          <div className="projects-toolbar-search-group">
            <div className="projects-search">
              <Search size={16} className="projects-search-icon" />
              <input
                type="text"
                placeholder={t('projects.search_placeholder', { defaultValue: 'Search this workspace...' })}
                aria-label={t('projects.search_placeholder', { defaultValue: 'Search this workspace...' })}
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Escape') {
                    setSearchQuery('');
                  }
                }}
              />
              {searchQuery && (
                <button
                  type="button"
                  className="btn btn-icon btn-text projects-search-clear"
                  onClick={() => setSearchQuery('')}
                  aria-label={t('common.clear_search', { defaultValue: 'Clear search' })}
                >
                  <XIcon width={14} height={14} />
                </button>
              )}
            </div>

            <div className="projects-toolbar-copy">
            </div>
          </div>

          <div className="projects-toolbar-controls">
              <div className="projects-toolbar-default" data-testid="projects-toolbar-default">
                <div className="projects-toolbar-primary">
                  <strong className="projects-results-count" data-testid="projects-results-count">{t('projects.results_count', {
                    visible: filteredAndSortedItems.length,
                    total: scopedItems.length,
                    defaultValue: `Showing ${filteredAndSortedItems.length} of ${scopedItems.length}`,
                  })}</strong>
                  <div className="projects-filter-menu" ref={filterMenuRef}>
                    <button
                      type="button"
                      className={`btn btn-icon projects-toolbar-icon projects-filter-trigger ${isFilterMenuOpen ? 'active' : ''} ${hasActiveFilters ? 'has-active' : ''}`}
                      onClick={() => setIsFilterMenuOpen((value) => !value)}
                      aria-haspopup="dialog"
                      aria-label={t('projects.filter_button', { defaultValue: 'Filter' })}
                      aria-expanded={isFilterMenuOpen}
                      aria-controls="projects-filter-panel"
                      data-tooltip={t('projects.filter_button', { defaultValue: 'Filter & Sort' })}
                      data-tooltip-pos="bottom"
                    >
                      <SlidersHorizontal size={16} />
                      {hasActiveFilters && (
                        <span className="projects-filter-trigger-count" aria-hidden="true">
                          {activeFilterCount}
                        </span>
                      )}
                    </button>

                    {isFilterMenuOpen && (
                      <div
                        id="projects-filter-panel"
                        className="projects-filter-popover"
                        role="dialog"
                        aria-label={t('projects.filter_button', { defaultValue: 'Filter' })}
                      >
                        <div className="projects-filter-popover-header">
                          <div className="projects-filter-popover-copy">
                            <strong>{t('projects.filter_button', { defaultValue: 'Filter & Sort' })}</strong>
                            <span>{filterPopoverHint}</span>
                          </div>
                          <button
                            type="button"
                            className="btn btn-text projects-filter-clear"
                            onClick={resetBrowseState}
                            disabled={!hasActiveFilters}
                          >
                            {t('projects.clear_filters', { defaultValue: 'Clear filters' })}
                          </button>
                        </div>

                        <div className="projects-filter-popover-body">
                          <div className="projects-filter-field">
                            <span className="projects-toolbar-field-label">
                              {t('projects.sort_label', { defaultValue: 'Sort items' })}
                            </span>
                            <Dropdown
                              value={sortOrder}
                              onChange={(value: string) =>
 setSortOrder(value as ProjectSortOrder)}
                              options={sortOptions}
                              style={{ width: '100%' }}
                              aria-label={t('projects.sort_label', { defaultValue: 'Sort items' })}
                            />
                          </div>
                          <div className="projects-filter-field">
                            <span className="projects-toolbar-field-label">
                              {t('projects.filter_type_label', { defaultValue: 'Filter by type' })}
                            </span>
                            <Dropdown
                              value={filterType}
                              onChange={(value: string) =>
 setFilterType(value as ProjectFilterType)}
                              options={filterTypeOptions}
                              style={{ width: '100%' }}
                              aria-label={t('projects.filter_type_label', { defaultValue: 'Filter by type' })}
                            />
                          </div>
                          <div className="projects-filter-field">
                            <span className="projects-toolbar-field-label">
                              {t('projects.filter_date_label', { defaultValue: 'Filter by date' })}
                            </span>
                            <Dropdown
                              value={dateFilter}
                              onChange={(value: string) =>
 setDateFilter(value as ProjectDateFilter)}
                              options={dateFilterOptions}
                              style={{ width: '100%' }}
                              aria-label={t('projects.filter_date_label', { defaultValue: 'Filter by date' })}
                            />
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </div>

                <div className="projects-toolbar-actions">
                  <div className="projects-view-toggles" role="group" aria-label={t('projects.view_mode', { defaultValue: 'View Mode' })}>
                    <button
                      type="button"
                      className={`btn btn-icon projects-toolbar-icon ${viewMode === 'list' ? 'active' : ''}`}
                      onClick={() => setConfig({ projectsViewMode: 'list' })}
                      aria-pressed={viewMode === 'list'}
                      aria-label={t('projects.view_list', { defaultValue: 'List View' })}
                      data-tooltip={t('projects.view_list', { defaultValue: 'List View' })}
                      data-tooltip-pos="bottom"
                    >
                      <List size={16} />
                    </button>
                    <button
                      type="button"
                      className={`btn btn-icon projects-toolbar-icon ${viewMode === 'grid' ? 'active' : ''}`}
                      onClick={() => setConfig({ projectsViewMode: 'grid' })}
                      aria-pressed={viewMode === 'grid'}
                      aria-label={t('projects.view_grid', { defaultValue: 'Grid View' })}
                      data-tooltip={t('projects.view_grid', { defaultValue: 'Grid View' })}
                      data-tooltip-pos="bottom"
                    >
                      <LayoutGrid size={16} />
                    </button>
                    <button
                      type="button"
                      className={`btn btn-icon projects-toolbar-icon ${viewMode === 'table' ? 'active' : ''}`}
                      onClick={() => setConfig({ projectsViewMode: 'table' })}
                      aria-pressed={viewMode === 'table'}
                      aria-label={t('projects.view_table', { defaultValue: 'Table View' })}
                      data-tooltip={t('projects.view_table', { defaultValue: 'Table View' })}
                      data-tooltip-pos="bottom"
                    >
                      <LayoutList size={16} />
                    </button>
                  </div>
                  <div className="projects-toolbar-separator" />
                  <button
                    type="button"
                    className="btn btn-icon projects-toolbar-icon"
                    onClick={() => historyService.openHistoryFolder()}
                    aria-label={t('history.open_folder', { defaultValue: 'Open File Directory' })}
                    data-tooltip={t('history.open_folder', { defaultValue: 'Open File Directory' })}
                    data-tooltip-pos="bottom"
                  >
                    <FolderIcon />
                  </button>
                  <button
                    type="button"
                    className={`btn btn-icon projects-toolbar-icon ${isSelectionMode ? 'active' : ''}`}
                    onClick={toggleSelectionMode}
                    aria-label={t('common.select', { defaultValue: 'Select' })}
                    data-tooltip={t('common.select', { defaultValue: 'Select' })}
                    data-tooltip-pos="bottom"
                  >
                    <CheckSquare size={16} />
                  </button>
                </div>
              </div>

            {isSelectionMode && (
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
                    className={`btn btn-icon projects-toolbar-icon ${selectedIds.length === filteredAndSortedItems.length && filteredAndSortedItems.length > 0 ? 'active' : ''}`}
                    onClick={handleToggleSelectAll}
                    aria-label={t('common.select_all', { defaultValue: 'Select All' })}
                    data-tooltip={t('common.select_all', { defaultValue: 'Select All' })}
                    data-tooltip-pos="top"
                  >
                    <ListChecks size={16} />
                  </button>
                  <div className="projects-toolbar-separator" />
                  <Dropdown
                    value={moveTarget}
                    onChange={setMoveTarget}
                    options={moveOptions}
                    style={{ width: '200px' }}
                    aria-label={t('projects.move_target', { defaultValue: 'Move target' })}
                  />
                  <button
                    type="button"
                    className="btn btn-icon projects-toolbar-icon"
                    onClick={() => void handleMoveSelected()}
                    disabled={selectedIds.length === 0 || (currentScopeMoveTarget !== null && moveTarget === currentScopeMoveTarget)}
                    aria-label={t('projects.move_selected', { defaultValue: 'Move Selected' })}
                    data-tooltip={t('projects.move_selected', { defaultValue: 'Move Selected' })}
                    data-tooltip-pos="top"
                  >
                    <ArrowRight size={16} />
                  </button>
                  <button
                    type="button"
                    className="btn btn-icon projects-toolbar-icon btn-danger"
                    onClick={() => void handleDeleteSelected()}
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
                    onClick={toggleSelectionMode}
                    aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
                    data-tooltip={t('common.cancel', { defaultValue: 'Cancel' })}
                    data-tooltip-pos="top"
                  >
                    <X size={16} />
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>

        <div className="projects-main-scroll">
          {!selectedItem && scopedItems.length === 0 && !isHistoryLoading && (
            <div className="projects-overview-card">
              <PlusCircleIcon />
              <h4>{t('projects.empty_state', { defaultValue: 'No items in this workspace yet.' })}</h4>
              <p>
                {isAllItemsScope
                  ? t('projects.empty_all_items_hint', {
                    defaultValue: 'Saved recordings and imports will appear here once you create some content.',
                  })
                  : browseProject
                  ? t('projects.empty_project_hint', {
                    defaultValue: 'Start a live recording or import files to begin building this project.',
                  })
                  : t('projects.empty_inbox_hint', {
                    defaultValue: 'New recordings and imports will arrive here until you move them into a project.',
                  })}
              </p>
            </div>
          )}

          {!isHistoryLoading && scopedItems.length > 0 && filteredAndSortedItems.length === 0 && (
            <div className="projects-overview-card">
              <Search size={28} />
              <h4>{t('projects.no_results_title', { defaultValue: 'No matching items' })}</h4>
              <p>
                {t('projects.no_results_hint', {
                  defaultValue: 'Try a different search or clear the current filters.',
                })}
              </p>
              <button type="button" className="btn btn-secondary" onClick={resetBrowseState}>
                {t('projects.clear_filters', { defaultValue: 'Clear filters' })}
              </button>
            </div>
          )}

          {isHistoryLoading && (
            <div className="projects-list-empty">
              {t('history.loading')}
            </div>
          )}

          {!isHistoryLoading && filteredAndSortedItems.length > 0 && (
            <div className={`projects-list projects-layout-${viewMode}`}>
              {viewMode === 'table' && (
                <div className="projects-table-header" role="row">
                  {isSelectionMode && <div role="columnheader" style={{ width: '40px' }} />}
                  <div style={{ flex: 1, display: 'flex', alignItems: 'center', minWidth: 0 }}>
                    <div className="projects-table-header-cell projects-table-header-title" role="columnheader">
                      {t('projects.table_header_name', { defaultValue: 'Name' })}
                    </div>
                    <div className="projects-table-header-cell projects-table-header-project" role="columnheader">
                      {t('projects.table_header_project', { defaultValue: 'Project' })}
                    </div>
                    <div style={{ flex: 2, display: 'flex', alignItems: 'center' }}>
                      <div className="projects-table-header-cell projects-table-header-date" role="columnheader">
                        {t('projects.table_header_date', { defaultValue: 'Date' })}
                      </div>
                      <div className="projects-table-header-cell projects-table-header-duration" role="columnheader">
                        {t('projects.table_header_duration', { defaultValue: 'Duration' })}
                      </div>
                    </div>
                  </div>
                  {!isSelectionMode && <div role="columnheader" style={{ width: '48px' }} />}
                </div>
              )}
              {filteredAndSortedItems.map((item) => (
                <HistoryItem
                  key={item.id}
                  item={item}
                  onLoad={handleOpenItem}
                  onDelete={handleDeleteHistoryItem}
                  searchQuery={searchQuery}
                  isSelectionMode={isSelectionMode}
                  isSelected={isSelectionMode ? selectedIds.includes(item.id) : selectedHistoryId === item.id}
                  onToggleSelection={toggleSelection}
                  layout={viewMode}
                />
              ))}
            </div>
          )}
        </div>
      </section>
      )}

      {selectedItem && (
          <aside className="projects-detail-pane">
            <TranscriptWorkbench onClose={clearOpenedItem} title={selectedItem.title} />
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
        isOpen={isSettingsOpen}
        project={browseProject}
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
