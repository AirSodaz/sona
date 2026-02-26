import React, { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { SettingsGeneralTab } from './settings/SettingsGeneralTab';
import { SettingsMicrophoneTab } from './settings/SettingsMicrophoneTab';
import { SettingsSubtitleTab } from './settings/SettingsSubtitleTab';
import { SettingsModelsTab } from './settings/SettingsModelsTab';
import { SettingsAIServiceTab } from './settings/SettingsAIServiceTab';
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
        appLanguage,
        setAppLanguage,
        theme,
        font,
        microphoneId,
        systemAudioDeviceId,
        muteDuringRecording,
        minimizeToTrayOnExit,
        autoCheckUpdates,

        lockWindow,
        alwaysOnTop,
        startOnLaunch,
        captionWindowWidth,
        captionFontSize,
        captionFontColor,

        aiServiceType,
        setAiServiceType,
        aiBaseUrl,
        aiApiKey,
        aiModel,
        updateAiServiceSetting,

        offlineModelPath,
        punctuationModelPath,
        vadModelPath,
        ctcModelPath,

        vadBufferSize,

        itnRulesOrder,
        setItnRulesOrder,
        enabledITNModels,
        setEnabledITNModels,
        enableITN,
        setEnableITN,
        installedITNModels,

        downloads,
        installedModels,

        handleDownload,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected,
        maxConcurrent,
        restoreDefaultModelSettings,
        updateConfig
    } = useSettingsLogic(isOpen, onClose, initialTab);

    // Focus management
    useFocusTrap(isOpen, onClose, modalRef);

    const handleTabKeyDown = (e: React.KeyboardEvent) => {
        const tabs = ['general', 'microphone', 'subtitle', 'models', 'local', 'ai_service', 'shortcuts', 'about'] as const;
        const currentIndex = tabs.indexOf(activeTab as typeof tabs[number]);

        let nextIndex = -1;
        if (e.key === 'ArrowDown') {
            e.preventDefault();
            nextIndex = (currentIndex + 1) % tabs.length;
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
        } else if (e.key === 'Home') {
            e.preventDefault();
            nextIndex = 0;
        } else if (e.key === 'End') {
            e.preventDefault();
            nextIndex = tabs.length - 1;
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
                            id="ai_service"
                            label={t('settings.ai.title')}
                            Icon={RobotIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                            tabIndex={activeTab === 'ai_service' ? 0 : -1}
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
                        <div className="settings-section-header">
                            <h3 className="settings-section-title">
                                {activeTab === 'general' && t('settings.general')}
                                {activeTab === 'microphone' && t('settings.input_device', { defaultValue: 'Input Device' })}
                                {activeTab === 'subtitle' && t('live.subtitle_settings', { defaultValue: 'Subtitle Settings' })}
                                {activeTab === 'models' && t('settings.model_hub')}
                                {activeTab === 'ai_service' && t('settings.ai.title')}
                                {activeTab === 'local' && t('settings.local_path')}
                                {activeTab === 'shortcuts' && t('shortcuts.title')}
                                {activeTab === 'about' && t('settings.about')}
                            </h3>
                            <div className="settings-divider" />
                        </div>

                        {activeTab === 'general' && (
                            <SettingsGeneralTab
                                appLanguage={appLanguage}
                                setAppLanguage={setAppLanguage}
                                theme={theme}
                                font={font}
                                minimizeToTrayOnExit={minimizeToTrayOnExit}
                                autoCheckUpdates={autoCheckUpdates}
                                updateConfig={updateConfig}
                            />
                        )}

                        {activeTab === 'microphone' && (
                            <SettingsMicrophoneTab
                                microphoneId={microphoneId}
                                systemAudioDeviceId={systemAudioDeviceId}
                                muteDuringRecording={muteDuringRecording}
                                updateConfig={updateConfig}
                            />
                        )}

                        {activeTab === 'subtitle' && (
                            <SettingsSubtitleTab
                                lockWindow={lockWindow}
                                alwaysOnTop={alwaysOnTop}
                                startOnLaunch={startOnLaunch}
                                captionWindowWidth={captionWindowWidth}
                                captionFontSize={captionFontSize}
                                captionFontColor={captionFontColor}
                                updateConfig={updateConfig}
                            />
                        )}

                        {activeTab === 'models' && (
                            <SettingsModelsTab
                                installedModels={installedModels}
                                downloads={downloads}
                                onLoad={handleLoad}
                                onDelete={handleDelete}
                                onDownload={handleDownload}
                                onCancelDownload={handleCancelDownload}
                                isModelSelected={isModelSelected}
                            />
                        )}

                        {activeTab === 'local' && (
                            <SettingsLocalTab
                                offlineModelPath={offlineModelPath}
                                punctuationModelPath={punctuationModelPath}
                                vadModelPath={vadModelPath}
                                ctcModelPath={ctcModelPath}
                                vadBufferSize={vadBufferSize}
                                maxConcurrent={maxConcurrent}
                                updateConfig={updateConfig}

                                itnRulesOrder={itnRulesOrder}
                                setItnRulesOrder={setItnRulesOrder}
                                enabledITNModels={enabledITNModels}
                                setEnabledITNModels={setEnabledITNModels}
                                enableITN={enableITN}
                                setEnableITN={setEnableITN}
                                installedITNModels={installedITNModels}
                                downloads={downloads}
                                onDownloadITN={handleDownload}
                                onCancelDownload={handleCancelDownload}
                                installedModels={installedModels}
                                onRestoreDefaults={restoreDefaultModelSettings}
                            />
                        )}

                        {activeTab === 'ai_service' && (
                            <SettingsAIServiceTab
                                aiServiceType={aiServiceType}
                                setAiServiceType={setAiServiceType}
                                aiBaseUrl={aiBaseUrl}
                                aiApiKey={aiApiKey}
                                aiModel={aiModel}
                                updateAiServiceSetting={updateAiServiceSetting}
                            />
                        )}

                        {activeTab === 'shortcuts' && (
                            <SettingsShortcutsTab />
                        )}

                        {activeTab === 'about' && (
                            <SettingsAboutTab />
                        )}

                    </div>

                </div>
            </div>
        </div >
    );
}
