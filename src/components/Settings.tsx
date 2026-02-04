import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useDialogStore } from '../stores/dialogStore';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { SettingsGeneralTab } from './settings/SettingsGeneralTab';
import { SettingsModelsTab } from './settings/SettingsModelsTab';
import { SettingsLocalTab } from './settings/SettingsLocalTab';
import {
    GeneralIcon,
    ModelIcon,
    LocalIcon,
    XIcon
} from './Icons';

interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
}

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

        downloadingId,
        deletingId,
        progress,
        statusMessage,
        installedModels,

        handleSave,
        handleBrowse,
        handleDownload,
        handleDownloadITN,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected
    } = useSettingsLogic(isOpen, onClose);

    // Focus management
    useEffect(() => {
        if (isOpen) {
            const previousFocus = document.activeElement as HTMLElement;
            // Wait for render
            requestAnimationFrame(() => {
                modalRef.current?.focus();
            });

            function handleKeyDown(e: KeyboardEvent) {
                if (e.key === 'Escape') {
                    // Only close if no other dialog is open (GlobalDialog)
                    if (useDialogStore.getState().isOpen) return;
                    onClose();
                    return;
                }

                if (e.key === 'Tab') {
                    if (!modalRef.current) return;

                    // Trap focus inside modal
                    const focusableSelector = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled])';
                    const focusableElements = modalRef.current.querySelectorAll(focusableSelector);

                    if (focusableElements.length === 0) return;

                    const firstElement = focusableElements[0] as HTMLElement;
                    const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                    if (e.shiftKey) {
                        if (document.activeElement === firstElement) {
                            e.preventDefault();
                            lastElement.focus();
                        }
                    } else {
                        if (document.activeElement === lastElement) {
                            e.preventDefault();
                            firstElement.focus();
                        }
                    }
                }
            }
            window.addEventListener('keydown', handleKeyDown);

            return () => {
                window.removeEventListener('keydown', handleKeyDown);
                previousFocus?.focus();
            };
        }
    }, [isOpen, onClose]);

    function renderTabButton(id: 'general' | 'models' | 'local', label: string, Icon: React.FC) {
        return (
            <button
                className={`settings-tab-btn ${activeTab === id ? 'active' : ''}`}
                onClick={() => setActiveTab(id)}
                role="tab"
                aria-selected={activeTab === id}
                aria-controls={`settings-panel-${id}`}
                id={`settings-tab-${id}`}
            >
                <Icon />
                {label}
            </button>
        );
    }

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
                        {renderTabButton('general', t('settings.general'), GeneralIcon)}
                        {renderTabButton('models', t('settings.model_hub'), ModelIcon)}
                        {renderTabButton('local', t('settings.local_path'), LocalIcon)}
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
                    <div className="settings-content-scroll">
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
                                downloadingId={downloadingId}
                                deletingId={deletingId}
                                progress={progress}
                                statusMessage={statusMessage}
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
                                downloadingId={downloadingId}
                                progress={progress}
                                onDownloadITN={handleDownloadITN}
                                onCancelDownload={handleCancelDownload}
                            />
                        )}

                    </div>

                    {/* Footer */}
                    <div className="settings-footer">
                        <button className="btn btn-secondary" onClick={onClose}>
                            {t('common.cancel')}
                        </button>
                        <button className="btn btn-primary" onClick={handleSave}>
                            {t('settings.save_button')}
                        </button>
                    </div>
                </div>
            </div>
        </div >
    );
};

export default Settings;
