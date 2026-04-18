import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, MoveRight } from 'lucide-react';
import { BookIcon } from '../Icons';
import { AppConfig, TextReplacementRule } from '../../types/config';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { Switch } from '../Switch';
import { v4 as uuidv4 } from 'uuid';

interface SettingsVocabularyTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
}

export function SettingsVocabularyTab({
    config,
    updateConfig
}: SettingsVocabularyTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [newFrom, setNewFrom] = useState('');
    const [newTo, setNewTo] = useState('');
    const [newIgnoreCase, setNewIgnoreCase] = useState(false);

    const replacements = config.textReplacements || [];

    const handleAddRule = () => {
        if (!newFrom.trim()) return;

        const newRule: TextReplacementRule = {
            id: uuidv4(),
            from: newFrom.trim(),
            to: newTo.trim(),
            enabled: true,
            ignoreCase: newIgnoreCase
        };

        updateConfig({
            textReplacements: [...replacements, newRule]
        });

        setNewFrom('');
        setNewTo('');
        setNewIgnoreCase(false);
    };

    const handleToggleRule = (id: string) => {
        updateConfig({
            textReplacements: replacements.map(r => 
                r.id === id ? { ...r, enabled: !r.enabled } : r
            )
        });
    };

    const handleDeleteRule = (id: string) => {
        updateConfig({
            textReplacements: replacements.filter(r => r.id !== id)
        });
    };

    const handleUpdateRule = (id: string, updates: Partial<TextReplacementRule>) => {
        updateConfig({
            textReplacements: replacements.map(r => 
                r.id === id ? { ...r, ...updates } : r
            )
        });
    };

    return (
        <SettingsTabContainer id="settings-panel-vocabulary" ariaLabelledby="settings-tab-vocabulary">
            <SettingsPageHeader 
                icon={<BookIcon width={28} height={28} />}
                title={t('settings.vocabulary')} 
                description={t('settings.vocabulary_description', { defaultValue: 'Manage custom vocabulary and text replacement rules.' })} 
            />

            <SettingsSection
                title={t('settings.text_replacement_title', { defaultValue: 'Text Replacement' })}
                icon={<BookIcon size={20} />}
                description={t('settings.text_replacement_description', { defaultValue: 'Automatically replace specific words or phrases in transcription results.' })}
            >
                {/* Add New Rule */}
                <div style={{ 
                    display: 'flex', 
                    flexDirection: 'column',
                    gap: '16px', 
                    padding: '24px',
                    background: 'var(--color-bg-primary)',
                }}>
                    <div style={{ display: 'flex', gap: '12px', alignItems: 'flex-end' }}>
                        <div style={{ flex: 1 }}>
                            <label style={{ display: 'block', fontSize: '0.85rem', marginBottom: '4px', color: 'var(--color-text-muted)' }}>
                                {t('settings.find', { defaultValue: 'Find' })}
                            </label>
                            <input
                                type="text"
                                className="settings-input"
                                value={newFrom}
                                onChange={(e) => setNewFrom(e.target.value)}
                                placeholder={t('settings.find_placeholder', { defaultValue: 'e.g. sona' })}
                                style={{ width: '100%' }}
                            />
                        </div>
                        <div style={{ flex: 1 }}>
                            <label style={{ display: 'block', fontSize: '0.85rem', marginBottom: '4px', color: 'var(--color-text-muted)' }}>
                                {t('settings.replace_with', { defaultValue: 'Replace with' })}
                            </label>
                            <input
                                type="text"
                                className="settings-input"
                                value={newTo}
                                onChange={(e) => setNewTo(e.target.value)}
                                placeholder={t('settings.replace_placeholder', { defaultValue: 'e.g. Sona' })}
                                style={{ width: '100%' }}
                            />
                        </div>
                        <button 
                            className="btn btn-primary" 
                            onClick={handleAddRule}
                            disabled={!newFrom.trim()}
                            style={{ height: '38px', display: 'flex', alignItems: 'center', gap: '6px', padding: '0 20px' }}
                        >
                            <Plus size={18} />
                            {t('common.add', { defaultValue: 'Add' })}
                        </button>
                    </div>
                    
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <Switch 
                            checked={newIgnoreCase} 
                            onChange={(checked) => setNewIgnoreCase(checked)} 
                        />
                        <span style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>
                            {t('settings.ignore_case', { defaultValue: 'Ignore Case' })}
                        </span>
                    </div>
                </div>

                {/* Rules List */}
                <div className="settings-list" style={{ background: 'var(--color-bg-primary)' }}>
                    {replacements.length === 0 ? (
                        <div style={{ 
                            padding: '48px 24px', 
                            textAlign: 'center', 
                            color: 'var(--color-text-muted)',
                            background: 'var(--color-bg-primary)'
                        }}>
                            {t('settings.no_rules', { defaultValue: 'No replacement rules defined.' })}
                        </div>
                    ) : (
                        replacements.map((rule) => (
                            <div key={rule.id} style={{ 
                                display: 'flex', 
                                flexDirection: 'column',
                                gap: '12px',
                                padding: '20px 24px',
                                borderTop: '1px solid var(--color-border-subtle)',
                                opacity: rule.enabled ? 1 : 0.6,
                                transition: 'opacity 0.2s ease'
                            }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                                    <div style={{ display: 'flex', alignItems: 'center' }}>
                                        <Switch 
                                            checked={rule.enabled} 
                                            onChange={() => handleToggleRule(rule.id)} 
                                        />
                                    </div>
                                    <div style={{ flex: 1, display: 'flex', gap: '12px', alignItems: 'center' }}>
                                        <input
                                            type="text"
                                            className="settings-input-minimal"
                                            value={rule.from}
                                            onChange={(e) => handleUpdateRule(rule.id, { from: e.target.value })}
                                            style={{ fontWeight: 600, fontSize: '0.95rem' }}
                                        />
                                        <MoveRight size={16} style={{ color: 'var(--color-text-muted)', flexShrink: 0 }} />
                                        <input
                                            type="text"
                                            className="settings-input-minimal"
                                            value={rule.to}
                                            onChange={(e) => handleUpdateRule(rule.id, { to: e.target.value })}
                                            style={{ fontSize: '0.95rem' }}
                                        />
                                    </div>
                                    <button 
                                        className="btn btn-icon btn-danger-soft"
                                        onClick={() => handleDeleteRule(rule.id)}
                                        title={t('common.delete')}
                                    >
                                        <Trash2 size={18} />
                                    </button>
                                </div>
                                <div style={{ marginLeft: '48px', display: 'flex', alignItems: 'center', gap: '8px' }}>
                                    <Switch 
                                        checked={!!rule.ignoreCase} 
                                        onChange={(checked) => handleUpdateRule(rule.id, { ignoreCase: checked })} 
                                        style={{ transform: 'scale(0.8)' }}
                                    />
                                    <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', fontWeight: 500 }}>
                                        {t('settings.ignore_case', { defaultValue: 'Ignore Case' })}
                                    </span>
                                </div>
                            </div>
                        ))
                    )}
                </div>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
