import { useCallback, useEffect, useMemo, useState } from 'react';
import { DEFAULT_PROJECT_PIPELINE, type ProjectPipelineConfig, type ProjectRecord } from '../../../types/project';
import type { TranslationFn } from '../types';
import { useDialogStore } from '../../../stores/dialogStore';

type ConfirmFn = ReturnType<typeof useDialogStore.getState>['confirm'];

interface UseProjectSettingsDraftParams {
  browseProject: ProjectRecord | null;
  confirm: ConfirmFn;
  t: TranslationFn;
}

export function useProjectSettingsDraft({
  browseProject,
  confirm,
  t,
}: UseProjectSettingsDraftParams) {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [draftIcon, setDraftIcon] = useState('');
  const [draftColor, setDraftColor] = useState('#64748b');
  const [draftPipeline, setDraftPipeline] = useState<ProjectPipelineConfig | undefined>(undefined);
  const resetProjectSettingsDraft = useCallback((project: ProjectRecord | null = browseProject) => {
    if (!project) {
      setDraftName('');
      setDraftDescription('');
      setDraftIcon('');
      setDraftColor('#64748b');
      setDraftPipeline(undefined);
      return;
    }

    setDraftName(project.name);
    setDraftDescription(project.description);
    setDraftIcon(project.icon || '');
    setDraftColor(project.color || '#64748b');
    setDraftPipeline(project.pipeline ? { ...DEFAULT_PROJECT_PIPELINE, ...project.pipeline } : undefined);
  }, [browseProject]);

  useEffect(() => {
    queueMicrotask(() => {
      if (!browseProject) {
        resetProjectSettingsDraft(null);
        setIsSettingsOpen(false);
        return;
      }

      resetProjectSettingsDraft(browseProject);
    });
  }, [browseProject, resetProjectSettingsDraft]);

  const isProjectSettingsDirty = useMemo(() => {
    if (!browseProject) {
      return false;
    }

    const currentDraft = {
      name: draftName.trim() || browseProject.name,
      description: draftDescription,
      icon: draftIcon,
      color: draftColor || '#64748b',
      pipeline: draftPipeline ?? undefined,
    };
    const savedProject = {
      name: browseProject.name,
      description: browseProject.description,
      icon: browseProject.icon || '',
      color: browseProject.color || '#64748b',
      pipeline: browseProject.pipeline ? { ...DEFAULT_PROJECT_PIPELINE, ...browseProject.pipeline } : undefined,
    };

    return JSON.stringify(currentDraft) !== JSON.stringify(savedProject);
  }, [browseProject, draftColor, draftDescription, draftIcon, draftName, draftPipeline]);

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

  return {
    isSettingsOpen,
    setIsSettingsOpen,
    draftName,
    setDraftName,
    draftDescription,
    setDraftDescription,
    draftIcon,
    setDraftIcon,
    draftColor,
    setDraftColor,
    draftPipeline,
    setDraftPipeline,
    isProjectSettingsDirty,
    resetProjectSettingsDraft,
    confirmDiscardProjectSettingsChanges,
    discardProjectSettingsDraft,
    handleRequestCloseProjectSettings,
  };
}
