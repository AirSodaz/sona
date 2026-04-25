import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Mic, Search, FilePenLine, Type } from 'lucide-react';
import { KeyboardIcon } from '../Icons';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { useShortcutConfig, useVoiceTypingConfig, useSetConfig } from '../../stores/configStore';
import { Switch } from '../Switch';
import { Dropdown } from '../Dropdown';

interface ShortcutInputProps {
    value: string;
    onChange: (newValue: string) => void;
}

function ShortcutInput({ value, onChange }: ShortcutInputProps) {
    const [isRecording, setIsRecording] = useState(false);
    const [isEditing, setIsEditing] = useState(false);
    const [tempValue, setTempValue] = useState(value);
    const { t } = useTranslation();

    useEffect(() => {
        if (!isEditing) {
            setTempValue(value);
        }
    }, [value, isEditing]);

    const finalizeShortcut = (e: React.KeyboardEvent) => {
        const keys: string[] = [];
        
        if (e.ctrlKey || e.key === 'Control') keys.push('Ctrl');
        if (e.altKey || e.key === 'Alt') keys.push('Alt');
        if (e.shiftKey || e.key === 'Shift') keys.push('Shift');
        if (e.metaKey || e.key === 'Meta') keys.push('Meta');

        const isModifier = ['Control', 'Alt', 'Shift', 'Meta'].includes(e.key);
        
        let mainKey = '';
        if (!isModifier) {
            if (e.code === 'Space' || e.key === ' ') {
                mainKey = 'Space';
            } else if (e.code && e.code.startsWith('Key')) {
                mainKey = e.code.replace('Key', '');
            } else if (e.code && e.code.startsWith('Digit')) {
                mainKey = e.code.replace('Digit', '');
            } else if (e.key && e.key.length === 1) {
                mainKey = e.key.toUpperCase();
            } else if (e.key && e.key !== 'Unidentified' && e.key !== 'Process' && e.key !== 'Dead') {
                mainKey = e.key;
            }
        }

        if (mainKey) {
            keys.push(mainKey);
        }

        const uniqueKeys = Array.from(new Set(keys));
        if (uniqueKeys.length > 0) {
            onChange(uniqueKeys.join(' + '));
            setIsRecording(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!isRecording) return;
        e.preventDefault();
        e.stopPropagation();

        if (e.key === 'Escape') {
            setIsRecording(false);
            return;
        }

        const isModifier = ['Control', 'Alt', 'Shift', 'Meta'].includes(e.key);
        if (!isModifier) {
            finalizeShortcut(e);
        }
    };

    const handleKeyUp = (e: React.KeyboardEvent) => {
        if (!isRecording) return;
        e.preventDefault();
        e.stopPropagation();

        finalizeShortcut(e);
    };

    if (isEditing) {
        return (
            <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                <input
                    type="text"
                    value={tempValue}
                    onChange={(e) => setTempValue(e.target.value)}
                    onBlur={() => {
                        onChange(tempValue);
                        setIsEditing(false);
                    }}
                    onKeyDown={(e) => {
                        switch (e.key) {
                            case 'Enter':
                                onChange(tempValue);
                                setIsEditing(false);
                                break;
                            case 'Escape':
                                setTempValue(value);
                                setIsEditing(false);
                                break;
                        }
                    }}
                    autoFocus
                    style={{ 
                        width: '120px', 
                        padding: '4px 8px', 
                        textAlign: 'center',
                        background: 'var(--color-bg-secondary)',
                        color: 'var(--color-text-primary)',
                        border: '1px solid var(--color-border)',
                        borderRadius: '6px'
                    }}
                />
            </div>
        );
    }

    return (
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <button
                className={`btn ${isRecording ? 'btn-primary' : 'btn-secondary'}`}
                style={{ minWidth: '120px', display: 'flex', justifyContent: 'center' }}
                onClick={() => setIsRecording(true)}
                onKeyDown={handleKeyDown}
                onKeyUp={handleKeyUp}
                onBlur={() => setIsRecording(false)}
            >
                {isRecording ? t('common.recording', { defaultValue: 'Recording...' }) : value}
            </button>
            <button 
                onClick={() => setIsEditing(true)}
                data-tooltip={t('shortcuts.edit_manually', { defaultValue: 'Edit manually' })}
                data-tooltip-pos="top"
                style={{ 
                    padding: '4px', 
                    background: 'transparent', 
                    border: 'none', 
                    color: 'var(--color-text-muted)', 
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center'
                }}
            >
                <FilePenLine size={16} />
            </button>
        </div>
    );
}

interface ShortcutItem {
    id?: string;
    key: string;
    description: string;
    editable?: boolean;
}

interface ShortcutSection {
    title: string;
    icon?: React.JSX.Element;
    items: ShortcutItem[];
}

export function SettingsShortcutsTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useShortcutConfig();
    const updateConfig = useSetConfig();

    const sections: ShortcutSection[] = [
        {
            title: t('shortcuts.section_live'),
            icon: <Mic size={20} />,
            items: [
                { id: 'liveRecordShortcut', key: config.liveRecordShortcut || 'Ctrl + Space', description: t('shortcuts.record_start_stop'), editable: true },
                { key: 'Space', description: t('shortcuts.record_pause_resume') },
            ]
        },
        {
            title: t('shortcuts.section_playback'),
            icon: <Play size={20} />,
            items: [
                { key: 'Space / K', description: t('shortcuts.play_pause') },
                { key: '← / →', description: `${t('shortcuts.seek_backward')} / ${t('shortcuts.seek_forward')}` },
                { key: '↑ / ↓', description: `${t('shortcuts.volume_up')} / ${t('shortcuts.volume_down')}` },
                { key: 'M', description: t('shortcuts.toggle_mute') },
            ]
        },
        {
            title: t('shortcuts.section_search'),
            icon: <Search size={20} />,
            items: [
                { key: 'Ctrl + F', description: t('shortcuts.find') },
                { key: 'Enter', description: t('shortcuts.next_match') },
                { key: 'Shift + Enter', description: t('shortcuts.prev_match') },
                { key: 'Esc', description: t('shortcuts.close') },
            ]
        },
        {
            title: t('shortcuts.section_editor'),
            icon: <FilePenLine size={20} />,
            items: [
                { key: 'Ctrl + B', description: t('shortcuts.editor_bold') },
                { key: 'Ctrl + I', description: t('shortcuts.editor_italic') },
                { key: 'Ctrl + U', description: t('shortcuts.editor_underline') },
                { key: 'Shift + Enter', description: t('shortcuts.editor_line_break') },
            ]
        }
    ];

    const vtConfig = useVoiceTypingConfig();

    return (
        <SettingsTabContainer id="settings-panel-shortcuts" ariaLabelledby="settings-tab-shortcuts">
            <SettingsPageHeader 
                icon={<KeyboardIcon width={28} height={28} />}
                title={t('shortcuts.title')} 
                description={t('settings.shortcuts_description')} 
            />

            <SettingsSection title={t('settings.voice_typing', 'Voice Typing')} icon={<Type size={20} />}>
                <SettingsItem
                    title={t('settings.enable_voice_typing', 'Enable Voice Typing')}
                    hint={t('settings.enable_voice_typing_hint', 'Type text directly into any application using your voice')}
                >
                    <Switch
                        checked={vtConfig.voiceTypingEnabled ?? false}
                        onChange={(val) => updateConfig({ voiceTypingEnabled: val })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.voice_typing_shortcut', 'Shortcut')}
                    hint={t('settings.voice_typing_shortcut_hint', 'Global shortcut to activate voice typing')}
                >
                    <ShortcutInput
                        value={vtConfig.voiceTypingShortcut || 'Alt+V'}
                        onChange={(val) => updateConfig({ voiceTypingShortcut: val })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.voice_typing_mode', 'Mode')}
                    hint={t('settings.voice_typing_mode_hint', 'How the shortcut triggers listening')}
                >
                    <div style={{ width: '180px' }}>
                        <Dropdown
                            id="vt-mode-select"
                            value={vtConfig.voiceTypingMode || 'hold'}
                            onChange={(val) => updateConfig({ voiceTypingMode: val as any })}
                            options={[
                                { value: 'hold', label: t('settings.voice_typing_mode_hold', 'Push to Talk (Hold)') },
                                { value: 'toggle', label: t('settings.voice_typing_mode_toggle', 'Toggle (Press once)') }
                            ]}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            {sections.map((section, index) => (
                <SettingsSection key={index} title={section.title} icon={section.icon}>
                    {section.items.map((item, i) => (
                        <SettingsItem key={i} title={item.description}>
                            {item.editable && item.id === 'liveRecordShortcut' ? (
                                <ShortcutInput 
                                    value={config.liveRecordShortcut || 'Ctrl + Space'}
                                    onChange={(val) => updateConfig({ liveRecordShortcut: val })}
                                />
                            ) : (
                                <div className="shortcut-keys" style={{ display: 'flex', gap: '4px', alignItems: 'center', flexWrap: 'wrap' }}>
                                    {item.key.split(' / ').map((k, kIndex, arr) => (
                                        <React.Fragment key={kIndex}>
                                            <kbd className="kbd">{k}</kbd>
                                            {kIndex < arr.length - 1 && <span className="text-muted mx-1">/</span>}
                                        </React.Fragment>
                                    ))}
                                </div>
                            )}
                        </SettingsItem>
                    ))}
                </SettingsSection>
            ))}

            <style>{`
                .kbd {
                    background: var(--color-bg-secondary);
                    border: 1px solid var(--color-border);
                    border-radius: 6px;
                    padding: 4px 8px;
                    font-family: var(--font-mono, monospace);
                    font-size: 0.85rem;
                    color: var(--color-text-primary);
                    box-shadow: 0 1px 2px rgba(0,0,0,0.05), inset 0 -1px 0 var(--color-border);
                    min-width: 24px;
                    text-align: center;
                    font-weight: 500;
                }
                .text-muted { color: var(--color-text-muted); }
                .mx-1 { margin-left: 0.25rem; margin-right: 0.25rem; }
            `}</style>
        </SettingsTabContainer>
    );
}
