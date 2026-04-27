import React, { useRef, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Type } from 'lucide-react';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { useModelManager, ModelManagerContext } from '../hooks/useModelManager';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useDialogStore } from '../stores/dialogStore';
import { useErrorDialogStore } from '../stores/errorDialogStore';
import { SettingsGeneralTab } from './settings/SettingsGeneralTab';
import { SettingsMicrophoneTab } from './settings/SettingsMicrophoneTab';
import { SettingsSubtitleTab } from './settings/SettingsSubtitleTab';
import { SettingsModelsTab } from './settings/SettingsModelsTab';
import { SettingsLLMServiceTab } from './settings/SettingsLLMServiceTab';
import { SettingsShortcutsTab } from './settings/SettingsShortcutsTab';
import { SettingsAboutTab } from './settings/SettingsAboutTab';
import { SettingsVocabularyTab } from './settings/SettingsVocabularyTab';
import { SettingsAutomationTab } from './settings/SettingsAutomationTab';
import { SettingsVoiceTypingTab } from './settings/SettingsVoiceTypingTab';
import { SettingsTabButton } from './settings/SettingsTabButton';
import { SettingsNavigationProvider } from './settings/SettingsNavigationContext';
import { SettingsTabInput } from '../hooks/useSettingsLogic';
import './settings/Settings.css';
import {
    GeneralIcon,
    MicIcon,
    SubtitleIcon,
    ModelIcon,
    RobotIcon,
    KeyboardIcon,
    InfoIcon,
    AutomationIcon,
    XIcon,
    BookIcon
} from './Icons';

/** Props for the Settings modal. */
interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
    initialTab?: SettingsTabInput;
}

const SETTINGS_TABS = [
    'general',
    'microphone',
    'subtitle',
    'voice_typing',
    'models',
    'vocabulary',
    'automation',
    'llm_service',
    'shortcuts',
    'about',
] as const;

/**
 * Modal dialog for application settings.
 *
 * Handles configuration for general settings, model management, and app preferences.
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
    } = useSettingsLogic(isOpen, onClose, initialTab);

    const modelManager = useModelManager(isOpen);
    const navigateToTab = useCallback((nextTab: typeof SETTINGS_TABS[number]) => {
        setActiveTab(nextTab);
        requestAnimationFrame(() => {
            const btn = document.getElementById(`settings-tab-${nextTab}`);
            btn?.focus();
        });
    }, [setActiveTab]);
    const navigationContextValue = useMemo(() => ({
        activeTab,
        navigateToTab,
    }), [activeTab, navigateToTab]);

    // Focus management
    useFocusTrap(isOpen, onClose, modalRef);

    useEffect(() => {
        if (!isOpen) return;

        const handleGlobalKeyDown = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.key === 'Tab') {
                // If a dialog is open on top of settings, don't switch tabs
                if (useDialogStore.getState().isOpen || useErrorDialogStore.getState().isOpen) {
                    return;
                }

                e.preventDefault();
                const currentIndex = SETTINGS_TABS.indexOf(activeTab as typeof SETTINGS_TABS[number]);
                const nextIndex = e.shiftKey
                    ? (currentIndex - 1 + SETTINGS_TABS.length) % SETTINGS_TABS.length
                    : (currentIndex + 1) % SETTINGS_TABS.length;

                const nextTab = SETTINGS_TABS[nextIndex];
                navigateToTab(nextTab);
            }
        };

        window.addEventListener('keydown', handleGlobalKeyDown);
        return () => window.removeEventListener('keydown', handleGlobalKeyDown);
    }, [isOpen, activeTab, navigateToTab]);

    const handleTabKeyDown = (e: React.KeyboardEvent) => {
        const currentIndex = SETTINGS_TABS.indexOf(activeTab as typeof SETTINGS_TABS[number]);

        let nextIndex = -1;
        switch (e.key) {
            case 'ArrowDown':
                e.preventDefault();
                nextIndex = (currentIndex + 1) % SETTINGS_TABS.length;
                break;
            case 'ArrowUp':
                e.preventDefault();
                nextIndex = (currentIndex - 1 + SETTINGS_TABS.length) % SETTINGS_TABS.length;
                break;
            case 'Home':
                e.preventDefault();
                nextIndex = 0;
                break;
            case 'End':
                e.preventDefault();
                nextIndex = SETTINGS_TABS.length - 1;
                break;
            default:
                break;
        }

        if (nextIndex !== -1) {
            const nextTab = SETTINGS_TABS[nextIndex];
            navigateToTab(nextTab);
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
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'general' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="microphone"
                            label={t('settings.input_device', { defaultValue: 'Input Device' })}
                            Icon={MicIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'microphone' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="subtitle"
                            label={t('live.subtitle_settings', { defaultValue: 'Subtitle Settings' })}
                            Icon={SubtitleIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'subtitle' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="voice_typing"
                            label={t('settings.voice_typing')}
                            Icon={() => <Type size={18} />}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'voice_typing' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="models"
                            label={t('settings.model_hub')}
                            Icon={ModelIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'models' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="vocabulary"
                            label={t('settings.vocabulary')}
                            Icon={BookIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'vocabulary' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="automation"
                            label={t('settings.automation', { defaultValue: 'Automation' })}
                            Icon={AutomationIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'automation' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="llm_service"
                            label={t('settings.llm.title')}
                            Icon={RobotIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'llm_service' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="shortcuts"
                            label={t('shortcuts.title')}
                            Icon={KeyboardIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
                            tabIndex={activeTab === 'shortcuts' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="about"
                            label={t('settings.about')}
                            Icon={InfoIcon}
                            activeTab={activeTab}
                            setActiveTab={navigateToTab}
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
                        <SettingsNavigationProvider value={navigationContextValue}>
                            <ModelManagerContext.Provider value={modelManager}>
                                {(() => {
                                    switch (activeTab) {
                                        case 'general':
                                            return <SettingsGeneralTab />;
                                        case 'microphone':
                                            return (
                                                <SettingsMicrophoneTab
                                                    isActiveTab={activeTab === 'microphone'}
                                                    isOpen={isOpen}
                                                />
                                            );
                                        case 'subtitle':
                                            return <SettingsSubtitleTab />;
                                        case 'voice_typing':
                                            return <SettingsVoiceTypingTab />;
                                        case 'models':
                                            return <SettingsModelsTab />;
                                        case 'vocabulary':
                                            return <SettingsVocabularyTab />;
                                        case 'automation':
                                            return <SettingsAutomationTab />;
                                        case 'llm_service':
                                            return <SettingsLLMServiceTab />;
                                        case 'shortcuts':
                                            return <SettingsShortcutsTab />;
                                        case 'about':
                                            return <SettingsAboutTab />;
                                        default:
                                            return null;
                                    }
                                })()}
                            </ModelManagerContext.Provider>
                        </SettingsNavigationProvider>
                    </div>

                </div>
            </div>
        </div>
    );
}
