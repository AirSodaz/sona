import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, ChevronDown, ChevronRight, FileText, List } from 'lucide-react';
import { BookIcon } from '../Icons';
import { TextReplacementRuleSet, TextReplacementRule, HotwordRuleSet, HotwordRule, PolishKeywordRuleSet } from '../../types/config';
import { useVocabularyConfig, useSetConfig } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import type { ProjectDefaults } from '../../types/project';
import { SettingsTabContainer, SettingsSection, SettingsPageHeader } from './SettingsLayout';
import { Switch } from '../Switch';
import { v4 as uuidv4 } from 'uuid';
import { SettingsContextSection } from './SettingsContextSection';
import { SettingsSummaryTemplateSection } from './SettingsSummaryTemplateSection';
import { SettingsSpeakerProfilesSection } from './SettingsSpeakerProfilesSection';
import { normalizePolishKeywordSets } from '../../utils/polishKeywords';

export function SettingsVocabularyTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useVocabularyConfig();
    const updateConfig = useSetConfig();
    const projects = useProjectStore((state) => state.projects);
    const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);
    
    // State for Text Replacement Sets
    const [newSetName, setNewSetName] = useState('');
    const [expandedSets, setExpandedSets] = useState<Set<string>>(new Set());
    const [batchEditingSets, setBatchEditingSets] = useState<Set<string>>(new Set());

    // State for Hotword Sets
    const [newHotwordSetName, setNewHotwordSetName] = useState('');
    const [expandedHotwordSets, setExpandedHotwordSets] = useState<Set<string>>(new Set());
    const [batchEditingHotwordSets, setBatchEditingHotwordSets] = useState<Set<string>>(new Set());

    // State for Polish Keyword Sets
    const [newPolishKeywordSetName, setNewPolishKeywordSetName] = useState('');
    const [expandedPolishKeywordSets, setExpandedPolishKeywordSets] = useState<Set<string>>(new Set());

    const sets = config.textReplacementSets || [];
    const hotwordSets = config.hotwordSets || [];
    const polishKeywordSets = normalizePolishKeywordSets(config.polishKeywordSets);

    const removeRuleSetReferenceFromProjects = async (
        key: 'enabledTextReplacementSetIds' | 'enabledHotwordSetIds' | 'enabledPolishKeywordSetIds',
        setId: string,
    ) => {
        const affectedProjects = projects.filter((project) => project.defaults[key].includes(setId));
        if (affectedProjects.length === 0) {
            return;
        }

        await Promise.all(affectedProjects.map((project) => (
            updateProjectDefaults(project.id, {
                [key]: project.defaults[key].filter((id) => id !== setId),
            } as Pick<ProjectDefaults, typeof key>)
        )));
    };

    // --- Text Replacement Handlers ---

    const toggleSetExpanded = (id: string) => {
        const newExpanded = new Set(expandedSets);
        if (newExpanded.has(id)) {
            newExpanded.delete(id);
        } else {
            newExpanded.add(id);
        }
        setExpandedSets(newExpanded);
    };

    const toggleBatchMode = (id: string) => {
        const newBatch = new Set(batchEditingSets);
        if (newBatch.has(id)) {
            newBatch.delete(id);
        } else {
            newBatch.add(id);
            setExpandedSets(prev => new Set(prev).add(id));
        }
        setBatchEditingSets(newBatch);
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
        updateConfig({ textReplacementSets: [...sets, newSet] });
        setNewSetName('');
        setExpandedSets(prev => new Set(prev).add(newSet.id));
    };

    const handleUpdateSet = (id: string, updates: Partial<TextReplacementRuleSet>) => {
        updateConfig({
            textReplacementSets: sets.map(s => s.id === id ? { ...s, ...updates } : s)
        });
    };

    const handleDeleteSet = async (id: string) => {
        updateConfig({ textReplacementSets: sets.filter(s => s.id !== id) });
        await removeRuleSetReferenceFromProjects('enabledTextReplacementSetIds', id);
    };

    const handleAddRuleToSet = (setId: string) => {
        updateConfig({
            textReplacementSets: sets.map(s => {
                if (s.id !== setId) return s;
                return { ...s, rules: [...s.rules, { id: uuidv4(), from: '', to: '' }] };
            })
        });
    };

    const handleUpdateRuleInSet = (setId: string, ruleId: string, updates: Partial<TextReplacementRule>) => {
        updateConfig({
            textReplacementSets: sets.map(s => {
                if (s.id !== setId) return s;
                return { ...s, rules: s.rules.map(r => r.id === ruleId ? { ...r, ...updates } : r) };
            })
        });
    };

    const handleDeleteRuleFromSet = (setId: string, ruleId: string) => {
        updateConfig({
            textReplacementSets: sets.map(s => {
                if (s.id !== setId) return s;
                return { ...s, rules: s.rules.filter(r => r.id !== ruleId) };
            })
        });
    };

    const rulesToString = (rules: TextReplacementRule[]): string => {
        return rules.map(r => `${r.from} => ${r.to}`).join('\n');
    };

    const stringToRules = (str: string): TextReplacementRule[] => {
        return str.split('\n')
            .filter(line => line.trim() !== '')
            .map((line) => {
                const pairs = ['=>', '->', '=', ':'] as const;
                for (const separator of pairs) {
                    if (line.includes(separator)) {
                        const [fromPart = '', toPart = ''] = line.split(separator);
                        return { id: uuidv4(), from: fromPart.trim(), to: toPart.trim() };
                    }
                }

                return { id: uuidv4(), from: line.trim(), to: '' };
            });
    };

    // --- Hotword Handlers ---

    const toggleHotwordSetExpanded = (id: string) => {
        const newExpanded = new Set(expandedHotwordSets);
        if (newExpanded.has(id)) newExpanded.delete(id);
        else newExpanded.add(id);
        setExpandedHotwordSets(newExpanded);
    };

    const toggleHotwordBatchMode = (id: string) => {
        const newBatch = new Set(batchEditingHotwordSets);
        if (newBatch.has(id)) newBatch.delete(id);
        else {
            newBatch.add(id);
            setExpandedHotwordSets(prev => new Set(prev).add(id));
        }
        setBatchEditingHotwordSets(newBatch);
    };

    const handleAddHotwordSet = () => {
        if (!newHotwordSetName.trim()) return;
        const newSet: HotwordRuleSet = {
            id: uuidv4(),
            name: newHotwordSetName.trim(),
            enabled: true,
            rules: []
        };
        updateConfig({ hotwordSets: [...hotwordSets, newSet] });
        setNewHotwordSetName('');
        setExpandedHotwordSets(prev => new Set(prev).add(newSet.id));
    };

    const handleUpdateHotwordSet = (id: string, updates: Partial<HotwordRuleSet>) => {
        updateConfig({
            hotwordSets: hotwordSets.map(s => s.id === id ? { ...s, ...updates } : s)
        });
    };

    const handleDeleteHotwordSet = async (id: string) => {
        updateConfig({ hotwordSets: hotwordSets.filter(s => s.id !== id) });
        await removeRuleSetReferenceFromProjects('enabledHotwordSetIds', id);
    };

    const handleAddHotwordToSet = (setId: string) => {
        updateConfig({
            hotwordSets: hotwordSets.map(s => {
                if (s.id !== setId) return s;
                return { ...s, rules: [...s.rules, { id: uuidv4(), text: '' }] };
            })
        });
    };

    const handleUpdateHotwordInSet = (setId: string, ruleId: string, updates: Partial<HotwordRule>) => {
        updateConfig({
            hotwordSets: hotwordSets.map(s => {
                if (s.id !== setId) return s;
                return { ...s, rules: s.rules.map(r => r.id === ruleId ? { ...r, ...updates } : r) };
            })
        });
    };

    const handleDeleteHotwordFromSet = (setId: string, ruleId: string) => {
        updateConfig({
            hotwordSets: hotwordSets.map(s => {
                if (s.id !== setId) return s;
                return { ...s, rules: s.rules.filter(r => r.id !== ruleId) };
            })
        });
    };

    const hotwordsToString = (rules: HotwordRule[]): string => {
        return rules.map(r => r.text).join('\n');
    };

    const stringToHotwords = (str: string): HotwordRule[] => {
        return str.split('\n')
            .filter(line => line.trim() !== '')
            .map(line => ({ id: uuidv4(), text: line.trim() }));
    };

    // --- Polish Keyword Set Handlers ---

    const togglePolishKeywordSetExpanded = (id: string) => {
        const nextExpanded = new Set(expandedPolishKeywordSets);
        if (nextExpanded.has(id)) nextExpanded.delete(id);
        else nextExpanded.add(id);
        setExpandedPolishKeywordSets(nextExpanded);
    };

    const handleAddPolishKeywordSet = () => {
        if (!newPolishKeywordSetName.trim()) return;
        const newSet: PolishKeywordRuleSet = {
            id: uuidv4(),
            name: newPolishKeywordSetName.trim(),
            enabled: true,
            keywords: '',
        };
        updateConfig({ polishKeywordSets: [...polishKeywordSets, newSet] });
        setNewPolishKeywordSetName('');
        setExpandedPolishKeywordSets(prev => new Set(prev).add(newSet.id));
    };

    const handleUpdatePolishKeywordSet = (id: string, updates: Partial<PolishKeywordRuleSet>) => {
        updateConfig({
            polishKeywordSets: polishKeywordSets.map((set) => (set.id === id ? { ...set, ...updates } : set)),
        });
    };

    const handleDeletePolishKeywordSet = async (id: string) => {
        updateConfig({ polishKeywordSets: polishKeywordSets.filter((set) => set.id !== id) });
        await removeRuleSetReferenceFromProjects('enabledPolishKeywordSetIds', id);
    };

    return (
        <SettingsTabContainer id="settings-panel-vocabulary" ariaLabelledby="settings-tab-vocabulary">
            <SettingsPageHeader 
                icon={<BookIcon width={28} height={28} />}
                title={t('settings.vocabulary')} 
                description={t('settings.vocabulary_description', { defaultValue: 'Manage custom vocabulary, hotwords, polish keyword sets, text polish context presets, and summary templates.' })} 
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
                <div className="settings-list" style={{ background: 'var(--color-bg-primary)', overflow: 'hidden' }}>
                    {sets.length === 0 ? (
                        <div style={{ 
                            padding: '48px 24px', 
                            textAlign: 'center', 
                            color: 'var(--color-text-muted)'
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
                                        <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', background: 'var(--color-bg-secondary)', padding: '2px 8px', borderRadius: 'var(--radius-sm)' }}>
                                            {set.rules.length} {t('settings.rules_count', { count: set.rules.length, defaultValue: 'rules' })}
                                        </span>
                                    </div>

                                    <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }} onClick={(e) => e.stopPropagation()}>
                                        <button
                                            className="btn btn-icon btn-secondary-soft"
                                            onClick={() => toggleBatchMode(set.id)}
                                            title={batchEditingSets.has(set.id) ? t('settings.switch_to_list', { defaultValue: 'Switch to List' }) : t('settings.switch_to_text', { defaultValue: 'Switch to Text' })}
                                            aria-label={batchEditingSets.has(set.id) ? t('settings.switch_to_list', { defaultValue: 'Switch to List' }) : t('settings.switch_to_text', { defaultValue: 'Switch to Text' })}
                                        >
                                            {batchEditingSets.has(set.id) ? <List size={18} /> : <FileText size={18} />}
                                        </button>

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
                                            title={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                                            aria-label={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
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
                                        {batchEditingSets.has(set.id) ? (
                                            <>
                                                <textarea
                                                    className="settings-input"
                                                    value={rulesToString(set.rules)}
                                                    onChange={(e) => handleUpdateSet(set.id, { rules: stringToRules(e.target.value) })}
                                                    placeholder={t('settings.rules_placeholder', { defaultValue: 'e.g. Find => Replace With' })}
                                                    rows={5}
                                                    style={{ 
                                                        width: '100%', 
                                                        minHeight: '120px',
                                                        fontFamily: 'var(--font-mono)',
                                                        fontSize: '0.85rem',
                                                        resize: 'vertical',
                                                        lineHeight: '1.4',
                                                        padding: '10px'
                                                    }}
                                                />
                                                <p style={{ 
                                                    fontSize: '0.75rem', 
                                                    color: 'var(--color-text-muted)',
                                                    margin: 0,
                                                    marginTop: '4px'
                                                }}>
                                                    {t('settings.rules_hint', { defaultValue: 'Use " => " to separate find and replace text. One rule per line.' })}
                                                </p>
                                            </>
                                        ) : (
                                            <>
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
                                                            <div style={{ color: 'var(--color-text-muted)', fontSize: '0.8rem', opacity: 0.6 }}>{'=>'}</div>
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
                                                            aria-label={t('common.delete')}
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
                                            </>
                                        )}
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </SettingsSection>

            <SettingsSection
                title={t('settings.hotwords_title', { defaultValue: 'Hotwords' })}
                icon={<BookIcon width={20} height={20} />}
                description={t('settings.hotwords_description', { defaultValue: 'Enhance recognition for specific terms. One per line. (Supported by Transducer and Qwen3 models)' })}
            >
                {/* Add New Hotword Set */}
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
                            value={newHotwordSetName}
                            onChange={(e) => setNewHotwordSetName(e.target.value)}
                            placeholder={t('settings.rule_set_name_placeholder', { defaultValue: 'e.g. Technical Terms' })}
                            style={{ width: '100%' }}
                        />
                    </div>
                    <button 
                        className="btn btn-primary" 
                        onClick={handleAddHotwordSet}
                        disabled={!newHotwordSetName.trim()}
                        style={{ height: '38px', display: 'flex', alignItems: 'center', gap: '6px', padding: '0 20px' }}
                    >
                        <Plus size={18} />
                        {t('settings.add_rule_set', { defaultValue: 'Add Set' })}
                    </button>
                </div>

                {/* Hotword Sets List */}
                <div className="settings-list" style={{ background: 'var(--color-bg-primary)', overflow: 'hidden' }}>
                    {hotwordSets.length === 0 ? (
                        <div style={{ 
                            padding: '48px 24px', 
                            textAlign: 'center', 
                            color: 'var(--color-text-muted)'
                        }}>
                            {t('settings.no_rule_sets', { defaultValue: 'No rule sets defined.' })}
                        </div>
                    ) : (
                        hotwordSets.map((set, index) => (
                            <div key={set.id} style={{ 
                                borderBottom: index === hotwordSets.length - 1 ? 'none' : '1px solid var(--color-border-subtle)',
                                background: set.enabled ? 'transparent' : 'var(--color-bg-secondary-soft)',
                            }}>
                                {/* Set Header */}
                                <div style={{ 
                                    display: 'flex', 
                                    alignItems: 'center', 
                                    gap: '12px',
                                    padding: '16px 24px',
                                    cursor: 'pointer'
                                }} onClick={() => toggleHotwordSetExpanded(set.id)}>
                                    <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)' }}>
                                        {expandedHotwordSets.has(set.id) ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                                    </div>
                                    
                                    <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '12px' }}>
                                        <input
                                            type="text"
                                            className="settings-input-minimal"
                                            value={set.name}
                                            onClick={(e) => e.stopPropagation()}
                                            onChange={(e) => handleUpdateHotwordSet(set.id, { name: e.target.value })}
                                            style={{ fontWeight: 600, fontSize: '1rem', width: 'auto', minWidth: '150px' }}
                                        />
                                        <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', background: 'var(--color-bg-secondary)', padding: '2px 8px', borderRadius: 'var(--radius-sm)' }}>
                                            {set.rules.length} {t('settings.rules_count', { count: set.rules.length, defaultValue: 'rules' })}
                                        </span>
                                    </div>

                                    <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }} onClick={(e) => e.stopPropagation()}>
                                        <button
                                            className="btn btn-icon btn-secondary-soft"
                                            onClick={() => toggleHotwordBatchMode(set.id)}
                                            title={batchEditingHotwordSets.has(set.id) ? t('settings.switch_to_list', { defaultValue: 'Switch to List' }) : t('settings.switch_to_text', { defaultValue: 'Switch to Text' })}
                                            aria-label={batchEditingHotwordSets.has(set.id) ? t('settings.switch_to_list', { defaultValue: 'Switch to List' }) : t('settings.switch_to_text', { defaultValue: 'Switch to Text' })}
                                        >
                                            {batchEditingHotwordSets.has(set.id) ? <List size={18} /> : <FileText size={18} />}
                                        </button>
                                        
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                            <Switch 
                                                checked={set.enabled} 
                                                onChange={(checked) => handleUpdateHotwordSet(set.id, { enabled: checked })} 
                                            />
                                        </div>

                                        <button 
                                            className="btn btn-icon btn-danger-soft"
                                            onClick={() => handleDeleteHotwordSet(set.id)}
                                            title={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                                            aria-label={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                                        >
                                            <Trash2 size={18} />
                                        </button>
                                    </div>
                                </div>

                                {/* Rules under this set */}
                                {expandedHotwordSets.has(set.id) && (
                                    <div style={{ 
                                        padding: '0 24px 24px 56px',
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '8px'
                                    }}>
                                        {batchEditingHotwordSets.has(set.id) ? (
                                            <>
                                                <textarea
                                                    className="settings-input"
                                                    value={hotwordsToString(set.rules)}
                                                    onChange={(e) => handleUpdateHotwordSet(set.id, { rules: stringToHotwords(e.target.value) })}
                                                    placeholder={t('settings.hotwords_placeholder', { defaultValue: 'e.g. ChatGPT\nSherpa-onnx :2.0' })}
                                                    rows={5}
                                                    style={{ 
                                                        width: '100%', 
                                                        minHeight: '120px',
                                                        fontFamily: 'var(--font-mono)',
                                                        fontSize: '0.85rem',
                                                        resize: 'vertical',
                                                        lineHeight: '1.4',
                                                        padding: '10px'
                                                    }}
                                                />
                                                <p style={{ 
                                                    fontSize: '0.75rem', 
                                                    color: 'var(--color-text-muted)',
                                                    margin: 0,
                                                    marginTop: '4px'
                                                }}>
                                                    {t('settings.hotwords_hint', { defaultValue: 'Tip: You can add weight by appending " :weight" (e.g. "Term :2.0"). Default weight is 1.0.' })}
                                                </p>
                                            </>
                                        ) : (
                                            <>
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
                                                                value={rule.text}
                                                                onChange={(e) => handleUpdateHotwordInSet(set.id, rule.id, { text: e.target.value })}
                                                                placeholder={t('settings.hotwords_placeholder', { defaultValue: 'e.g. ChatGPT' })}
                                                                style={{ fontWeight: 500 }}
                                                            />
                                                        </div>
                                                        <button 
                                                            className="btn btn-icon btn-danger-soft"
                                                            onClick={() => handleDeleteHotwordFromSet(set.id, rule.id)}
                                                            style={{ padding: '4px' }}
                                                            aria-label={t('common.delete')}
                                                        >
                                                            <Trash2 size={14} />
                                                        </button>
                                                    </div>
                                                ))}
                                                
                                                <button 
                                                    className="btn btn-secondary-soft" 
                                                    onClick={() => handleAddHotwordToSet(set.id)}
                                                    style={{ alignSelf: 'flex-start', marginTop: '4px', fontSize: '0.85rem', display: 'flex', alignItems: 'center', gap: '6px', padding: '6px 16px' }}
                                                >
                                                    <Plus size={16} />
                                                    {t('common.add')}
                                                </button>
                                                <p style={{ 
                                                    fontSize: '0.75rem', 
                                                    color: 'var(--color-text-muted)',
                                                    margin: 0,
                                                    marginTop: '4px'
                                                }}>
                                                    {t('settings.hotwords_hint', { defaultValue: 'Tip: You can add weight by appending " :weight" (e.g. "Term :2.0"). Default weight is 1.0.' })}
                                                </p>
                                            </>
                                        )}
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </SettingsSection>

            <SettingsSection
                title={t('settings.polish_keywords_title', { defaultValue: 'Polish Keywords' })}
                icon={<BookIcon width={20} height={20} />}
                description={t('settings.polish_keywords_description', {
                    defaultValue: 'Group reusable keyword guidance into global sets. Enabled sets are combined when text polish runs.',
                })}
            >
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
                            value={newPolishKeywordSetName}
                            onChange={(e) => setNewPolishKeywordSetName(e.target.value)}
                            placeholder={t('settings.polish_keyword_set_name_placeholder', { defaultValue: 'e.g. Brand Terms' })}
                            style={{ width: '100%' }}
                        />
                    </div>
                    <button
                        className="btn btn-primary"
                        onClick={handleAddPolishKeywordSet}
                        disabled={!newPolishKeywordSetName.trim()}
                        style={{ height: '38px', display: 'flex', alignItems: 'center', gap: '6px', padding: '0 20px' }}
                    >
                        <Plus size={18} />
                        {t('settings.add_rule_set', { defaultValue: 'Add Set' })}
                    </button>
                </div>

                <div className="settings-list" style={{ background: 'var(--color-bg-primary)', overflow: 'hidden' }}>
                    {polishKeywordSets.length === 0 ? (
                        <div style={{
                            padding: '48px 24px',
                            textAlign: 'center',
                            color: 'var(--color-text-muted)'
                        }}>
                            {t('settings.no_polish_keyword_sets', { defaultValue: 'No polish keyword sets yet.' })}
                        </div>
                    ) : (
                        polishKeywordSets.map((set, index) => (
                            <div key={set.id} style={{
                                borderBottom: index === polishKeywordSets.length - 1 ? 'none' : '1px solid var(--color-border-subtle)',
                                background: set.enabled ? 'transparent' : 'var(--color-bg-secondary-soft)',
                            }}>
                                <div style={{
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '12px',
                                    padding: '16px 24px',
                                    cursor: 'pointer'
                                }} onClick={() => togglePolishKeywordSetExpanded(set.id)}>
                                    <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)' }}>
                                        {expandedPolishKeywordSets.has(set.id) ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                                    </div>

                                    <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '12px' }}>
                                        <input
                                            type="text"
                                            className="settings-input-minimal"
                                            value={set.name}
                                            onClick={(e) => e.stopPropagation()}
                                            onChange={(e) => handleUpdatePolishKeywordSet(set.id, { name: e.target.value })}
                                            style={{ fontWeight: 600, fontSize: '1rem', width: 'auto', minWidth: '150px' }}
                                        />
                                        <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', background: 'var(--color-bg-secondary)', padding: '2px 8px', borderRadius: 'var(--radius-sm)' }}>
                                            {set.keywords.trim()
                                                ? t('settings.polish_keywords_ready', { defaultValue: 'Ready' })
                                                : t('settings.polish_keywords_empty', { defaultValue: 'Empty' })}
                                        </span>
                                    </div>

                                    <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }} onClick={(e) => e.stopPropagation()}>
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                            <Switch
                                                checked={set.enabled}
                                                onChange={(checked) => handleUpdatePolishKeywordSet(set.id, { enabled: checked })}
                                            />
                                        </div>

                                        <button
                                            className="btn btn-icon btn-danger-soft"
                                            onClick={() => handleDeletePolishKeywordSet(set.id)}
                                            title={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                                            aria-label={t('settings.delete_rule_set', { defaultValue: `Delete ${set.name}` })}
                                        >
                                            <Trash2 size={18} />
                                        </button>
                                    </div>
                                </div>

                                {expandedPolishKeywordSets.has(set.id) && (
                                    <div style={{
                                        padding: '0 24px 24px 56px',
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '8px'
                                    }}>
                                        <textarea
                                            className="settings-input"
                                            value={set.keywords}
                                            onChange={(e) => handleUpdatePolishKeywordSet(set.id, { keywords: e.target.value })}
                                            placeholder={t('settings.polish_keywords_placeholder', {
                                                defaultValue: 'e.g. Product names, terminology, preferred spellings',
                                            })}
                                            rows={5}
                                            style={{
                                                width: '100%',
                                                minHeight: '120px',
                                                fontFamily: 'var(--font-mono)',
                                                fontSize: '0.85rem',
                                                resize: 'vertical',
                                                lineHeight: '1.4',
                                                padding: '10px'
                                            }}
                                        />
                                        <p style={{
                                            fontSize: '0.75rem',
                                            color: 'var(--color-text-muted)',
                                            margin: 0,
                                            marginTop: '4px'
                                        }}>
                                            {t('settings.polish_keywords_hint', {
                                                defaultValue: 'Use this block for preferred terms or style guidance. Enabled sets are combined in order during polishing.',
                                            })}
                                        </p>
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </SettingsSection>

            <SettingsSpeakerProfilesSection />
            <SettingsContextSection />
            <SettingsSummaryTemplateSection />
        </SettingsTabContainer>
    );
}
