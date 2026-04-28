import React from 'react';
import { useTranslation } from 'react-i18next';
import { Languages } from 'lucide-react';
import { GeneralIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { useUIConfig, useSetConfig } from '../../stores/configStore';
import type { UIConfig } from '../../types/config';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';

type FontValue = NonNullable<UIConfig['font']>;

function getFontFamily(fontValue: string): string {
    switch (fontValue) {
        case 'mono': return 'monospace';
        case 'serif': return 'serif';
        default: return 'inherit';
    }
}

export function SettingsGeneralTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useUIConfig();
    const updateConfig = useSetConfig();

    const appLanguage = config.appLanguage || 'auto';
    const theme = config.theme || 'auto';
    const font = config.font || 'system';
    const minimizeToTrayOnExit = config.minimizeToTrayOnExit ?? true;
    const autoCheckUpdates = config.autoCheckUpdates ?? true;

    return (
        <SettingsTabContainer id="settings-panel-general" ariaLabelledby="settings-tab-general">
            <SettingsPageHeader 
                icon={<GeneralIcon width={28} height={28} />}
                title={t('settings.general')} 
                description={t('settings.general_description')} 
            />
            <SettingsSection
                title={t('settings.general_title')}
                icon={<Languages size={20} />}
            >
                <SettingsItem
                    title={t('settings.language')}
                    hint={t('settings.language_hint')}
                >
                    <div style={{ width: '200px' }}>
                        <Dropdown
                            id="settings-language"
                            value={appLanguage}
                            onChange={(value) => updateConfig({ appLanguage: value as 'auto' | 'en' | 'zh' })}
                            options={[
                                { value: 'auto', label: t('common.auto') },
                                { value: 'en', label: t('settings.language_en') },
                                { value: 'zh', label: t('settings.language_zh') }
                            ]}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.theme')}
                    layout="vertical"
                >
                    <div className="theme-selector-container">
                        <button
                            className={`theme-card ${theme === 'light' ? 'active' : ''}`}
                            onClick={() => updateConfig({ theme: 'light' })}
                            aria-label={t('settings.theme_light')}
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
                            <span className="theme-label">{t('settings.theme_light')}</span>
                        </button>

                        <button
                            className={`theme-card ${theme === 'dark' ? 'active' : ''}`}
                            onClick={() => updateConfig({ theme: 'dark' })}
                            aria-label={t('settings.theme_dark')}
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
                            <span className="theme-label">{t('settings.theme_dark')}</span>
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
                    title={t('settings.font')}
                >
                    <div style={{ width: '240px' }}>
                        <Dropdown
                            id="settings-font"
                            value={font}
                            onChange={(value) => updateConfig({ font: value as FontValue })}
                            options={[
                                { value: 'system', label: t('settings.font_system'), style: { fontFamily: 'inherit' } },
                                { value: 'serif', label: t('settings.font_serif'), style: { fontFamily: 'serif' } },
                                { value: 'sans', label: t('settings.font_sans'), style: { fontFamily: 'sans-serif' } },
                                { value: 'mono', label: t('settings.font_mono'), style: { fontFamily: 'monospace' } },
                                { value: 'arial', label: t('settings.font_arial'), style: { fontFamily: 'Arial' } },
                                { value: 'georgia', label: t('settings.font_georgia'), style: { fontFamily: 'Georgia' } }
                            ]}
                            style={{ fontFamily: getFontFamily(font) }}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <SettingsSection>
                <SettingsItem
                    title={t('settings.minimize_to_tray')}
                    hint={t('settings.minimize_to_tray_hint')}
                >
                    <Switch
                        checked={minimizeToTrayOnExit}
                        onChange={(enabled) => updateConfig({ minimizeToTrayOnExit: enabled })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.auto_check_updates')}
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
