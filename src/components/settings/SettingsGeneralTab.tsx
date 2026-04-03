import React from 'react';
import { useTranslation } from 'react-i18next';
import { Settings } from 'lucide-react';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { AppConfig } from '../../types/transcript';
import { SettingsTabContainer, SettingsSection, SettingsItem } from './SettingsLayout';

interface SettingsGeneralTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
}

function getFontFamily(fontValue: string): string {
    switch (fontValue) {
        case 'mono': return 'monospace';
        case 'serif': return 'serif';
        default: return 'inherit';
    }
}

export function SettingsGeneralTab({
    config,
    updateConfig
}: SettingsGeneralTabProps): React.JSX.Element {
    const { t } = useTranslation();

    const appLanguage = config.appLanguage || 'auto';
    const theme = config.theme || 'auto';
    const font = config.font || 'system';
    const minimizeToTrayOnExit = config.minimizeToTrayOnExit ?? true;
    const autoCheckUpdates = config.autoCheckUpdates ?? true;

    return (
        <SettingsTabContainer id="settings-panel-general" ariaLabelledby="settings-tab-general">
            <SettingsSection
                title={t('settings.general_title', { defaultValue: 'General Preferences' })}
                icon={<Settings size={20} />}
                description={t('settings.general_description', { defaultValue: 'Manage basic application behaviors and appearance.' })}
            >
                <SettingsItem
                    title={t('settings.language')}
                    hint={t('settings.language_hint', { defaultValue: 'Change the display language.' })}
                >
                    <div style={{ width: '200px' }}>
                        <Dropdown
                            id="settings-language"
                            value={appLanguage}
                            onChange={(value) => updateConfig({ appLanguage: value as 'auto' | 'en' | 'zh' })}
                            options={[
                                { value: 'auto', label: t('common.auto') },
                                { value: 'en', label: 'English' },
                                { value: 'zh', label: '中文' }
                            ]}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.theme', { defaultValue: 'Theme' })}
                    layout="vertical"
                >
                    <div className="theme-selector-container">
                        <button
                            className={`theme-card ${theme === 'light' ? 'active' : ''}`}
                            onClick={() => updateConfig({ theme: 'light' })}
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
                            onClick={() => updateConfig({ theme: 'dark' })}
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
                            onClick={() => updateConfig({ theme: 'auto' })}
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
                </SettingsItem>

                <SettingsItem
                    title={t('settings.font', { defaultValue: 'Font Family' })}
                >
                    <div style={{ width: '240px' }}>
                        <Dropdown
                            id="settings-font"
                            value={font}
                            onChange={(value) => updateConfig({ font: value as any })}
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
                </SettingsItem>
            </SettingsSection>

            <SettingsSection>
                <SettingsItem
                    title={t('settings.minimize_to_tray', { defaultValue: 'Minimize to tray on exit' })}
                    hint={t('settings.minimize_to_tray_hint', { defaultValue: 'When enabled, closing the window will minimize the application to the system tray instead of quitting.' })}
                >
                    <Switch
                        checked={minimizeToTrayOnExit}
                        onChange={(enabled) => updateConfig({ minimizeToTrayOnExit: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.auto_check_updates', { defaultValue: 'Automatically check for updates' })}
                >
                    <Switch
                        checked={autoCheckUpdates}
                        onChange={(enabled) => updateConfig({ autoCheckUpdates: enabled })}
                    />
                </SettingsItem>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
