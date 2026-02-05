import React from 'react';
import { useTranslation } from 'react-i18next';

interface SettingsGeneralTabProps {
    appLanguage: string;
    setAppLanguage: (lang: 'auto' | 'en' | 'zh') => void;
    theme: string;
    setTheme: (theme: 'auto' | 'light' | 'dark') => void;
    font: string;
    setFont: (font: string) => void;
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
    setFont
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
                    <select
                        id="settings-language"
                        className="settings-input"
                        value={appLanguage}
                        onChange={(e) => setAppLanguage(e.target.value as 'auto' | 'en' | 'zh')}
                        style={{ width: '100%' }}
                    >
                        <option value="auto">{t('common.auto')}</option>
                        <option value="en">English</option>
                        <option value="zh">中文</option>
                    </select>
                </div>
                <div className="settings-hint">
                    {t('settings.language_hint', { defaultValue: '' })}
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <label htmlFor="settings-theme" className="settings-label">{t('settings.theme', { defaultValue: 'Theme' })}</label>
                <div style={{ maxWidth: 300 }}>
                    <select
                        id="settings-theme"
                        className="settings-input"
                        value={theme}
                        onChange={(e) => setTheme(e.target.value as 'auto' | 'light' | 'dark')}
                        style={{ width: '100%' }}
                    >
                        <option value="auto">{t('common.auto')}</option>
                        <option value="light">{t('settings.theme_light', { defaultValue: 'Light' })}</option>
                        <option value="dark">{t('settings.theme_dark', { defaultValue: 'Dark' })}</option>
                    </select>
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                <label htmlFor="settings-font" className="settings-label">{t('settings.font', { defaultValue: 'Font' })}</label>
                <div style={{ maxWidth: 300 }}>
                    <select
                        id="settings-font"
                        className="settings-input"
                        value={font}
                        onChange={(e) => setFont(e.target.value)}
                        style={{ width: '100%', fontFamily: getFontFamily(font) }}
                    >
                        <option value="system">{t('settings.font_system', { defaultValue: 'System Default' })}</option>
                        <option value="serif">Serif (Merriweather)</option>
                        <option value="sans">Sans Serif (Inter)</option>
                        <option value="mono">Monospace (JetBrains Mono)</option>
                        <option value="arial">Arial</option>
                        <option value="georgia">Georgia</option>
                    </select>
                </div>
            </div>
        </div>
    );
}
