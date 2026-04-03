import React from 'react';
import { useTranslation } from 'react-i18next';
import { Subtitles, Monitor } from 'lucide-react';
import { Switch } from '../Switch';
import { AppConfig } from '../../types/transcript';
import { SettingsTabContainer, SettingsSection, SettingsItem } from './SettingsLayout';

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
            <SettingsSection
                title={t('settings.subtitle_behavior_title', { defaultValue: 'Window Behavior' })}
                icon={<Monitor size={20} />}
                description={t('settings.subtitle_behavior_desc', { defaultValue: 'Control how the live caption window behaves.' })}
            >
                <SettingsItem
                    title={t('live.start_on_launch', { defaultValue: 'Start on Launch' })}
                    hint={t('live.start_on_launch_hint', { defaultValue: 'Automatically start Live Caption when the program opens' })}
                >
                    <Switch
                        checked={startOnLaunch}
                        onChange={(enabled) => updateConfig({ startOnLaunch: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('live.always_on_top', { defaultValue: 'Always on Top' })}
                    hint={t('live.always_on_top_hint', { defaultValue: 'Keep window above others' })}
                >
                    <Switch
                        checked={alwaysOnTop}
                        onChange={(enabled) => updateConfig({ alwaysOnTop: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('live.lock_window', { defaultValue: 'Lock Window' })}
                    hint={t('live.lock_window_hint', { defaultValue: 'Make window click-through' })}
                >
                    <Switch
                        checked={lockWindow}
                        onChange={(enabled) => updateConfig({ lockWindow: enabled })}
                    />
                </SettingsItem>
            </SettingsSection>

            <SettingsSection
                title={t('settings.subtitle_appearance_title', { defaultValue: 'Appearance' })}
                icon={<Subtitles size={20} />}
                description={t('settings.subtitle_appearance_desc', { defaultValue: 'Customize the look of your live captions.' })}
            >
                <SettingsItem
                    title={t('live.window_width', { defaultValue: 'Floating Window Width' })}
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
                    title={t('live.font_size', { defaultValue: 'Font Size' })}
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
                    title={t('live.font_color', { defaultValue: 'Font Color' })}
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
                                aria-label={t('live.font_color', { defaultValue: 'Font Color' })}
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
                            placeholder="#RRGGBB"
                            maxLength={7}
                            aria-label={t('live.font_color_hex', { defaultValue: 'Hex color code' })}
                            className="settings-input"
                            style={{ width: '100px', fontFamily: 'monospace', textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
