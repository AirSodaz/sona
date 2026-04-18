import React, { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { useModelManager } from '../hooks/useModelManager';
import { useLlmConfig } from '../hooks/useLlmConfig';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { SettingsGeneralTab } from './settings/SettingsGeneralTab';
import { SettingsMicrophoneTab } from './settings/SettingsMicrophoneTab';
import { SettingsSubtitleTab } from './settings/SettingsSubtitleTab';
import { SettingsModelsTab } from './settings/SettingsModelsTab';
import { SettingsLLMServiceTab } from './settings/SettingsLLMServiceTab';
import { SettingsLocalTab } from './settings/SettingsLocalTab';
import { SettingsShortcutsTab } from './settings/SettingsShortcutsTab';
import { SettingsAboutTab } from './settings/SettingsAboutTab';
import { SettingsTabButton } from './settings/SettingsTabButton';
import {
    GeneralIcon,
    MicIcon,
    SubtitleIcon,
    ModelIcon,
    RobotIcon,
    LocalIcon,
    KeyboardIcon,
    InfoIcon,
    XIcon
} from './Icons';

/** Props for the Settings modal. */
interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
    initialTab?: 'general' | 'microphone' | 'subtitle' | 'models' | 'local' | 'shortcuts' | 'about';
}

/**
 * Modal dialog for application settings.
 *
 * Handles configuration for general settings, model management, and local paths.
 *
 * @param props Component props.
 * @return The settings modal or null if not closed.
 */
export function Settings({ isOpen, onClose, initialTab }: SettingsProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const modalRef = useRef<HTMLDivElement>(null);

    const {
        activeTab,
        setActiveTab,
        config,
        updateConfig,
    } = useSettingsLogic(isOpen, onClose, initialTab);

    const {
        downloads,
        installedModels,
        handleDownload,
        handleCancelDownload,
        handleDelete,
        restoreDefaultModelSettings,
    } = useModelManager(isOpen);

    const { changeLlmServiceType } = useLlmConfig();

    // Focus management
    useFocusTrap(isOpen, onClose, modalRef);

    const handleTabKeyDown = (e: React.KeyboardEvent) => {
        const tabs = ['general', 'microphone', 'subtitle', 'models', 'local', 'llm_service', 'shortcuts', 'about'] as const;
        const currentIndex = tabs.indexOf(activeTab as typeof tabs[number]);

        let nextIndex = -1;
        switch (e.key) {
            case 'ArrowDown':
                e.preventDefault();
                nextIndex = (currentIndex + 1) % tabs.length;
                break;
            case 'ArrowUp':
                e.preventDefault();
                nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
                break;
            case 'Home':
                e.preventDefault();
                nextIndex = 0;
                break;
            case 'End':
                e.preventDefault();
                nextIndex = tabs.length - 1;
                break;
            default:
                break;
        }

        if (nextIndex !== -1) {
            const nextTab = tabs[nextIndex];
            setActiveTab(nextTab);
            // Move focus to the new tab button
            requestAnimationFrame(() => {
                const btn = document.getElementById(`settings-tab-${nextTab}`);
                btn?.focus();
            });
        }
    };

    if (!isOpen) return null;

    return (
        <div className="settings-overlay" onClick={onClose}>
            <div
                ref={modalRef}
                className="settings-modal"
                onClick={(e) => e.stopPropagation()}
                role="dialog"
                aria-modal="true"
                aria-labelledby="settings-title"
                tabIndex={-1}
                style={{ outline: 'none' }}
            >
                {/* Sidebar */}
                <div className="settings-sidebar">
                    <div className="settings-sidebar-header">
                        <h2 id="settings-title">{t('settings.title')}</h2>
                    </div>

                    <div
                        className="settings-tabs-container"
                        role="tablist"
                        aria-orientation="vertical"
                        onKeyDown={handleTabKeyDown}
                    >
                        <SettingsTabButton
                            id="general"
                            label={t('settings.general')}
                            Icon={GeneralIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'general' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="microphone"
                            label={t('settings.input_device', { defaultValue: 'Input Device' })}
                            Icon={MicIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'microphone' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="subtitle"
                            label={t('live.subtitle_settings', { defaultValue: 'Subtitle Settings' })}
                            Icon={SubtitleIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'subtitle' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="models"
                            label={t('settings.model_hub')}
                            Icon={ModelIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'models' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="local"
                            label={t('settings.local_path')}
                            Icon={LocalIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'local' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="llm_service"
                            label={t('settings.llm.title')}
                            Icon={RobotIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'llm_service' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="shortcuts"
                            label={t('shortcuts.title')}
                            Icon={KeyboardIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'shortcuts' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="about"
                            label={t('settings.about')}
                            Icon={InfoIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'about' ? 0 : -1}
                        />
                    </div>
                </div>

                {/* Main Content */}
                <div className="settings-content">
                    {/* Header with close button */}
                    <div className="settings-close-btn-container">
                        <button
                            className="btn btn-icon"
                            onClick={onClose}
                            aria-label={t('common.close')}
                            data-tooltip={t('common.close')}
                            data-tooltip-pos="bottom-left"
                        >
                            <XIcon />
                        </button>
                    </div>

                    {/* Scrollable Content Area */}
                    <div className="settings-content-scroll full-height">
                        {(() => {
                            switch (activeTab) {
                                case 'general':
                                    return (
                                        <SettingsGeneralTab
                                            config={config}
                                            updateConfig={updateConfig}
                                        />
                                    );
                                case 'microphone':
                                    return (
                                        <SettingsMicrophoneTab
                                            config={config}
                                            updateConfig={updateConfig}
                                            isActiveTab={activeTab === 'microphone'}
                                            isOpen={isOpen}
                                        />
                                    );
                                case 'subtitle':
                                    return (
                                        <SettingsSubtitleTab
                                            config={config}
                                            updateConfig={updateConfig}
                                        />
                                    );
                                case 'models':
                                    return (
                                        <SettingsModelsTab
                                            config={config}
                                            updateConfig={updateConfig}
                                            installedModels={installedModels}
                                            downloads={downloads}
                                            onDelete={handleDelete}
                                            onDownload={handleDownload}
                                            onCancelDownload={handleCancelDownload}
                                        />
                                    );
                                case 'local':
                                    return (
                                        <SettingsLocalTab
                                            config={config}
                                            updateConfig={updateConfig}
                                            onRestoreDefaults={restoreDefaultModelSettings}
                                        />
                                    );
                                case 'llm_service':
                                    return (
                                        <SettingsLLMServiceTab
                                            config={config}
                                            updateConfig={updateConfig}
                                            changeLlmServiceType={changeLlmServiceType}
                                        />
                                    );
                                case 'shortcuts':
                                    return (
                                        <SettingsShortcutsTab />
                                    );
                                case 'about':
                                    return (
                                        <SettingsAboutTab />
                                    );
                                default:
                                    return null;
                            }
                        })()}

                    </div>

                </div>
            </div>
        </div >
    );
}
