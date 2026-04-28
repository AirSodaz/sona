import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { AppConfig } from '../../types/config';
import type { ProjectDefaults, ProjectRecord } from '../../types/project';
import { getPolishPresetOptions } from '../../utils/polishPresets';
import { getSummaryTemplateOptions } from '../../utils/summaryTemplates';
import { Checkbox } from '../Checkbox';
import { Dropdown } from '../Dropdown';
import { IconPicker } from '../IconPicker';
import { FolderIcon, XIcon } from '../Icons';
import { LANGUAGE_OPTIONS } from './constants';

interface ProjectSettingsModalProps {
  isOpen: boolean;
  project: ProjectRecord | null;
  draftName: string;
  draftDescription: string;
  draftIcon: string;
  draftDefaults: ProjectDefaults | null;
  globalConfig: AppConfig;
  onClose: () => void;
  onSave: () => void;
  onDelete: () => void;
  onNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onIconChange: (value: string) => void;
  onDefaultsChange: (defaults: ProjectDefaults) => void;
}

export function ProjectSettingsModal({
  isOpen,
  project,
  draftName,
  draftDescription,
  draftIcon,
  draftDefaults,
  globalConfig,
  onClose,
  onSave,
  onDelete,
  onNameChange,
  onDescriptionChange,
  onIconChange,
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

  const summaryTemplateOptions = getSummaryTemplateOptions(globalConfig.summaryCustomTemplates, t);
  const languageOptions = LANGUAGE_OPTIONS.map((language) => ({
    value: language,
    label: t(`translation.languages.${language}`),
  }));
  const polishPresetOptions = getPolishPresetOptions(globalConfig.polishCustomPresets, t);

  const toggleRuleSetId = (
    key:
      | 'enabledTextReplacementSetIds'
      | 'enabledHotwordSetIds'
      | 'enabledPolishKeywordSetIds'
      | 'enabledSpeakerProfileIds',
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
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: 'var(--spacing-lg) var(--spacing-lg) var(--spacing-md)',
            borderBottom: '1px solid var(--color-border)',
          }}
        >
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

        <div
          className="settings-content-scroll"
          style={{
            padding: 'var(--spacing-lg)',
            flex: 1,
            overflowY: 'auto',
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-lg)',
          }}
        >
          <div className="projects-field">
            <label htmlFor="project-settings-name">
              {t('projects.project_name', { defaultValue: 'Project Name' })}
            </label>
            <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
              <IconPicker icon={draftIcon} onChange={onIconChange} defaultIcon={<FolderIcon />} />
              <input
                id="project-settings-name"
                type="text"
                className="settings-input"
                style={{ flex: 1 }}
                value={draftName}
                onChange={(event) => onNameChange(event.target.value)}
                placeholder={t('projects.new_project_name', { defaultValue: 'Project name' })}
              />
            </div>
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
                value={draftDefaults.summaryTemplateId}
                onChange={(value: string) => onDefaultsChange({
                  ...draftDefaults,
                  summaryTemplateId: value as ProjectDefaults['summaryTemplateId'],
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
                {t('projects.polish_preset', { defaultValue: 'Default Polish Preset' })}
              </label>
              <Dropdown
                value={draftDefaults.polishPresetId}
                onChange={(value: string) => onDefaultsChange({
                  ...draftDefaults,
                  polishPresetId: value,
                })}
                options={polishPresetOptions}
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

            <div className="projects-settings-card">
              <div className="projects-settings-card-title">
                {t('projects.polish_keyword_sets', { defaultValue: 'Enabled Polish Keyword Sets' })}
              </div>
              <div className="projects-settings-card-list">
                {(globalConfig.polishKeywordSets || []).length === 0 ? (
                  <span className="projects-settings-empty-copy">
                    {t('projects.no_polish_keyword_sets', {
                      defaultValue: 'No global polish keyword sets yet.',
                    })}
                  </span>
                ) : (
                  (globalConfig.polishKeywordSets || []).map((set) => (
                    <Checkbox
                      key={set.id}
                      checked={draftDefaults.enabledPolishKeywordSetIds.includes(set.id)}
                      onChange={() => toggleRuleSetId('enabledPolishKeywordSetIds', set.id)}
                      label={set.name}
                    />
                  ))
                )}
              </div>
            </div>

            <div className="projects-settings-card">
              <div className="projects-settings-card-title">
                {t('projects.speaker_profiles', { defaultValue: 'Enabled Speaker Profiles' })}
              </div>
              <div className="projects-settings-card-list">
                {(globalConfig.speakerProfiles || []).length === 0 ? (
                  <span className="projects-settings-empty-copy">
                    {t('projects.no_speaker_profiles', {
                      defaultValue: 'No global speaker profiles yet.',
                    })}
                  </span>
                ) : (
                  (globalConfig.speakerProfiles || []).map((profile) => (
                    <Checkbox
                      key={profile.id}
                      checked={draftDefaults.enabledSpeakerProfileIds.includes(profile.id)}
                      onChange={() => toggleRuleSetId('enabledSpeakerProfileIds', profile.id)}
                      label={profile.name}
                    />
                  ))
                )}
              </div>
            </div>
          </div>
        </div>

        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: 'var(--spacing-md) var(--spacing-lg)',
            borderTop: '1px solid var(--color-border)',
            background: 'var(--color-bg-elevated)',
          }}
        >
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
