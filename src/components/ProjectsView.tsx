import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';
import { Checkbox } from './Checkbox';
import { HistoryItem } from './history/HistoryItem';
import { useProjectStore } from '../stores/projectStore';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { historyService } from '../services/historyService';
import type { HistoryItem as HistoryItemType } from '../types/history';
import type { ProjectDefaults } from '../types/project';

const LANGUAGE_OPTIONS = [
  'zh',
  'en',
  'ja',
  'ko',
  'fr',
  'de',
  'es',
];

const SUMMARY_TEMPLATE_OPTIONS = ['general', 'meeting', 'lecture'] as const;
const POLISH_SCENARIO_OPTIONS = [
  'customer_service',
  'meeting',
  'interview',
  'lecture',
  'podcast',
  'custom',
] as const;

export function ProjectsView(): React.JSX.Element {
  const { t } = useTranslation();
  const projects = useProjectStore((state) => state.projects);
  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => state.projects.find((item) => item.id === state.activeProjectId) || null);
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

  const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
  const setMode = useTranscriptStore((state) => state.setMode);
  const globalConfig = useConfigStore((state) => state.config);

  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);

  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectDescription, setNewProjectDescription] = useState('');
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [draftDefaults, setDraftDefaults] = useState<ProjectDefaults | null>(null);
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [moveTarget, setMoveTarget] = useState('inbox');

  useEffect(() => {
    void loadHistoryItems();
  }, [loadHistoryItems]);

  useEffect(() => {
    if (!activeProject) {
      setDraftName('');
      setDraftDescription('');
      setDraftDefaults(null);
      return;
    }

    setDraftName(activeProject.name);
    setDraftDescription(activeProject.description);
    setDraftDefaults(activeProject.defaults);
  }, [activeProject]);

  useEffect(() => {
    if (activeProjectId) {
      setMoveTarget('inbox');
      return;
    }

    setMoveTarget(projects[0]?.id || 'inbox');
  }, [activeProjectId, projects]);

  const scopedItems = useMemo(
    () => historyItems.filter((item) => (activeProjectId ? item.projectId === activeProjectId : item.projectId === null)),
    [historyItems, activeProjectId],
  );

  const moveOptions = useMemo(
    () => [
      { value: 'inbox', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
      ...projects.map((project) => ({ value: project.id, label: project.name })),
    ],
    [projects, t],
  );

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
      await setActiveProjectId(item.projectId);
    } catch (error) {
      await showError({
        code: 'history.load_failed',
        messageKey: 'errors.history.load_failed',
        cause: error,
      });
    }
  };

  const handleDeleteHistoryItem = async (e: React.MouseEvent, id: string) => {
    e.stopPropagation();

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
  };

  const handleDeleteProject = async () => {
    if (!activeProject) {
      return;
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

  const toggleRuleSetId = (key: 'enabledTextReplacementSetIds' | 'enabledHotwordSetIds', id: string) => {
    if (!draftDefaults) {
      return;
    }

    const current = new Set(draftDefaults[key]);
    if (current.has(id)) {
      current.delete(id);
    } else {
      current.add(id);
    }

    setDraftDefaults({
      ...draftDefaults,
      [key]: Array.from(current),
    });
  };

  return (
    <div
      style={{
        display: 'flex',
        height: '100%',
        background: 'var(--color-bg-primary)',
      }}
    >
      <div
        style={{
          width: '220px',
          borderRight: '1px solid var(--color-border)',
          background: 'var(--color-bg-secondary)',
          display: 'flex',
          flexDirection: 'column',
          minWidth: 0,
        }}
      >
        <div style={{ padding: 'var(--spacing-md)', borderBottom: '1px solid var(--color-border)' }}>
          <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', marginBottom: 'var(--spacing-xs)' }}>
            {t('projects.create_project', { defaultValue: 'Create Project' })}
          </div>
          <input
            type="text"
            value={newProjectName}
            onChange={(e) => setNewProjectName(e.target.value)}
            placeholder={t('projects.new_project_name', { defaultValue: 'Project name' })}
            style={{ width: '100%', marginBottom: 'var(--spacing-sm)' }}
          />
          <textarea
            value={newProjectDescription}
            onChange={(e) => setNewProjectDescription(e.target.value)}
            placeholder={t('projects.new_project_description', { defaultValue: 'Short description' })}
            style={{
              width: '100%',
              minHeight: '70px',
              resize: 'vertical',
              marginBottom: 'var(--spacing-sm)',
            }}
          />
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void handleCreateProject()}
            disabled={!newProjectName.trim()}
            style={{ width: '100%' }}
          >
            {t('projects.create_action', { defaultValue: 'Create Project' })}
          </button>
        </div>

        <div style={{ padding: 'var(--spacing-sm)', overflowY: 'auto', flex: 1 }}>
          <button
            type="button"
            className={`tab-button ${activeProjectId === null ? 'active' : ''}`}
            onClick={() => void setActiveProjectId(null)}
            style={{ width: '100%', justifyContent: 'flex-start', marginBottom: 'var(--spacing-xs)' }}
          >
            <span>{t('projects.inbox', { defaultValue: 'Inbox' })}</span>
          </button>

          {projects.map((project) => (
            <button
              key={project.id}
              type="button"
              className={`tab-button ${activeProjectId === project.id ? 'active' : ''}`}
              onClick={() => void setActiveProjectId(project.id)}
              style={{
                width: '100%',
                justifyContent: 'flex-start',
                marginBottom: 'var(--spacing-xs)',
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'flex-start',
                gap: '2px',
              }}
            >
              <span>{project.name}</span>
              {project.description && (
                <span
                  style={{
                    fontSize: '0.75rem',
                    color: 'var(--color-text-muted)',
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    width: '100%',
                  }}
                >
                  {project.description}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>

      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'flex-start',
            justifyContent: 'space-between',
            gap: 'var(--spacing-lg)',
            padding: 'var(--spacing-md)',
            borderBottom: '1px solid var(--color-border)',
            background: 'var(--color-bg-primary)',
          }}
        >
          <div style={{ minWidth: 0 }}>
            <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
              {t('projects.workspace_label', { defaultValue: 'Workspace' })}
            </div>
            <h3 style={{ margin: 0, color: 'var(--color-text-primary)' }}>
              {activeProject?.name || t('projects.inbox', { defaultValue: 'Inbox' })}
            </h3>
            <p style={{ margin: 'var(--spacing-xs) 0 0', color: 'var(--color-text-secondary)' }}>
              {activeProject?.description || t('projects.inbox_description', { defaultValue: 'Inbox collects unassigned recordings and imports.' })}
            </p>
          </div>

          <div style={{ display: 'flex', gap: 'var(--spacing-sm)', flexWrap: 'wrap', justifyContent: 'flex-end' }}>
            <button type="button" className="btn btn-secondary" onClick={() => setMode('live')}>
              {t('projects.start_live_record', { defaultValue: 'Start Live Record' })}
            </button>
            <button type="button" className="btn btn-secondary" onClick={() => setMode('batch')}>
              {t('projects.open_batch_import', { defaultValue: 'Open Batch Import' })}
            </button>
            {activeProject && (
              <>
                <button type="button" className="btn btn-primary" onClick={() => void handleSaveProject()}>
                  {t('common.save', { defaultValue: 'Save' })}
                </button>
                <button type="button" className="btn btn-danger" onClick={() => void handleDeleteProject()}>
                  {t('projects.delete_project', { defaultValue: 'Delete Project' })}
                </button>
              </>
            )}
          </div>
        </div>

        {activeProject && draftDefaults && (
          <div
            style={{
              padding: 'var(--spacing-md)',
              borderBottom: '1px solid var(--color-border)',
              display: 'grid',
              gap: 'var(--spacing-md)',
              gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
              background: 'var(--color-bg-secondary-soft)',
            }}
          >
            <div style={{ gridColumn: '1 / -1' }}>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                {t('projects.project_name', { defaultValue: 'Project Name' })}
              </label>
              <input value={draftName} onChange={(e) => setDraftName(e.target.value)} style={{ width: '100%' }} />
            </div>

            <div style={{ gridColumn: '1 / -1' }}>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                {t('projects.project_description', { defaultValue: 'Description' })}
              </label>
              <textarea
                value={draftDescription}
                onChange={(e) => setDraftDescription(e.target.value)}
                style={{ width: '100%', minHeight: '72px', resize: 'vertical' }}
              />
            </div>

            <div>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                {t('projects.summary_template', { defaultValue: 'Default Summary Template' })}
              </label>
              <Dropdown
                value={draftDefaults.summaryTemplate}
                onChange={(value) => setDraftDefaults({ ...draftDefaults, summaryTemplate: value as ProjectDefaults['summaryTemplate'] })}
                options={summaryTemplateOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                {t('projects.translation_language', { defaultValue: 'Default Translation Language' })}
              </label>
              <Dropdown
                value={draftDefaults.translationLanguage}
                onChange={(value) => setDraftDefaults({ ...draftDefaults, translationLanguage: value })}
                options={languageOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                {t('projects.polish_scenario', { defaultValue: 'Default Polish Scenario' })}
              </label>
              <Dropdown
                value={draftDefaults.polishScenario}
                onChange={(value) => setDraftDefaults({ ...draftDefaults, polishScenario: value })}
                options={polishScenarioOptions}
                style={{ width: '100%' }}
              />
            </div>

            <div>
              <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                {t('projects.export_prefix', { defaultValue: 'Export Filename Prefix' })}
              </label>
              <input
                value={draftDefaults.exportFileNamePrefix}
                onChange={(e) => setDraftDefaults({ ...draftDefaults, exportFileNamePrefix: e.target.value })}
                style={{ width: '100%' }}
              />
            </div>

            {(draftDefaults.polishScenario === 'custom' || !draftDefaults.polishScenario) && (
              <div style={{ gridColumn: '1 / -1' }}>
                <label style={{ display: 'block', marginBottom: 'var(--spacing-xs)', fontWeight: 500 }}>
                  {t('projects.polish_context', { defaultValue: 'Default Polish Context' })}
                </label>
                <textarea
                  value={draftDefaults.polishContext}
                  onChange={(e) => setDraftDefaults({ ...draftDefaults, polishContext: e.target.value })}
                  style={{ width: '100%', minHeight: '80px', resize: 'vertical' }}
                />
              </div>
            )}

            <div>
              <div style={{ fontWeight: 500, marginBottom: 'var(--spacing-sm)' }}>
                {t('projects.text_replacement_sets', { defaultValue: 'Enabled Text Replacement Sets' })}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                {(globalConfig.textReplacementSets || []).length === 0 ? (
                  <span style={{ color: 'var(--color-text-muted)', fontSize: '0.875rem' }}>
                    {t('projects.no_text_replacement_sets', { defaultValue: 'No global text replacement sets yet.' })}
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

            <div>
              <div style={{ fontWeight: 500, marginBottom: 'var(--spacing-sm)' }}>
                {t('projects.hotword_sets', { defaultValue: 'Enabled Hotword Sets' })}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                {(globalConfig.hotwordSets || []).length === 0 ? (
                  <span style={{ color: 'var(--color-text-muted)', fontSize: '0.875rem' }}>
                    {t('projects.no_hotword_sets', { defaultValue: 'No global hotword sets yet.' })}
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
        )}

        <div style={{ padding: 'var(--spacing-md)', borderBottom: '1px solid var(--color-border)', display: 'flex', justifyContent: 'space-between', gap: 'var(--spacing-md)', alignItems: 'center' }}>
          <div>
            <div style={{ fontWeight: 600, color: 'var(--color-text-primary)' }}>
              {t('projects.items_title', { count: scopedItems.length, defaultValue: `${scopedItems.length} items` })}
            </div>
            <div style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>
              {t('projects.items_hint', { defaultValue: 'Open items in place or move them between Inbox and projects.' })}
            </div>
          </div>

          <div style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
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
                  style={{ width: '200px' }}
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

        <div style={{ flex: 1, overflowY: 'auto', padding: 'var(--spacing-md)' }}>
          {isHistoryLoading && (
            <div style={{ color: 'var(--color-text-muted)', textAlign: 'center', padding: 'var(--spacing-xl)' }}>
              {t('history.loading')}
            </div>
          )}

          {!isHistoryLoading && scopedItems.length === 0 && (
            <div style={{ color: 'var(--color-text-muted)', textAlign: 'center', padding: 'var(--spacing-2xl) var(--spacing-xl)' }}>
              {t('projects.empty_state', { defaultValue: 'No items in this workspace yet.' })}
            </div>
          )}

          {!isHistoryLoading && scopedItems.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-sm)' }}>
              {scopedItems.map((item) => (
                <HistoryItem
                  key={item.id}
                  item={item}
                  onLoad={handleOpenItem}
                  onDelete={handleDeleteHistoryItem}
                  isSelectionMode={isSelectionMode}
                  isSelected={selectedIds.includes(item.id)}
                  onToggleSelection={toggleSelection}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default ProjectsView;
