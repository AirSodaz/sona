import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '../Switch';
import { AppConfig } from '../../types/transcript';

interface SettingsSubtitleTabProps {
    lockWindow: boolean;
    alwaysOnTop: boolean;
    startOnLaunch: boolean;
    captionWindowWidth: number;
    captionFontSize: number;
    captionFontColor: string;
    updateConfig: (config: Partial<AppConfig>) => void;
}

export function SettingsSubtitleTab({
    lockWindow,
    alwaysOnTop,
    startOnLaunch,
    captionWindowWidth,
    captionFontSize,
    captionFontColor,
    updateConfig
}: SettingsSubtitleTabProps): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <div className="settings-group" role="tabpanel">
            <div className="settings-item">
                <div className="settings-item-row">
                    <div>
                        <div className="settings-label" style={{ marginBottom: 0 }}>
                            {t('live.start_on_launch', { defaultValue: 'Start on Launch' })}
                        </div>
                        <div className="settings-hint">
                            {t('live.start_on_launch_hint', { defaultValue: 'Automatically start Live Caption when the program opens' })}
                        </div>
                    </div>
                    <Switch
                        checked={startOnLaunch}
                        onChange={(enabled) => updateConfig({ startOnLaunch: enabled })}
                    />
                </div>
            </div>

            <div className="settings-item with-divider">
                <div className="settings-item-row">
                    <div>
                        <div className="settings-label" style={{ marginBottom: 0 }}>
                            {t('live.lock_window', { defaultValue: 'Lock Window' })}
                        </div>
                        <div className="settings-hint">
                            {t('live.lock_window_hint', { defaultValue: 'Make window click-through' })}
                        </div>
                    </div>
                    <Switch
                        checked={lockWindow}
                        onChange={(enabled) => updateConfig({ lockWindow: enabled })}
                    />
                </div>
            </div>

            <div className="settings-item with-divider">
                <div className="settings-item-row">
                    <div>
                        <div className="settings-label" style={{ marginBottom: 0 }}>
                            {t('live.always_on_top', { defaultValue: 'Always on Top' })}
                        </div>
                        <div className="settings-hint">
                            {t('live.always_on_top_hint', { defaultValue: 'Keep window above others' })}
                        </div>
                    </div>
                    <Switch
                        checked={alwaysOnTop}
                        onChange={(enabled) => updateConfig({ alwaysOnTop: enabled })}
                    />
                </div>
            </div>

            <div className="settings-item with-divider">
                <div className="settings-item-row">
                    <span className="settings-label" style={{ marginBottom: 0 }}>
                        {t('live.window_width', { defaultValue: 'Floating Window Width' })}
                    </span>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <input
                            type="number"
                            min="300"
                            max="1600"
                            step="50"
                            value={captionWindowWidth}
                            onChange={(e) => updateConfig({ captionWindowWidth: Number(e.target.value) })}
                            className="settings-input"
                            style={{ width: '100px' }}
                        />
                    </div>
                </div>
            </div>

            <div className="settings-item with-divider">
                <div className="settings-item-row">
                    <span className="settings-label" style={{ marginBottom: 0 }}>
                        {t('live.font_size', { defaultValue: 'Font Size' })}
                    </span>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                        <input
                            type="number"
                            min="12"
                            max="72"
                            step="1"
                            value={captionFontSize}
                            onChange={(e) => updateConfig({ captionFontSize: Number(e.target.value) })}
                            className="settings-input"
                            style={{ width: '100px' }}
                        />
                    </div>
                </div>
            </div>

            <div className="settings-item with-divider">
                <div className="settings-item-row">
                    <span className="settings-label" style={{ marginBottom: 0 }}>
                        {t('live.font_color', { defaultValue: 'Font Color' })}
                    </span>
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
                            style={{ width: '100px', fontFamily: 'monospace' }}
                        />
                    </div>
                </div>
            </div>
        </div>
    );
}
