import React from 'react';
import { useTranslation } from 'react-i18next';
import { Subtitles, Monitor } from 'lucide-react';
import { Switch } from '../Switch';
import { AppConfig } from '../../types/transcript';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';

interface SettingsSubtitleTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
}

export function SettingsSubtitleTab({
    config,
    updateConfig
}: SettingsSubtitleTabProps): React.JSX.Element {
    const { t } = useTranslation();

    const lockWindow = config.lockWindow ?? false;
    const alwaysOnTop = config.alwaysOnTop ?? true;
    const startOnLaunch = config.startOnLaunch ?? false;
    const captionWindowWidth = config.captionWindowWidth ?? 800;
    const captionFontSize = config.captionFontSize ?? 24;
    const captionFontColor = config.captionFontColor || '#ffffff';

    return (
        <SettingsTabContainer id="settings-panel-subtitle" ariaLabelledby="settings-tab-subtitle">
            <SettingsPageHeader 
                icon={<Monitor size={28} />} 
                title={t('live.subtitle_settings')} 
                description={t('settings.subtitle_behavior_desc')} 
            />
            <SettingsSection
                title={t('settings.subtitle_behavior_title')}
            >
                <SettingsItem
                    title={t('live.start_on_launch')}
                    hint={t('live.start_on_launch_hint')}
                >
                    <Switch
                        checked={startOnLaunch}
                        onChange={(enabled) => updateConfig({ startOnLaunch: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('live.always_on_top')}
                    hint={t('live.always_on_top_hint')}
                >
                    <Switch
                        checked={alwaysOnTop}
                        onChange={(enabled) => updateConfig({ alwaysOnTop: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('live.lock_window')}
                    hint={t('live.lock_window_hint')}
                >
                    <Switch
                        checked={lockWindow}
                        onChange={(enabled) => updateConfig({ lockWindow: enabled })}
                    />
                </SettingsItem>
            </SettingsSection>

            <SettingsSection
                title={t('settings.subtitle_appearance_title')}
                icon={<Subtitles size={20} />}
                description={t('settings.subtitle_appearance_desc')}
            >
                <SettingsItem
                    title={t('live.window_width')}
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <input
                            type="number"
                            min="300"
                            max="1600"
                            step="50"
                            value={captionWindowWidth}
                            onChange={(e) => updateConfig({ captionWindowWidth: Number(e.target.value) })}
                            className="settings-input"
                            style={{ width: '100px', textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('live.font_size')}
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <input
                            type="number"
                            min="12"
                            max="72"
                            step="1"
                            value={captionFontSize}
                            onChange={(e) => updateConfig({ captionFontSize: Number(e.target.value) })}
                            className="settings-input"
                            style={{ width: '100px', textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('live.font_color')}
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <div
                            style={{
                                width: '36px',
                                height: '36px',
                                borderRadius: 'var(--radius-sm)',
                                border: '1px solid var(--color-border)',
                                overflow: 'hidden',
                                flexShrink: 0
                            }}
                        >
                            <input
                                type="color"
                                value={captionFontColor}
                                onChange={(e) => updateConfig({ captionFontColor: e.target.value })}
                                aria-label={t('live.font_color')}
                                style={{
                                    width: '150%',
                                    height: '150%',
                                    padding: 0,
                                    margin: '-25%',
                                    border: 'none',
                                    cursor: 'pointer',
                                    background: 'none'
                                }}
                            />
                        </div>
                        <input
                            type="text"
                            value={captionFontColor}
                            onChange={(e) => updateConfig({ captionFontColor: e.target.value })}
                            placeholder={t('live.font_color_hex_placeholder')}
                            maxLength={7}
                            aria-label={t('live.font_color_hex')}
                            className="settings-input"
                            style={{ width: '100px', fontFamily: 'monospace', textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
