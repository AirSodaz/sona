import React from 'react';
import { useTranslation } from 'react-i18next';
import { Languages, Stethoscope } from 'lucide-react';
import { GeneralIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { APP_LANGUAGE_OPTIONS } from '../../constants/appLanguages';
import { useUIConfig, useSetConfig } from '../../stores/configStore';
import type { AppLanguagePreference, AppLogLevel, UIConfig } from '../../types/config';
import { markSettingsPerf } from '../../utils/settingsPerf';
import { APP_LOG_LEVELS, normalizeLogLevel } from '../../utils/logLevel';
import { loadBackupSettingsSection } from './settingsGeneralDeferredLoaders';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';

interface SettingsGeneralTabProps {
    isVisible?: boolean;
    isPrewarming?: boolean;
    onOpenDiagnostics?: () => void;
}

type FontValue = NonNullable<UIConfig['font']>;

const BackupSettingsSection = React.lazy(loadBackupSettingsSection);

function getFontFamily(fontValue: string): string {
    switch (fontValue) {
        case 'mono': return 'monospace';
        case 'serif': return 'serif';
        default: return 'inherit';
    }
}

export function SettingsGeneralTab({
    isVisible = true,
    isPrewarming = false,
    onOpenDiagnostics,
}: SettingsGeneralTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const config = useUIConfig();
    const updateConfig = useSetConfig();

    React.useEffect(() => {
        if (!isVisible && !isPrewarming) {
            return;
        }

        const markerPrefix = isPrewarming ? 'settings.prewarm.general' : 'settings.general';
        markSettingsPerf(`${markerPrefix}.commit`);
        const frameId = requestAnimationFrame(() => {
            markSettingsPerf(`${markerPrefix}.raf`);
        });

        return () => cancelAnimationFrame(frameId);
    }, [isPrewarming, isVisible]);

    const appLanguage = config.appLanguage || 'auto';
    const theme = config.theme || 'auto';
    const font = config.font || 'system';
    const minimizeToTrayOnExit = config.minimizeToTrayOnExit ?? true;
    const autoCheckUpdates = config.autoCheckUpdates ?? true;
    const logLevel = normalizeLogLevel(config.logLevel);

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
                            onChange={(value) => updateConfig({ appLanguage: value as AppLanguagePreference })}
                            options={APP_LANGUAGE_OPTIONS.map((option) => ({
                                value: option.value,
                                label: t(option.labelKey, { defaultValue: option.defaultLabel }),
                            }))}
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
                    title={t('settings.log_level')}
                    hint={t('settings.log_level_hint')}
                >
                    <div style={{ width: '180px' }}>
                        <Dropdown
                            id="settings-log-level"
                            aria-label={t('settings.log_level')}
                            value={logLevel}
                            onChange={(value) => updateConfig({ logLevel: value as AppLogLevel })}
                            options={APP_LOG_LEVELS.map((level) => ({
                                value: level,
                                label: t(`settings.log_level_${level}`),
                            }))}
                        />
                    </div>
                </SettingsItem>

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

            <React.Suspense fallback={null}>
                <BackupSettingsSection
                    isVisible={isVisible}
                    isPrewarming={isPrewarming}
                />
            </React.Suspense>

            <SettingsSection
                title={t('settings.diagnostics.title', { defaultValue: 'Model & Environment Diagnostics' })}
                description={t('settings.diagnostics.entry_description', {
                    defaultValue: 'Open a dedicated diagnostics page for the local transcription path, runtime readiness, and packaged environment checks.',
                })}
                icon={<Stethoscope size={20} />}
            >
                <SettingsItem
                    title={t('settings.diagnostics.entry_title', { defaultValue: 'Diagnostics' })}
                    hint={t('settings.diagnostics.entry_hint', {
                        defaultValue: 'Review the current local setup and jump straight to the clearest fix when something is off.',
                    })}
                >
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={onOpenDiagnostics}
                        disabled={!onOpenDiagnostics}
                    >
                        {t('settings.diagnostics.open_button', { defaultValue: 'Open Diagnostics' })}
                    </button>
                </SettingsItem>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
