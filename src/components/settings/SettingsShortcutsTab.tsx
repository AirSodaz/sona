import React from 'react';
import { useTranslation } from 'react-i18next';

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
        }
    ];

    return (
        <div className="settings-tab-content">
            <div className="shortcuts-grid">
                {sections.map((section, index) => (
                    <div key={index} className="shortcut-section mb-6">
                        <h4 className="text-sm font-semibold text-muted mb-3 uppercase tracking-wider">
                            {section.title}
                        </h4>
                        <div className="shortcut-list">
                            {section.items.map((item, i) => (
                                <div key={i} className="shortcut-item">
                                    <div className="shortcut-keys">
                                        {item.key.split(' / ').map((k, kIndex, arr) => (
                                            <React.Fragment key={kIndex}>
                                                <kbd className="kbd">{k}</kbd>
                                                {kIndex < arr.length - 1 && <span className="mx-1 text-muted">/</span>}
                                            </React.Fragment>
                                        ))}
                                    </div>
                                    <span className="shortcut-desc">{item.description}</span>
                                </div>
                            ))}
                        </div>
                    </div>
                ))}
            </div>

            <style>{`
                .shortcuts-grid {
                    display: grid;
                    gap: 24px;
                }
                
                .shortcut-section {
                    background: var(--color-bg-elevated);
                    border: 1px solid var(--color-border);
                    border-radius: var(--radius-lg);
                    padding: 16px;
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
                    color: var(--color-text-secondary);
                    font-size: 0.9rem;
                }

                .uppercase { text-transform: uppercase; }
                .tracking-wider { letter-spacing: 0.05em; }
                .font-semibold { font-weight: 600; }
                .text-sm { font-size: 0.875rem; }
                .text-muted { color: var(--color-text-muted); }
                .mb-3 { margin-bottom: 0.75rem; }
                .mb-4 { margin-bottom: 1rem; }
                .mb-6 { margin-bottom: 1.5rem; }
                .mx-1 { margin-left: 0.25rem; margin-right: 0.25rem; }
            `}</style>
        </div>
    );
}
