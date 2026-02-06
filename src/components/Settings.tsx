import React, { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { SettingsGeneralTab } from './settings/SettingsGeneralTab';
import { SettingsModelsTab } from './settings/SettingsModelsTab';
import { SettingsLocalTab } from './settings/SettingsLocalTab';
import { SettingsTabButton } from './settings/SettingsTabButton';
import {
    GeneralIcon,
    ModelIcon,
    LocalIcon,
    XIcon
} from './Icons';

/** Props for the Settings modal. */
interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
}

/**
 * Modal dialog for application settings.
 *
 * Handles configuration for general settings, model management, and local paths.
 *
 * @param props Component props.
 * @return The settings modal or null if not closed.
 */
export function Settings({ isOpen, onClose }: SettingsProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const modalRef = useRef<HTMLDivElement>(null);

    const {
        activeTab,
        setActiveTab,
        appLanguage,
        setAppLanguage,
        theme,
        setTheme,
        font,
        setFont,

        streamingModelPath,
        setStreamingModelPath,
        offlineModelPath,
        setOfflineModelPath,
        punctuationModelPath,
        setPunctuationModelPath,
        vadModelPath,
        setVadModelPath,

        vadBufferSize,
        setVadBufferSize,

        itnRulesOrder,
        setItnRulesOrder,
        enabledITNModels,
        setEnabledITNModels,
        installedITNModels,

        downloads,
        // downloadingId, progress, statusMessage,
        installedModels,

        handleBrowse,
        handleDownload,
        handleDownloadITN,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected
    } = useSettingsLogic(isOpen, onClose);

    // Focus management
    useFocusTrap(isOpen, onClose, modalRef);

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

                    <div className="settings-tabs-container" role="tablist" aria-orientation="vertical">
                        <SettingsTabButton
                            id="general"
                            label={t('settings.general')}
                            Icon={GeneralIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                        />
                        <SettingsTabButton
                            id="models"
                            label={t('settings.model_hub')}
                            Icon={ModelIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
                        />
                        <SettingsTabButton
                            id="local"
                            label={t('settings.local_path')}
                            Icon={LocalIcon}
                            activeTab={activeTab}
                            setActiveTab={setActiveTab}
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
                                {activeTab === 'models' && t('settings.model_hub')}
                                {activeTab === 'local' && t('settings.local_path')}
                            </h3>
                            <div className="settings-divider" />
                        </div>

                        {activeTab === 'general' && (
                            <SettingsGeneralTab
                                appLanguage={appLanguage}
                                setAppLanguage={setAppLanguage}
                                theme={theme}
                                setTheme={setTheme}
                                font={font}
                                setFont={setFont}
                            />
                        )}

                        {activeTab === 'models' && (
                            <SettingsModelsTab
                                installedModels={installedModels}
                                downloads={downloads}
                                vadBufferSize={vadBufferSize}
                                setVadBufferSize={setVadBufferSize}
                                onLoad={handleLoad}
                                onDelete={handleDelete}
                                onDownload={handleDownload}
                                onCancelDownload={handleCancelDownload}
                                isModelSelected={isModelSelected}
                            />
                        )}

                        {activeTab === 'local' && (
                            <SettingsLocalTab
                                streamingModelPath={streamingModelPath}
                                setStreamingModelPath={setStreamingModelPath}
                                offlineModelPath={offlineModelPath}
                                setOfflineModelPath={setOfflineModelPath}
                                punctuationModelPath={punctuationModelPath}
                                setPunctuationModelPath={setPunctuationModelPath}
                                vadModelPath={vadModelPath}
                                setVadModelPath={setVadModelPath}
                                handleBrowse={handleBrowse}

                                itnRulesOrder={itnRulesOrder}
                                setItnRulesOrder={setItnRulesOrder}
                                enabledITNModels={enabledITNModels}
                                setEnabledITNModels={setEnabledITNModels}
                                installedITNModels={installedITNModels}
                                downloads={downloads}
                                onDownloadITN={handleDownloadITN}
                                onCancelDownload={handleCancelDownload}
                            />
                        )}

                    </div>

                </div>
            </div>
        </div >
    );
};

export default Settings;
