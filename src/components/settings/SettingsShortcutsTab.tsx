import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Mic, Search, FilePenLine } from 'lucide-react';
import { KeyboardIcon } from '../Icons';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { AppConfig } from '../../types/config';

interface ShortcutInputProps {
    value: string;
    onChange: (newValue: string) => void;
}

function ShortcutInput({ value, onChange }: ShortcutInputProps) {
    const [isRecording, setIsRecording] = useState(false);
    const { t } = useTranslation();

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!isRecording) return;
        e.preventDefault();
        e.stopPropagation();

        if (e.key === 'Escape') {
            setIsRecording(false);
            return;
        }

        const keys: string[] = [];
        if (e.ctrlKey) keys.push('Ctrl');
        if (e.altKey) keys.push('Alt');
        if (e.shiftKey) keys.push('Shift');
        if (e.metaKey) keys.push('Meta');

        // Filter out isolated modifier key presses
        if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) {
            return;
        }

        let mainKey = e.key;
        if (mainKey === ' ') mainKey = 'Space';
        else if (mainKey.length === 1) mainKey = mainKey.toUpperCase();

        keys.push(mainKey);
        onChange(keys.join(' + '));
        setIsRecording(false);
    };

    return (
        <button
            className={`btn ${isRecording ? 'btn-primary' : 'btn-secondary'}`}
            style={{ minWidth: '120px', display: 'flex', justifyContent: 'center' }}
            onClick={() => setIsRecording(true)}
            onKeyDown={handleKeyDown}
            onBlur={() => setIsRecording(false)}
        >
            {isRecording ? t('common.recording', { defaultValue: 'Recording...' }) : value}
        </button>
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

interface SettingsShortcutsTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
}

export function SettingsShortcutsTab({ config, updateConfig }: SettingsShortcutsTabProps): React.JSX.Element {
    const { t } = useTranslation();

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

    return (
        <SettingsTabContainer id="settings-panel-shortcuts" ariaLabelledby="settings-tab-shortcuts">
            <SettingsPageHeader 
                icon={<KeyboardIcon width={28} height={28} />}
                title={t('shortcuts.title')} 
                description={t('settings.shortcuts_description')} 
            />

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
