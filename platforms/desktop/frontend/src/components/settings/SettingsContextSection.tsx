import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2 } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { FileTextIcon } from '../Icons';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { SettingsItem, SettingsSection } from './SettingsLayout';
import {
  BUILTIN_POLISH_PRESETS,
  DEFAULT_POLISH_PRESET_ID,
  getPolishPresetOptions,
  normalizePolishCustomPresets,
} from '../../utils/polishPresets';
import { Dropdown } from '../Dropdown';

export function SettingsContextSection(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useLlmAssistantConfig();
    const updateConfig = useSetConfig();
    const projects = useProjectStore((state) => state.projects);
    const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);

    const [newPresetName, setNewPresetName] = useState('');
    const [newPresetContext, setNewPresetContext] = useState('');

    const customPresets = useMemo(
        () => normalizePolishCustomPresets(config.polishCustomPresets),
        [config.polishCustomPresets],
    );
    const presetOptions = useMemo(
        () => getPolishPresetOptions(customPresets, t),
        [customPresets, t],
    );

    const handleAddPreset = () => {
        const name = newPresetName.trim();
        const context = newPresetContext.trim();
        if (!name || !context) {
            return;
        }

        updateConfig({
            polishCustomPresets: [
                ...customPresets,
                {
                    id: uuidv4(),
                    name,
                    context,
                },
            ],
        });
        setNewPresetName('');
        setNewPresetContext('');
    };

    const handleUpdatePreset = (presetId: string, updates: { name?: string; context?: string }) => {
        if (typeof updates.name === 'string' && !updates.name.trim()) {
            return;
        }

        if (typeof updates.context === 'string' && !updates.context.trim()) {
            return;
        }

        updateConfig({
            polishCustomPresets: customPresets.map((preset) => (
                preset.id === presetId
                    ? {
                        ...preset,
                        ...updates,
                    }
                    : preset
            )),
        });
    };

    const handleDeletePreset = async (presetId: string) => {
        const nextPresets = customPresets.filter((preset) => preset.id !== presetId);
        const nextPresetId = config.polishPresetId === presetId
            ? DEFAULT_POLISH_PRESET_ID
            : (config.polishPresetId || DEFAULT_POLISH_PRESET_ID);

        updateConfig({
            polishCustomPresets: nextPresets,
            polishPresetId: nextPresetId,
        });

        const affectedProjects = projects.filter((project) => project.defaults.polishPresetId === presetId);
        await Promise.all(affectedProjects.map((project) => (
            updateProjectDefaults(project.id, { polishPresetId: DEFAULT_POLISH_PRESET_ID })
        )));
    };

    return (
        <>
            <SettingsSection
                title={t('settings.context_default_title', { defaultValue: 'Default Polish Preset' })}
                description={t('settings.context_default_description', {
                    defaultValue: 'Choose which preset text polishing should use by default outside project-specific overrides.',
                })}
                icon={<FileTextIcon width={20} height={20} />}
            >
                <SettingsItem
                    title={t('projects.polish_preset', { defaultValue: 'Default Polish Preset' })}
                    hint={t('settings.context_default_hint', {
                        defaultValue: 'Projects can still override this with their own preset selection.',
                    })}
                >
                    <div style={{ width: '280px', maxWidth: '100%' }}>
                        <Dropdown
                            value={config.polishPresetId || DEFAULT_POLISH_PRESET_ID}
                            onChange={(value) => updateConfig({ polishPresetId: value })}
                            options={presetOptions}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <SettingsSection
                title={t('settings.context_builtin_title', { defaultValue: 'Built-in Presets' })}
                description={t('settings.context_builtin_description', {
                    defaultValue: 'These presets are provided by Sona and can be viewed here, but not edited.',
                })}
                icon={<FileTextIcon width={20} height={20} />}
            >
                {BUILTIN_POLISH_PRESETS.map((preset) => (
                    <div
                        key={preset.id}
                        style={{
                            padding: '20px 24px',
                            background: 'var(--color-bg-primary)',
                            display: 'flex',
                            flexDirection: 'column',
                            gap: '10px',
                        }}
                    >
                        <div style={{ fontSize: '0.95rem', fontWeight: 600, color: 'var(--color-text-primary)' }}>
                            {t(preset.labelKey, { defaultValue: preset.defaultLabel })}
                        </div>
                        <div
                            style={{
                                padding: '12px 14px',
                                borderRadius: 'var(--radius-sm)',
                                border: '1px solid var(--color-border)',
                                background: 'var(--color-bg-elevated)',
                                color: preset.context
                                    ? 'var(--color-text-primary)'
                                    : 'var(--color-text-muted)',
                                fontSize: '0.875rem',
                                lineHeight: 1.5,
                                whiteSpace: 'pre-wrap',
                            }}
                        >
                            {preset.context || t('settings.context_empty_builtin', {
                                defaultValue: 'No extra context. Sona will polish without adding a preset-specific note.',
                            })}
                        </div>
                    </div>
                ))}
            </SettingsSection>

            <SettingsSection
                title={t('settings.context_custom_title', { defaultValue: 'Custom Presets' })}
                description={t('settings.context_custom_description', {
                    defaultValue: 'Create reusable name + context pairs and switch between them from text polish or project settings.',
                })}
                icon={<FileTextIcon width={20} height={20} />}
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
                        value={newPresetName}
                        onChange={(event) => setNewPresetName(event.target.value)}
                        placeholder={t('settings.context_name_placeholder', { defaultValue: 'Preset name' })}
                    />
                    <textarea
                        className="settings-input"
                        value={newPresetContext}
                        onChange={(event) => setNewPresetContext(event.target.value)}
                        placeholder={t('settings.context_placeholder', { defaultValue: 'Enter preset context...' })}
                        style={{ minHeight: '96px', resize: 'vertical' }}
                    />
                    <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                        <button
                            type="button"
                            className="btn btn-primary"
                            onClick={handleAddPreset}
                            disabled={!newPresetName.trim() || !newPresetContext.trim()}
                            style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}
                        >
                            <Plus size={16} />
                            {t('settings.add_context_preset', { defaultValue: 'Add Preset' })}
                        </button>
                    </div>
                </div>

                {customPresets.length === 0 ? (
                    <div
                        style={{
                            padding: '24px',
                            background: 'var(--color-bg-primary)',
                            color: 'var(--color-text-muted)',
                            fontSize: '0.875rem',
                        }}
                    >
                        {t('settings.context_no_custom_presets', { defaultValue: 'No custom presets yet.' })}
                    </div>
                ) : customPresets.map((preset) => (
                    <div
                        key={preset.id}
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
                                value={preset.name}
                                onChange={(event) => handleUpdatePreset(preset.id, { name: event.target.value })}
                                aria-label={t('settings.context_name_placeholder', { defaultValue: 'Preset name' })}
                                style={{ flex: 1 }}
                            />
                            <button
                                type="button"
                                className="btn btn-icon btn-danger-soft"
                                onClick={() => void handleDeletePreset(preset.id)}
                                aria-label={t('common.delete_item', {
                                    item: preset.name,
                                    defaultValue: `Delete ${preset.name}`,
                                })}
                            >
                                <Trash2 size={16} />
                            </button>
                        </div>
                        <textarea
                            className="settings-input"
                            value={preset.context}
                            onChange={(event) => handleUpdatePreset(preset.id, { context: event.target.value })}
                            aria-label={preset.name}
                            style={{ minHeight: '120px', resize: 'vertical' }}
                        />
                    </div>
                ))}
            </SettingsSection>
        </>
    );
}
