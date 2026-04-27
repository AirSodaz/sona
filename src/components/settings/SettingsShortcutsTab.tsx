import React from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Mic, Search, FilePenLine } from 'lucide-react';
import { KeyboardIcon } from '../Icons';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { useShortcutConfig, useSetConfig } from '../../stores/configStore';
import { SettingsShortcutInput } from './SettingsShortcutInput';

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
                { key: 'Ctrl + F', description: t('shortcuts.find_current_context', { defaultValue: 'Find in current context' }) },
                { key: 'Enter / Shift + Enter', description: t('shortcuts.next_prev_transcript_match', { defaultValue: 'Next / previous transcript match' }) },
                { key: '↑ / ↓', description: t('shortcuts.navigate_workspace_results', { defaultValue: 'Navigate workspace search results' }) },
                { key: 'Esc', description: t('shortcuts.clear_or_close_search', { defaultValue: 'Clear or close current search' }) },
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
                                <SettingsShortcutInput
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
