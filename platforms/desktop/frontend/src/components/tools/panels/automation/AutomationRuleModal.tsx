import { FolderOpen } from 'lucide-react';
import type React from 'react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LANGUAGE_OPTIONS } from '../../../../constants/languages';
import { resolveItemPipeline } from '../../../../services/projectPipeline';
import { openDialog } from '../../../../services/tauri/platform/dialog';
import { useAutomationStore } from '../../../../stores/automationStore';
import { useConfigStore } from '../../../../stores/configStore';
import { useDialogStore } from '../../../../stores/dialogStore';
import { useProjectStore } from '../../../../stores/projectStore';
import type { AutomationRule } from '../../../../types/automation';
import type { ExportFormat, ExportMode } from '../../../../utils/exportFormats';
import { getLocalizedLanguageName } from '../../../../utils/languageUtils';
import { coercePolishPresetId, getPolishPresetOptions } from '../../../../utils/polishPresets';
import { Dropdown } from '../../../Dropdown';
import { Modal } from '../../../Modal';
import { Switch } from '../../../Switch';
import './ToolAutomationPanel.css';

interface AutomationRuleModalProps {
  rule: AutomationRule | null;
  onClose: () => void;
}

const EXPORT_FORMATS: ExportFormat[] = ['txt', 'srt', 'vtt', 'md', 'json'];
const EXPORT_MODES: ExportMode[] = ['original', 'translation', 'bilingual'];

