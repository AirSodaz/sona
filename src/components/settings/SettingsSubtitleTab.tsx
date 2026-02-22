import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '../Switch';

interface SettingsSubtitleTabProps {
    lockWindow: boolean;
    setLockWindow: (enabled: boolean) => void;
    alwaysOnTop: boolean;
    setAlwaysOnTop: (enabled: boolean) => void;
    startOnLaunch: boolean;
    setStartOnLaunch: (enabled: boolean) => void;
    captionWindowWidth: number;
    setCaptionWindowWidth: (width: number) => void;
    captionFontSize: number;
    setCaptionFontSize: (size: number) => void;
    captionFontColor: string;
    setCaptionFontColor: (color: string) => void;
}

export function SettingsSubtitleTab({
    lockWindow,
    setLockWindow,
    alwaysOnTop,
    setAlwaysOnTop,
    startOnLaunch,
    setStartOnLaunch,
    captionWindowWidth,
    setCaptionWindowWidth,
    captionFontSize,
    setCaptionFontSize,
    captionFontColor,
    setCaptionFontColor
}: SettingsSubtitleTabProps): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <div className="settings-group" role="tabpanel">
            <div className="settings-item">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
                    <span className="settings-label" style={{ marginBottom: 0 }}>
                        {t('live.start_on_launch', { defaultValue: 'Start on Launch' })}
                    </span>
                    <Switch
                        checked={startOnLaunch}
                        onChange={setStartOnLaunch}
                    />
                </div>
                <div className="settings-hint">
                    {t('live.start_on_launch_hint', { defaultValue: 'Automatically start Live Caption when the program opens' })}
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
                    <span className="settings-label" style={{ marginBottom: 0 }}>
                        {t('live.lock_window', { defaultValue: 'Lock Window' })}
                    </span>
                    <Switch
                        checked={lockWindow}
                        onChange={setLockWindow}
                    />
                </div>
                <div className="settings-hint">
                    {t('live.lock_window_hint', { defaultValue: 'Make window click-through' })}
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
                    <span className="settings-label" style={{ marginBottom: 0 }}>
                        {t('live.always_on_top', { defaultValue: 'Always on Top' })}
                    </span>
                    <Switch
                        checked={alwaysOnTop}
                        onChange={setAlwaysOnTop}
                    />
                </div>
                <div className="settings-hint">
                    {t('live.always_on_top_hint', { defaultValue: 'Keep window above others' })}
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
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
                            onChange={(e) => setCaptionWindowWidth(Number(e.target.value))}
                            className="settings-input"
                            style={{ width: '100px' }}
                        />
                    </div>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
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
                            onChange={(e) => setCaptionFontSize(Number(e.target.value))}
                            className="settings-input"
                            style={{ width: '100px' }}
                        />
                    </div>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
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
                                onChange={(e) => setCaptionFontColor(e.target.value)}
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
                            onChange={(e) => setCaptionFontColor(e.target.value)}
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
