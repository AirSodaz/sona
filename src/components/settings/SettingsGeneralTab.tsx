import React from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';

interface SettingsGeneralTabProps {
    appLanguage: string;
    setAppLanguage: (lang: 'auto' | 'en' | 'zh') => void;
    theme: string;
    setTheme: (theme: 'auto' | 'light' | 'dark') => void;
    font: string;
    setFont: (font: string) => void;
    minimizeToTrayOnExit: boolean;
    setMinimizeToTrayOnExit: (enabled: boolean) => void;
}

function getFontFamily(fontValue: string): string {
    switch (fontValue) {
        case 'mono': return 'monospace';
        case 'serif': return 'serif';
        default: return 'inherit';
    }
}

export function SettingsGeneralTab({
    appLanguage,
    setAppLanguage,
    theme,
    setTheme,
    font,
    setFont,
    minimizeToTrayOnExit,
    setMinimizeToTrayOnExit
}: SettingsGeneralTabProps): React.JSX.Element {
    const { t } = useTranslation();

    return (
        <div
            className="settings-group"
            role="tabpanel"
            id="settings-panel-general"
            aria-labelledby="settings-tab-general"
            tabIndex={0}
        >
            <div className="settings-item">
                <label htmlFor="settings-language" className="settings-label">{t('settings.language')}</label>
                <div style={{ maxWidth: 300 }}>
                    <Dropdown
                        id="settings-language"
                        value={appLanguage}
                        onChange={(value) => setAppLanguage(value as 'auto' | 'en' | 'zh')}
                        options={[
                            { value: 'auto', label: t('common.auto') },
                            { value: 'en', label: 'English' },
                            { value: 'zh', label: '中文' }
                        ]}
                    />
                </div>
                <div className="settings-hint">
                    {t('settings.language_hint', { defaultValue: '' })}
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <label htmlFor="settings-theme" className="settings-label">{t('settings.theme', { defaultValue: 'Theme' })}</label>
                <div style={{ maxWidth: 400 }}>
                    <div className="theme-selector-container">
                        <button
                            className={`theme-card ${theme === 'light' ? 'active' : ''}`}
                            onClick={() => setTheme('light')}
                            aria-label={t('settings.theme_light', { defaultValue: 'Light' })}
                            aria-pressed={theme === 'light'}
                        >
                            <div className="theme-preview light">
                                <div className="theme-preview-window">
                                    <div className="theme-preview-lines">
                                        <div className="line short"></div>
                                        <div className="line long"></div>
                                    </div>
                                    <div className="theme-preview-sidebar"></div>
                                </div>
                            </div>
                            <span className="theme-label">{t('settings.theme_light', { defaultValue: 'Light' })}</span>
                        </button>

                        <button
                            className={`theme-card ${theme === 'dark' ? 'active' : ''}`}
                            onClick={() => setTheme('dark')}
                            aria-label={t('settings.theme_dark', { defaultValue: 'Dark' })}
                            aria-pressed={theme === 'dark'}
                        >
                            <div className="theme-preview dark">
                                <div className="theme-preview-window">
                                    <div className="theme-preview-lines">
                                        <div className="line short"></div>
                                        <div className="line long"></div>
                                    </div>
                                    <div className="theme-preview-sidebar"></div>
                                </div>
                            </div>
                            <span className="theme-label">{t('settings.theme_dark', { defaultValue: 'Dark' })}</span>
                        </button>

                        <button
                            className={`theme-card ${theme === 'auto' ? 'active' : ''}`}
                            onClick={() => setTheme('auto')}
                            aria-label={t('common.auto')}
                            aria-pressed={theme === 'auto'}
                        >
                            <div className="theme-preview auto">
                                <div className="theme-preview-window">
                                    <div className="theme-preview-lines">
                                        <div className="line short"></div>
                                        <div className="line long"></div>
                                    </div>
                                    <div className="theme-preview-sidebar"></div>
                                </div>
                            </div>
                            <span className="theme-label">{t('common.auto')}</span>
                        </button>
                    </div>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <label htmlFor="settings-font" className="settings-label">{t('settings.font', { defaultValue: 'Font' })}</label>
                <div style={{ maxWidth: 300 }}>
                    <Dropdown
                        id="settings-font"
                        value={font}
                        onChange={setFont}
                        options={[
                            { value: 'system', label: t('settings.font_system', { defaultValue: 'System Default' }), style: { fontFamily: 'inherit' } },
                            { value: 'serif', label: 'Serif (Merriweather)', style: { fontFamily: 'serif' } },
                            { value: 'sans', label: 'Sans Serif (Inter)', style: { fontFamily: 'sans-serif' } },
                            { value: 'mono', label: 'Monospace (JetBrains Mono)', style: { fontFamily: 'monospace' } },
                            { value: 'arial', label: 'Arial', style: { fontFamily: 'Arial' } },
                            { value: 'georgia', label: 'Georgia', style: { fontFamily: 'Georgia' } }
                        ]}
                        style={{ fontFamily: getFontFamily(font) }}
                    />
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
                    <div>
                        <div className="settings-label" style={{ marginBottom: 0 }}>{t('settings.minimize_to_tray', { defaultValue: 'Minimize to tray on exit' })}</div>
                        <div className="settings-hint">
                            {t('settings.minimize_to_tray_hint', { defaultValue: 'When enabled, closing the window will minimize the application to the system tray instead of quitting.' })}
                        </div>
                    </div>
                    <Switch
                        checked={minimizeToTrayOnExit}
                        onChange={setMinimizeToTrayOnExit}
                    />
                </div>
            </div>
        </div>
    );
}