export function AutomationRuleModal({
  rule,
  onClose,
}: AutomationRuleModalProps): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const saveRule = useAutomationStore((state) => state.saveRule);
  const projects = useProjectStore((state) => state.projects);
  const config = useConfigStore((state) => state.config);
  const alert = useDialogStore((state) => state.alert);
  const showError = useDialogStore((state) => state.showError);

  const [name, setName] = useState(rule?.name ?? '');
  const [watchDirectory, setWatchDirectory] = useState(rule?.watchDirectory ?? '');
  const [recursive, setRecursive] = useState(rule?.recursive ?? false);
  const [saveHistory, setSaveHistory] = useState(rule?.saveHistory ?? true);
  const initialProjectId =
    rule?.projectId && (rule.projectId === 'inbox' || projects.some((p) => p.id === rule.projectId))
      ? rule.projectId
      : 'inbox';
  const [projectId, setProjectId] = useState(initialProjectId);
  const [exportDirectory, setExportDirectory] = useState(rule?.exportConfig?.directory ?? '');
  const [exportFormat, setExportFormat] = useState<ExportFormat>(
    rule?.exportConfig?.format ?? 'txt'
  );
  const [exportMode, setExportMode] = useState<ExportMode>(rule?.exportConfig?.mode ?? 'original');
  const [autoPolish, setAutoPolish] = useState(rule?.stageConfig?.autoPolish ?? false);
  const [polishPresetId, setPolishPresetId] = useState(
    coercePolishPresetId(rule?.stageConfig?.polishPresetId, config.polishCustomPresets)
  );
  const [autoTranslate, setAutoTranslate] = useState(rule?.stageConfig?.autoTranslate ?? false);
  const [translationLanguage, setTranslationLanguage] = useState(
    rule?.stageConfig?.translationLanguage || 'en'
  );
  const [saving, setSaving] = useState(false);

  const projectOptions = useMemo(
    () => [
      { value: 'inbox', label: t('projects.inbox', { defaultValue: 'Inbox' }) },
      ...projects.map((project) => ({ value: project.id, label: project.name })),
    ],
    [projects, t]
  );
  const polishOptions = useMemo(
    () => getPolishPresetOptions(config.polishCustomPresets, t),
    [config.polishCustomPresets, t]
  );
  const languageOptions = useMemo(
    () =>
      LANGUAGE_OPTIONS.map((lang) => ({
        value: lang.code,
        label: getLocalizedLanguageName(lang.code, i18n.language || 'en'),
      })),
    [i18n.language]
  );
  const pipeline = useMemo(() => {
    const resolvedProjectId = projectId && projectId !== 'inbox' ? projectId : null;
    return resolveItemPipeline(resolvedProjectId, projects, config);
  }, [config, projectId, projects]);

  const browse = async (current: string, apply: (path: string) => void) => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath: current || undefined,
    });
    if (typeof selected === 'string' && selected) {
      apply(selected);
    }
  };

  const handleSave = async () => {
    if (!name.trim() || !watchDirectory.trim()) {
      await alert(
        t('automation.required_fields', {
          defaultValue: 'Complete the rule name and watch directory before saving.',
        }),
        { variant: 'warning' }
      );
      return;
    }
    if (!saveHistory && !exportDirectory.trim()) {
      await alert(
        t('automation.export_directory_required', {
          defaultValue: 'Choose an output directory for export-only rules.',
        }),
        { variant: 'warning' }
      );
      return;
    }

    setSaving(true);
    try {
      await saveRule({
        id: rule?.id,
        name,
        saveHistory,
        projectId: saveHistory ? projectId || 'inbox' : '',
        watchDirectory,
        recursive,
        enabled: rule?.enabled ?? false,
        stageConfig: {
          autoPolish: saveHistory ? false : autoPolish,
          polishPresetId,
          autoTranslate: saveHistory ? false : autoTranslate,
          translationLanguage,
        },
        exportConfig: {
          directory: saveHistory ? '' : exportDirectory,
          format: exportFormat,
          mode: autoTranslate ? exportMode : 'original',
          prefix: rule?.exportConfig?.prefix ?? '',
        },
      });
      onClose();
    } catch (error) {
      await showError({
        code: 'automation.save_failed',
        messageKey: 'errors.automation.save_failed',
        cause: error,
      });
      setSaving(false);
    }
  };

  const enabledStages = [
    pipeline.autoPolish ? t('automation.stage.polish', { defaultValue: 'Auto Polish' }) : null,
    pipeline.autoTranslate
      ? t('automation.stage.translate', { defaultValue: 'Auto Translate' })
      : null,
    pipeline.autoSummary ? t('automation.stage.summary', { defaultValue: 'Auto Summary' }) : null,
    pipeline.autoExport ? t('automation.stage.export', { defaultValue: 'Auto Export' }) : null,
  ].filter((stage): stage is string => Boolean(stage));

  return (
    <Modal
      isOpen
      onClose={onClose}
      size="md"
      title={
        rule
          ? t('automation.edit_rule', { defaultValue: 'Edit Rule' })
          : t('automation.new_rule', { defaultValue: 'New Rule' })
      }
      footer={
        <div className="automation-modal-footer">
          <button type="button" className="btn btn-secondary" onClick={onClose} disabled={saving}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void handleSave()}
            disabled={saving}
          >
            {t('common.save', { defaultValue: 'Save' })}
          </button>
        </div>
      }
    >
      <div className="automation-rule-form">
        <label className="automation-field">
          <span>{t('automation.rule_name', { defaultValue: 'Rule Name' })}</span>
          <input
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t('automation.rule_name_placeholder', {
              defaultValue: 'e.g. Weekly Meeting Inbox',
            })}
          />
        </label>
        <label className="automation-field">
          <span>{t('automation.watch_directory', { defaultValue: 'Watch Directory' })}</span>
          <div className="automation-path-row">
            <input
              type="text"
              value={watchDirectory}
              onChange={(event) => setWatchDirectory(event.target.value)}
              placeholder={t('automation.watch_directory_placeholder', {
                defaultValue: 'Choose a folder to monitor',
              })}
            />
            <button
              type="button"
              className="btn btn-secondary automation-browse-btn"
              aria-label={t('automation.browse', { defaultValue: 'Browse' })}
              onClick={() => void browse(watchDirectory, setWatchDirectory)}
            >
              <FolderOpen size={16} />
            </button>
          </div>
        </label>
        <div className="automation-switch-row">
          <span>{t('automation.recursive', { defaultValue: 'Include Subdirectories' })}</span>
          <Switch
            checked={recursive}
            onChange={setRecursive}
            aria-label={t('automation.recursive', { defaultValue: 'Include Subdirectories' })}
          />
        </div>
        <div
          className="automation-mode-switch"
          role="group"
          aria-label={t('automation.save_mode', { defaultValue: 'Save mode' })}
        >
          <button
            type="button"
            className={saveHistory ? 'is-active' : ''}
            aria-pressed={saveHistory}
            onClick={() => setSaveHistory(true)}
          >
            {t('automation.save_to_project', { defaultValue: 'Save to Project' })}
          </button>
          <button
            type="button"
            className={!saveHistory ? 'is-active' : ''}
            aria-pressed={!saveHistory}
            onClick={() => setSaveHistory(false)}
          >
            {t('automation.export_only', { defaultValue: 'Export Only (No History)' })}
          </button>
        </div>
        {saveHistory ? (
          <>
            <label className="automation-field">
              <span>{t('automation.target_project', { defaultValue: 'Target Project' })}</span>
              <Dropdown
                options={projectOptions}
                value={projectId || 'inbox'}
                onChange={setProjectId}
                aria-label={t('automation.target_project', { defaultValue: 'Target Project' })}
              />
            </label>
            <div className="automation-pipeline-card">
              <span className="automation-pipeline-card-label">
                {t('automation.pipeline_preset', { defaultValue: 'Project Pipeline' })}
              </span>
              {enabledStages.length > 0 ? (
                <ul className="automation-pipeline-stages">
                  {enabledStages.map((stage) => (
                    <li key={stage} className="automation-pipeline-stage-chip">
                      <span className="automation-stage-dot" aria-hidden="true" />
                      {stage}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="automation-pipeline-empty">
                  {t('automation.transcribe_only', {
                    defaultValue: 'Transcribe only — no project pipeline enabled',
                  })}
                </p>
              )}
            </div>
          </>
        ) : (
          <>
            <label className="automation-field">
              <span>{t('automation.output_directory', { defaultValue: 'Output Directory' })}</span>
              <div className="automation-path-row">
                <input
                  type="text"
                  value={exportDirectory}
                  onChange={(event) => setExportDirectory(event.target.value)}
                  placeholder={t('automation.output_directory_placeholder', {
                    defaultValue: 'Choose an export directory',
                  })}
                />
                <button
                  type="button"
                  className="btn btn-secondary automation-browse-btn"
                  aria-label={t('automation.browse_output', { defaultValue: 'Browse output' })}
                  onClick={() => void browse(exportDirectory, setExportDirectory)}
                >
                  <FolderOpen size={16} />
                </button>
              </div>
            </label>
            <label className="automation-field">
              <span>{t('automation.export_format', { defaultValue: 'Export Format' })}</span>
              <Dropdown
                options={EXPORT_FORMATS.map((format) => ({
                  value: format,
                  label: format.toUpperCase(),
                }))}
                value={exportFormat}
                onChange={(value) => setExportFormat(value as ExportFormat)}
                aria-label={t('automation.export_format', { defaultValue: 'Export Format' })}
              />
            </label>
            <div className="automation-switch-row">
              <span>{t('automation.auto_polish', { defaultValue: 'Auto Polish' })}</span>
              <Switch
                checked={autoPolish}
                onChange={setAutoPolish}
                aria-label={t('automation.auto_polish', { defaultValue: 'Auto Polish' })}
              />
            </div>
            {autoPolish ? (
              <div className="automation-nested-field">
                <label className="automation-field">
                  <span>{t('automation.polish_preset', { defaultValue: 'Polish Preset' })}</span>
                  <Dropdown
                    options={polishOptions}
                    value={coercePolishPresetId(polishPresetId, config.polishCustomPresets)}
                    onChange={setPolishPresetId}
                    aria-label={t('automation.polish_preset', { defaultValue: 'Polish Preset' })}
                  />
                </label>
              </div>
            ) : null}
            <div className="automation-switch-row">
              <span>{t('automation.auto_translate', { defaultValue: 'Auto Translate' })}</span>
              <Switch
                checked={autoTranslate}
                onChange={setAutoTranslate}
                aria-label={t('automation.auto_translate', { defaultValue: 'Auto Translate' })}
              />
            </div>
            {autoTranslate ? (
              <div className="automation-nested-grid">
                <label className="automation-field">
                  <span>
                    {t('automation.target_language', { defaultValue: 'Target Language' })}
                  </span>
                  <Dropdown
                    options={languageOptions}
                    value={translationLanguage}
                    onChange={setTranslationLanguage}
                    aria-label={t('automation.target_language', {
                      defaultValue: 'Target Language',
                    })}
                  />
                </label>
                <label className="automation-field">
                  <span>{t('automation.export_mode', { defaultValue: 'Export Mode' })}</span>
                  <Dropdown
                    options={EXPORT_MODES.map((mode) => ({ value: mode, label: mode }))}
                    value={exportMode}
                    onChange={(value) => setExportMode(value as ExportMode)}
                    aria-label={t('automation.export_mode', { defaultValue: 'Export Mode' })}
                  />
                </label>
              </div>
            ) : null}
            <p className="automation-export-notice">
              {t('automation.export_only_notice', {
                defaultValue: 'Export-only rules do not save history or run auto summary.',
              })}
            </p>
          </>
        )}
      </div>
    </Modal>
  );
}
