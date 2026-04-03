import React from 'react';
import { useTranslation } from 'react-i18next';
import { Keyboard } from 'lucide-react';
import { SettingsTabContainer, SettingsSection, SettingsItem } from './SettingsLayout';

interface ShortcutItem {
    key: string;
    description: string;
}

interface ShortcutSection {
    title: string;
    items: ShortcutItem[];
}

export function SettingsShortcutsTab(): React.JSX.Element {
    const { t } = useTranslation();

    const sections: ShortcutSection[] = [
        {
            title: t('shortcuts.section_playback'),
            items: [
                { key: 'Space / K', description: t('shortcuts.play_pause') },
                { key: '← / →', description: `${t('shortcuts.seek_backward')} / ${t('shortcuts.seek_forward')}` },
                { key: '↑ / ↓', description: `${t('shortcuts.volume_up')} / ${t('shortcuts.volume_down')}` },
                { key: 'M', description: t('shortcuts.toggle_mute') },
            ]
        },
        {
            title: t('shortcuts.section_live'),
            items: [
                { key: 'Ctrl + Space', description: t('shortcuts.record_start_stop') },
                { key: 'Space', description: t('shortcuts.record_pause_resume') },
            ]
        },
        {
            title: t('shortcuts.section_search'),
            items: [
                { key: 'Ctrl + F', description: t('shortcuts.find') },
                { key: 'Enter', description: t('shortcuts.next_match') },
                { key: 'Shift + Enter', description: t('shortcuts.prev_match') },
                { key: 'Esc', description: t('shortcuts.close') },
            ]
        },
        {
            title: t('shortcuts.section_editor'),
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
            <div className="settings-tab-header" style={{ display: 'flex', flexDirection: 'column', gap: '8px', padding: '0 8px 16px' }}>
                <h2 style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '1.25rem', fontWeight: 600, margin: 0, color: 'var(--color-text-primary)' }}>
                    <Keyboard size={24} /> {t('settings.shortcuts_title')}
                </h2>
                <p style={{ color: 'var(--color-text-muted)', margin: 0, fontSize: '0.9rem' }}>
                    {t('settings.shortcuts_description')}
                </p>
            </div>

            {sections.map((section, index) => (
                <SettingsSection key={index} title={section.title}>
                    {section.items.map((item, i) => (
                        <SettingsItem key={i} title={item.description}>
                            <div className="shortcut-keys" style={{ display: 'flex', gap: '4px', alignItems: 'center', flexWrap: 'wrap' }}>
                                {item.key.split(' / ').map((k, kIndex, arr) => (
                                    <React.Fragment key={kIndex}>
                                        <kbd className="kbd">{k}</kbd>
                                        {kIndex < arr.length - 1 && <span className="text-muted mx-1">/</span>}
                                    </React.Fragment>
                                ))}
                            </div>
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
