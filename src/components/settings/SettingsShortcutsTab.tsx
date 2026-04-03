import React from 'react';
import { useTranslation } from 'react-i18next';
import { Keyboard } from 'lucide-react';
import { SettingsTabContainer, SettingsSection } from './SettingsLayout';

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
            <SettingsSection
                title={t('settings.shortcuts_title', { defaultValue: 'Keyboard Shortcuts' })}
                icon={<Keyboard size={20} />}
                description={t('settings.shortcuts_description', { defaultValue: 'View and customize application hotkeys.' })}
            >
                {sections.map((section, index) => (
                    <div key={index} className="shortcut-group-wrapper" style={{ padding: '0 24px' }}>
                        <h4 className="shortcut-group-title">
                            {section.title}
                        </h4>
                        <div className="shortcut-list">
                            {section.items.map((item, i) => (
                                <div key={i} className="shortcut-item">
                                    <span className="shortcut-desc">{item.description}</span>
                                    <div className="shortcut-keys">
                                        {item.key.split(' / ').map((k, kIndex, arr) => (
                                            <React.Fragment key={kIndex}>
                                                <kbd className="kbd">{k}</kbd>
                                                {kIndex < arr.length - 1 && <span className="mx-1 text-muted">/</span>}
                                            </React.Fragment>
                                        ))}
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>
                ))}
            </SettingsSection>

            <style>{`
                .shortcut-group-wrapper:first-child {
                    padding-top: 24px !important;
                }
                .shortcut-group-wrapper:last-child {
                    padding-bottom: 24px !important;
                }
                .shortcut-group-wrapper + .shortcut-group-wrapper {
                    margin-top: 24px;
                }
                .shortcut-group-title {
                    font-size: 0.8rem;
                    font-weight: 600;
                    color: var(--color-text-muted);
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                    margin-bottom: 12px;
                }
                .shortcut-list {
                    display: flex;
                    flex-direction: column;
                    gap: 12px;
                }

                .shortcut-item {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 4px 0;
                }

                .shortcut-keys {
                    display: flex;
                    align-items: center;
                    flex-wrap: wrap;
                    gap: 4px;
                }

                .kbd {
                    background: var(--color-bg-secondary);
                    border: 1px solid var(--color-border);
                    border-radius: 4px;
                    padding: 2px 6px;
                    font-family: monospace;
                    font-size: 0.85rem;
                    color: var(--color-text-primary);
                    box-shadow: 0 1px 0 var(--color-border);
                    min-width: 20px;
                    text-align: center;
                }

                .shortcut-desc {
                    color: var(--color-text-primary);
                    font-size: 0.9rem;
                }

                .text-muted { color: var(--color-text-muted); }
                .mx-1 { margin-left: 0.25rem; margin-right: 0.25rem; }
            `}</style>
        </SettingsTabContainer>
    );
}
