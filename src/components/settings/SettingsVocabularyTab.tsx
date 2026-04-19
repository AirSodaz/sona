import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, MoveRight, ChevronDown, ChevronRight } from 'lucide-react';
import { BookIcon } from '../Icons';
import { AppConfig, TextReplacementRuleSet, TextReplacementRule } from '../../types/config';
import { SettingsTabContainer, SettingsSection, SettingsPageHeader } from './SettingsLayout';
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
    const [newSetName, setNewSetName] = useState('');
    const [expandedSets, setExpandedSets] = useState<Set<string>>(new Set());

    const sets = config.textReplacementSets || [];

    const toggleSetExpanded = (id: string) => {
        const newExpanded = new Set(expandedSets);
        if (newExpanded.has(id)) {
            newExpanded.delete(id);
        } else {
            newExpanded.add(id);
        }
        setExpandedSets(newExpanded);
    };

    const handleAddSet = () => {
        if (!newSetName.trim()) return;

        const newSet: TextReplacementRuleSet = {
            id: uuidv4(),
            name: newSetName.trim(),
            enabled: true,
            ignoreCase: false,
            rules: []
        };

        updateConfig({
            textReplacementSets: [...sets, newSet]
        });

        setNewSetName('');
        setExpandedSets(prev => new Set(prev).add(newSet.id));
    };

    const handleUpdateSet = (id: string, updates: Partial<TextReplacementRuleSet>) => {
        updateConfig({
            textReplacementSets: sets.map(s => s.id === id ? { ...s, ...updates } : s)
        });
    };

    const handleDeleteSet = (id: string) => {
        updateConfig({
            textReplacementSets: sets.filter(s => s.id !== id)
        });
    };

    const handleAddRuleToSet = (setId: string) => {
        updateConfig({
            textReplacementSets: sets.map(s => {
                if (s.id !== setId) return s;
                return {
                    ...s,
                    rules: [...s.rules, { id: uuidv4(), from: '', to: '' }]
                };
            })
        });
    };

    const handleUpdateRuleInSet = (setId: string, ruleId: string, updates: Partial<TextReplacementRule>) => {
        updateConfig({
            textReplacementSets: sets.map(s => {
                if (s.id !== setId) return s;
                return {
                    ...s,
                    rules: s.rules.map(r => r.id === ruleId ? { ...r, ...updates } : r)
                };
            })
        });
    };

    const handleDeleteRuleFromSet = (setId: string, ruleId: string) => {
        updateConfig({
            textReplacementSets: sets.map(s => {
                if (s.id !== setId) return s;
                return {
                    ...s,
                    rules: s.rules.filter(r => r.id !== ruleId)
                };
            })
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
                icon={<BookIcon width={20} height={20} />}
                description={t('settings.text_replacement_description', { defaultValue: 'Group rules into sets to easily enable or disable them.' })}
            >
                {/* Add New Set */}
                <div style={{ 
                    display: 'flex', 
                    gap: '12px', 
                    padding: '24px',
                    background: 'var(--color-bg-primary)',
                    alignItems: 'flex-end',
                    borderBottom: '1px solid var(--color-border-subtle)'
                }}>
                    <div style={{ flex: 1 }}>
                        <label style={{ display: 'block', fontSize: '0.85rem', marginBottom: '4px', color: 'var(--color-text-muted)' }}>
                            {t('settings.rule_set_name', { defaultValue: 'Rule Set Name' })}
                        </label>
                        <input
                            type="text"
                            className="settings-input"
                            value={newSetName}
                            onChange={(e) => setNewSetName(e.target.value)}
                            placeholder={t('settings.rule_set_name_placeholder', { defaultValue: 'e.g. Technical Terms' })}
                            style={{ width: '100%' }}
                        />
                    </div>
                    <button 
                        className="btn btn-primary" 
                        onClick={handleAddSet}
                        disabled={!newSetName.trim()}
                        style={{ height: '38px', display: 'flex', alignItems: 'center', gap: '6px', padding: '0 20px' }}
                    >
                        <Plus size={18} />
                        {t('settings.add_rule_set', { defaultValue: 'Add Set' })}
                    </button>
                </div>

                {/* Sets List */}
                <div className="settings-list" style={{ background: 'var(--color-bg-primary)' }}>
                    {sets.length === 0 ? (
                        <div style={{ 
                            padding: '48px 24px', 
                            textAlign: 'center', 
                            color: 'var(--color-text-muted)',
                            background: 'var(--color-bg-primary)'
                        }}>
                            {t('settings.no_rule_sets', { defaultValue: 'No rule sets defined.' })}
                        </div>
                    ) : (
                        sets.map((set, index) => (
                            <div key={set.id} style={{ 
                                borderBottom: index === sets.length - 1 ? 'none' : '1px solid var(--color-border-subtle)',
                                background: set.enabled ? 'transparent' : 'var(--color-bg-secondary-soft)',
                            }}>
                                {/* Set Header */}
                                <div style={{ 
                                    display: 'flex', 
                                    alignItems: 'center', 
                                    gap: '12px',
                                    padding: '16px 24px',
                                    cursor: 'pointer'
                                }} onClick={() => toggleSetExpanded(set.id)}>
                                    <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)' }}>
                                        {expandedSets.has(set.id) ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                                    </div>
                                    
                                    <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '12px' }}>
                                        <input
                                            type="text"
                                            className="settings-input-minimal"
                                            value={set.name}
                                            onClick={(e) => e.stopPropagation()}
                                            onChange={(e) => handleUpdateSet(set.id, { name: e.target.value })}
                                            style={{ fontWeight: 600, fontSize: '1rem', width: 'auto', minWidth: '150px' }}
                                        />
                                        <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', background: 'var(--color-bg-secondary)', padding: '2px 8px', borderRadius: '10px' }}>
                                            {set.rules.length} {t('settings.rules_count', { count: set.rules.length, defaultValue: 'rules' })}
                                        </span>
                                    </div>

                                    <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }} onClick={(e) => e.stopPropagation()}>
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                            <Switch 
                                                checked={set.ignoreCase} 
                                                onChange={(checked) => handleUpdateSet(set.id, { ignoreCase: checked })} 
                                                style={{ transform: 'scale(0.8)' }}
                                            />
                                            <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', fontWeight: 500 }}>
                                                {t('settings.ignore_case')}
                                            </span>
                                        </div>
                                        
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                            <Switch 
                                                checked={set.enabled} 
                                                onChange={(checked) => handleUpdateSet(set.id, { enabled: checked })} 
                                            />
                                        </div>

                                        <button 
                                            className="btn btn-icon btn-danger-soft"
                                            onClick={() => handleDeleteSet(set.id)}
                                            title={t('common.delete')}
                                        >
                                            <Trash2 size={18} />
                                        </button>
                                    </div>
                                </div>

                                {/* Rules under this set */}
                                {expandedSets.has(set.id) && (
                                    <div style={{ 
                                        padding: '0 24px 24px 56px',
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '8px'
                                    }}>
                                        {set.rules.map((rule) => (
                                            <div key={rule.id} style={{ 
                                                display: 'flex', 
                                                alignItems: 'center', 
                                                gap: '12px',
                                                padding: '8px 12px',
                                                background: 'var(--color-bg-secondary)',
                                                borderRadius: 'var(--radius-md)'
                                            }}>
                                                <div style={{ flex: 1, display: 'flex', gap: '12px', alignItems: 'center' }}>
                                                    <input
                                                        type="text"
                                                        className="settings-input-minimal"
                                                        value={rule.from}
                                                        onChange={(e) => handleUpdateRuleInSet(set.id, rule.id, { from: e.target.value })}
                                                        placeholder={t('settings.find')}
                                                        style={{ fontWeight: 500 }}
                                                    />
                                                    <MoveRight size={14} style={{ color: 'var(--color-text-muted)', flexShrink: 0 }} />
                                                    <input
                                                        type="text"
                                                        className="settings-input-minimal"
                                                        value={rule.to}
                                                        onChange={(e) => handleUpdateRuleInSet(set.id, rule.id, { to: e.target.value })}
                                                        placeholder={t('settings.replace_with')}
                                                    />
                                                </div>
                                                <button 
                                                    className="btn btn-icon btn-danger-soft"
                                                    onClick={() => handleDeleteRuleFromSet(set.id, rule.id)}
                                                    style={{ padding: '4px' }}
                                                >
                                                    <Trash2 size={14} />
                                                </button>
                                            </div>
                                        ))}
                                        
                                        <button 
                                            className="btn btn-secondary-soft" 
                                            onClick={() => handleAddRuleToSet(set.id)}
                                            style={{ alignSelf: 'flex-start', marginTop: '4px', fontSize: '0.85rem', display: 'flex', alignItems: 'center', gap: '6px', padding: '6px 16px' }}
                                        >
                                            <Plus size={16} />
                                            {t('common.add')}
                                        </button>
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
