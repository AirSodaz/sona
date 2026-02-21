import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '../Switch';

interface SettingsSubtitleTabProps {
    lockWindow: boolean;
    setLockWindow: (enabled: boolean) => void;
    alwaysOnTop: boolean;
    setAlwaysOnTop: (enabled: boolean) => void;
}

export function SettingsSubtitleTab({
    lockWindow,
    setLockWindow,
    alwaysOnTop,
    setAlwaysOnTop
}: SettingsSubtitleTabProps): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <div className="settings-group" role="tabpanel">
            <div className="settings-item">
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
        </div>
    );
}
