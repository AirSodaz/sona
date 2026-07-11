import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2 } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { SummaryIcon } from '../Icons';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { DEFAULT_SUMMARY_TEMPLATE_ID, SummaryCustomTemplate } from '../../types/transcript';
import { SettingsItem, SettingsSection } from './SettingsLayout';
import {
  BUILTIN_SUMMARY_TEMPLATES,
  getSummaryTemplateOptions,
  normalizeSummaryCustomTemplates,
} from '../../utils/summaryTemplates';
import { Dropdown } from '../Dropdown';

export function SettingsSummaryTemplateSection(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const projects = useProjectStore((state) => state.projects);
  const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);

  const [newTemplateName, setNewTemplateName] = useState('');
  const [newTemplateInstructions, setNewTemplateInstructions] = useState('');

  const customTemplates = useMemo(
    () => normalizeSummaryCustomTemplates(config.summaryCustomTemplates),
    [config.summaryCustomTemplates],
  );
  const templateOptions = useMemo(
    () => getSummaryTemplateOptions(customTemplates, t),
    [customTemplates, t],
  );

  const handleAddTemplate = () => {
    const name = newTemplateName.trim();
    const instructions = newTemplateInstructions.trim();
    if (!name || !instructions) {
      return;
    }

    updateConfig({
      summaryCustomTemplates: [
        ...customTemplates,
        {
          id: uuidv4(),
          name,
          instructions,
        },
      ],
    });
    setNewTemplateName('');
    setNewTemplateInstructions('');
  };

  const handleUpdateTemplate = (
    templateId: string,
    updates: Partial<Pick<SummaryCustomTemplate, 'name' | 'instructions'>>,
  ) => {
    if (typeof updates.name === 'string' && !updates.name.trim()) {
      return;
    }

    if (typeof updates.instructions === 'string' && !updates.instructions.trim()) {
      return;
    }

    updateConfig({
      summaryCustomTemplates: customTemplates.map((template) => (
        template.id === templateId
          ? {
            ...template,
            ...updates,
          }
          : template
      )),
    });
  };

  const handleDeleteTemplate = async (templateId: string) => {
    const nextTemplates = customTemplates.filter((template) => template.id !== templateId);
    const nextTemplateId = config.summaryTemplateId === templateId
      ? DEFAULT_SUMMARY_TEMPLATE_ID
      : (config.summaryTemplateId || DEFAULT_SUMMARY_TEMPLATE_ID);

    updateConfig({
      summaryCustomTemplates: nextTemplates,
      summaryTemplateId: nextTemplateId,
    });

    const affectedProjects = projects.filter((project) => project.defaults.summaryTemplateId === templateId);
    await Promise.all(affectedProjects.map((project) => (
      updateProjectDefaults(project.id, { summaryTemplateId: DEFAULT_SUMMARY_TEMPLATE_ID })
    )));
  };

  return (
    <>
      <SettingsSection
        title={t('settings.summary_templates_default_title', { defaultValue: 'Default Summary Template' })}
        description={t('settings.summary_templates_default_description', {
          defaultValue: 'Choose which summary template should be selected by default outside project-specific overrides.',
        })}
        icon={<SummaryIcon width={20} height={20} />}
      >
        <SettingsItem
          title={t('projects.summary_template', { defaultValue: 'Default Summary Template' })}
          hint={t('settings.summary_templates_default_hint', {
            defaultValue: 'Projects can still choose a different summary template in project settings.',
          })}
        >
          <div style={{ width: '280px', maxWidth: '100%' }}>
            <Dropdown
              value={config.summaryTemplateId || DEFAULT_SUMMARY_TEMPLATE_ID}
              onChange={(value) => updateConfig({ summaryTemplateId: value })}
              options={templateOptions}
            />
          </div>
        </SettingsItem>
      </SettingsSection>

      <SettingsSection
        title={t('settings.summary_templates_builtin_title', { defaultValue: 'Built-in Summary Templates' })}
        description={t('settings.summary_templates_builtin_description', {
          defaultValue: 'These templates are provided by Sona and can be viewed here, but not edited.',
        })}
        icon={<SummaryIcon width={20} height={20} />}
      >
        {BUILTIN_SUMMARY_TEMPLATES.map((template) => (
          <div
            key={template.id}
            style={{
              padding: '20px 24px',
              background: 'var(--color-bg-primary)',
              display: 'flex',
              flexDirection: 'column',
              gap: '10px',
            }}
          >
            <div style={{ fontSize: '0.95rem', fontWeight: 600, color: 'var(--color-text-primary)' }}>
              {t(template.labelKey, { defaultValue: template.defaultLabel })}
            </div>
            <div
              style={{
                padding: '12px 14px',
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--color-border)',
                background: 'var(--color-bg-elevated)',
                color: 'var(--color-text-primary)',
                fontSize: '0.875rem',
                lineHeight: 1.5,
                whiteSpace: 'pre-wrap',
              }}
            >
              {template.instructions}
            </div>
          </div>
        ))}
      </SettingsSection>

      <SettingsSection
        title={t('settings.summary_templates_custom_title', { defaultValue: 'Custom Summary Templates' })}
        description={t('settings.summary_templates_custom_description', {
          defaultValue: 'Create reusable summary template names and instructions for different workflows.',
        })}
        icon={<SummaryIcon width={20} height={20} />}
      >
        <div
          style={{
            padding: '20px 24px',
            background: 'var(--color-bg-primary)',
            display: 'flex',
            flexDirection: 'column',
            gap: '12px',
          }}
        >
          <input
            type="text"
            className="settings-input"
            value={newTemplateName}
            onChange={(event) => setNewTemplateName(event.target.value)}
            placeholder={t('settings.summary_template_name_placeholder', { defaultValue: 'Template name' })}
          />
          <textarea
            className="settings-input"
            value={newTemplateInstructions}
            onChange={(event) => setNewTemplateInstructions(event.target.value)}
            placeholder={t('settings.summary_template_instructions_placeholder', {
              defaultValue: 'Enter template instructions...',
            })}
            style={{ minHeight: '96px', resize: 'vertical' }}
          />
          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button
              type="button"
              className="btn btn-primary"
              onClick={handleAddTemplate}
              disabled={!newTemplateName.trim() || !newTemplateInstructions.trim()}
              style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}
            >
              <Plus size={16} />
              {t('settings.add_summary_template', { defaultValue: 'Add Template' })}
            </button>
          </div>
        </div>

        {customTemplates.length === 0 ? (
          <div
            style={{
              padding: '24px',
              background: 'var(--color-bg-primary)',
              color: 'var(--color-text-muted)',
              fontSize: '0.875rem',
            }}
          >
            {t('settings.summary_templates_no_custom', { defaultValue: 'No custom summary templates yet.' })}
          </div>
        ) : customTemplates.map((template) => (
          <div
            key={template.id}
            style={{
              padding: '20px 24px',
              background: 'var(--color-bg-primary)',
              display: 'flex',
              flexDirection: 'column',
              gap: '12px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: '12px' }}>
              <input
                type="text"
                className="settings-input"
                value={template.name}
                onChange={(event) => handleUpdateTemplate(template.id, { name: event.target.value })}
                aria-label={t('settings.summary_template_name_placeholder', { defaultValue: 'Template name' })}
                style={{ flex: 1 }}
              />
              <button
                type="button"
                className="btn btn-icon btn-danger-soft"
                onClick={() => void handleDeleteTemplate(template.id)}
                aria-label={t('common.delete_item', {
                  item: template.name,
                  defaultValue: `Delete ${template.name}`,
                })}
              >
                <Trash2 size={16} />
              </button>
            </div>
            <textarea
              className="settings-input"
              value={template.instructions}
              onChange={(event) => handleUpdateTemplate(template.id, { instructions: event.target.value })}
              aria-label={template.name}
              style={{ minHeight: '120px', resize: 'vertical' }}
            />
          </div>
        ))}
      </SettingsSection>
    </>
  );
}
