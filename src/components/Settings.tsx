import React, { Suspense, lazy, useRef, useEffect, useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BarChart3, Server } from 'lucide-react';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useDialogStore } from '../stores/dialogStore';
import { useErrorDialogStore } from '../stores/errorDialogStore';
import { SettingsTabButton } from './settings/SettingsTabButton';
import { SettingsNavigationProvider } from './settings/SettingsNavigationContext';
import { SettingsTabInput, type SettingsTab } from '../hooks/useSettingsLogic';
import { markSettingsPerf } from '../utils/settingsPerf';
import {
    SETTINGS_TABS,
    loadSettingsAboutTab,
    loadSettingsAutomationTab,
    loadSettingsDashboardTab,
    loadSettingsGeneralTab,
    loadSettingsLLMServiceTab,
    loadSettingsMicrophoneTab,
    loadSettingsModelsPane,
    loadSettingsShortcutsTab,
    loadSettingsSubtitleTab,
    loadSettingsVocabularyTab,
} from './settings/settingsLoaders';
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
import type { LlmProvider } from '../types/transcript';

/** Props for the Settings modal. */
interface SettingsProps {
    isOpen: boolean;
    prewarm?: boolean;
    onClose: () => void;
    initialTab?: SettingsTabInput;
    onOpenDiagnostics?: () => void;
    onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
}

const SettingsGeneralTab = lazy(loadSettingsGeneralTab);
const SettingsDashboardTab = lazy(loadSettingsDashboardTab);
const SettingsMicrophoneTab = lazy(loadSettingsMicrophoneTab);
const SettingsSubtitleTab = lazy(loadSettingsSubtitleTab);
const SettingsModelsPane = lazy(loadSettingsModelsPane);
const SettingsVocabularyTab = lazy(loadSettingsVocabularyTab);
const SettingsAutomationTab = lazy(loadSettingsAutomationTab);
const SettingsApiServerTab = lazy(() => import('./settings/settingsLoaders').then(m => m.loadSettingsApiServerTab()));
const SettingsLLMServiceTab = lazy(loadSettingsLLMServiceTab);
const SettingsShortcutsTab = lazy(loadSettingsShortcutsTab);
const SettingsAboutTab = lazy(loadSettingsAboutTab);

function renderSettingsPane(
    activeTab: SettingsTab,
    isOpen: boolean,
    isActive: boolean,
    isPrewarming = false,
    onOpenDiagnostics?: () => void,
    onOpenLlmProviderDetails?: (provider: LlmProvider) => void,
): React.JSX.Element | null {
    switch (activeTab) {
        case 'general':
            return (
                <SettingsGeneralTab
                    isVisible={isOpen}
                    isPrewarming={isPrewarming}
                    onOpenDiagnostics={onOpenDiagnostics}
                />
            );
        case 'dashboard':
            return <SettingsDashboardTab isActive={isActive} />;
        case 'microphone':
            return (
                <SettingsMicrophoneTab
                    isActiveTab={isActive}
                    isOpen={isOpen}
                />
            );
        case 'subtitle':
            return <SettingsSubtitleTab />;
        case 'models':
            return <SettingsModelsPane isOpen={isOpen} isActive={isActive} />;
        case 'vocabulary':
            return <SettingsVocabularyTab />;
        case 'automation':
            return <SettingsAutomationTab />;
        case 'api_server':
            return <SettingsApiServerTab />;
        case 'llm_service':
            return <SettingsLLMServiceTab isActive={isActive} onOpenProviderDetails={onOpenLlmProviderDetails} />;
        case 'shortcuts':
            return <SettingsShortcutsTab />;
        case 'about':
            return <SettingsAboutTab />;
        default:
            return null;
    }
}

function addMountedSettingsTab(current: SettingsTab[], tab: SettingsTab): SettingsTab[] {
    if (current.includes(tab)) {
        return current;
    }

    const next = new Set(current);
    next.add(tab);
    return SETTINGS_TABS.filter((candidate) => next.has(candidate));
}

function requestSettingsFrame(callback: FrameRequestCallback): number {
    if (typeof requestAnimationFrame === 'function') {
        return requestAnimationFrame(callback);
    }

    return window.setTimeout(() => callback(performance.now()), 16);
}

function cancelSettingsFrame(frameId: number): void {
    if (typeof cancelAnimationFrame === 'function') {
        cancelAnimationFrame(frameId);
        return;
    }

    window.clearTimeout(frameId);
}

function SettingsPaneFrame({
    tab,
    isVisible,
    children,
}: {
    tab: SettingsTab;
    isVisible: boolean;
    children: React.ReactNode;
}): React.JSX.Element {
    useEffect(() => {
        if (!isVisible) return;

        markSettingsPerf(`settings.tab.${tab}.commit`, { tab });
        const frameId = requestSettingsFrame(() => {
            markSettingsPerf(`settings.tab.${tab}.raf`, { tab });
        });

        return () => cancelSettingsFrame(frameId);
    }, [isVisible, tab]);

    return (
        <div
            className="settings-pane-frame"
            data-settings-tab-pane={tab}
            hidden={!isVisible}
            aria-hidden={isVisible ? undefined : true}
        >
            {children}
        </div>
    );
}

const SettingsPaneContent = React.memo(function SettingsPaneContent({
    tab,
    isOpen,
    isActive,
    isPrewarming,
    onOpenDiagnostics,
    onOpenLlmProviderDetails,
}: {
    tab: SettingsTab;
    isOpen: boolean;
    isActive: boolean;
    isPrewarming: boolean;
    onOpenDiagnostics?: () => void;
    onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
}): React.JSX.Element {
    return (
        <Suspense fallback={null}>
            {renderSettingsPane(tab, isOpen, isActive, isPrewarming, onOpenDiagnostics, onOpenLlmProviderDetails)}
        </Suspense>
    );
});

/**
 * Modal dialog for application settings.
 *
 * Handles configuration for general settings, model management, and app preferences.
 *
 * @param props Component props.
 * @return The settings modal or null if not closed.
 */
export function Settings({
    isOpen,
    prewarm = false,
    onClose,
    initialTab,
    onOpenDiagnostics,
    onOpenLlmProviderDetails,
}: SettingsProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const modalRef = useRef<HTMLDivElement>(null);
    const scrollContainerRef = useRef<HTMLDivElement>(null);
    const isPrewarming = prewarm && !isOpen;
    const shouldRender = isOpen || isPrewarming;
    const effectiveInitialTab = isPrewarming ? 'general' : initialTab;

    const {
        activeTab,
        setActiveTab,
    } = useSettingsLogic(isOpen, onClose, effectiveInitialTab);
    const renderedTab = isPrewarming ? 'general' : activeTab;
    const [mountedTabs, setMountedTabs] = useState<SettingsTab[]>(['general']);
    const renderedMountedTabs = useMemo(
        () => (shouldRender ? addMountedSettingsTab(mountedTabs, renderedTab) : mountedTabs),
        [mountedTabs, renderedTab, shouldRender],
    );

    const navigateToTab = useCallback((nextTab: typeof SETTINGS_TABS[number]) => {
        markSettingsPerf('settings.tab.click', { tab: nextTab, previousTab: renderedTab });
        setActiveTab(nextTab);
        requestAnimationFrame(() => {
            const btn = document.getElementById(`settings-tab-${nextTab}`);
            btn?.focus();
        });
    }, [renderedTab, setActiveTab]);
    const navigationContextValue = useMemo(() => ({
        activeTab: renderedTab,
        navigateToTab,
    }), [renderedTab, navigateToTab]);

    // Focus management
    useFocusTrap(isOpen, onClose, modalRef);

    // Reset scroll position on active tab change
    useEffect(() => {
        if (!isOpen) return;
        if (scrollContainerRef.current) {
            scrollContainerRef.current.scrollTop = 0;
        }
    }, [activeTab, isOpen]);

    useEffect(() => {
        if (!shouldRender) return;

        let cancelled = false;
        queueMicrotask(() => {
            if (!cancelled) {
                setMountedTabs((current) => addMountedSettingsTab(current, renderedTab));
            }
        });

        return () => {
            cancelled = true;
        };
    }, [renderedTab, shouldRender]);

    useEffect(() => {
        if (!isPrewarming) return;

        let cancelled = false;
        let frameId: number | null = null;
        let nextIndex = 0;
        const tabsToPrewarm = SETTINGS_TABS.filter((tab) => tab !== 'general');

        markSettingsPerf('settings.prewarm.tabs.start');

        const mountNextTab = () => {
            if (cancelled) {
                return;
            }

            const tab = tabsToPrewarm[nextIndex];
            nextIndex += 1;

            if (tab) {
                setMountedTabs((current) => addMountedSettingsTab(current, tab));
            }

            if (nextIndex < tabsToPrewarm.length) {
                frameId = requestSettingsFrame(mountNextTab);
                return;
            }

            frameId = requestSettingsFrame(() => {
                if (!cancelled) {
                    markSettingsPerf('settings.prewarm.tabs.end');
                }
            });
        };

        frameId = requestSettingsFrame(mountNextTab);

        return () => {
            cancelled = true;
            if (frameId !== null) {
                cancelSettingsFrame(frameId);
            }
        };
    }, [isPrewarming]);

    useEffect(() => {
        if (!shouldRender) return;

        const markerPrefix = isPrewarming ? 'settings.prewarm.shell' : 'settings.shell';
        markSettingsPerf(`${markerPrefix}.commit`);
        const frameId = requestAnimationFrame(() => {
            markSettingsPerf(`${markerPrefix}.raf`);
        });

        return () => cancelAnimationFrame(frameId);
    }, [isPrewarming, shouldRender]);

    useEffect(() => {
        if (!isOpen) return;

        const handleGlobalKeyDown = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.key === 'Tab') {
                // If a dialog is open on top of settings, don't switch tabs
                if (useDialogStore.getState().isOpen || useErrorDialogStore.getState().isOpen) {
                    return;
                }

                e.preventDefault();
                const currentIndex = SETTINGS_TABS.indexOf(renderedTab as typeof SETTINGS_TABS[number]);
                const nextIndex = e.shiftKey
                    ? (currentIndex - 1 + SETTINGS_TABS.length) % SETTINGS_TABS.length
                    : (currentIndex + 1) % SETTINGS_TABS.length;

                const nextTab = SETTINGS_TABS[nextIndex];
                navigateToTab(nextTab);
            }
        };

        window.addEventListener('keydown', handleGlobalKeyDown);
        return () => window.removeEventListener('keydown', handleGlobalKeyDown);
    }, [isOpen, renderedTab, navigateToTab]);

    const handleTabKeyDown = (e: React.KeyboardEvent) => {
        const currentIndex = SETTINGS_TABS.indexOf(renderedTab as typeof SETTINGS_TABS[number]);

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

    if (!shouldRender) return null;

    const settingsModal = (
            <div
                ref={modalRef}
                className="settings-modal"
                onClick={isOpen ? (e) => e.stopPropagation() : undefined}
                role={isOpen ? 'dialog' : undefined}
                aria-modal={isOpen ? 'true' : undefined}
                aria-labelledby={isOpen ? 'settings-title' : undefined}
                tabIndex={isOpen ? -1 : undefined}
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
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'general' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="dashboard"
                            label={t('settings.dashboard.title', { defaultValue: 'Dashboard' })}
                            Icon={() => <BarChart3 size={18} />}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'dashboard' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="microphone"
                            label={t('settings.input_device', { defaultValue: 'Input Device' })}
                            Icon={MicIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'microphone' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="subtitle"
                            label={t('settings.subtitle_voice_typing_title', {
                                defaultValue: 'Subtitles & Voice Typing',
                            })}
                            Icon={SubtitleIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'subtitle' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="models"
                            label={t('settings.model_hub')}
                            Icon={ModelIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'models' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="vocabulary"
                            label={t('settings.vocabulary')}
                            Icon={BookIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'vocabulary' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="automation"
                            label={t('settings.automation', { defaultValue: 'Automation' })}
                            Icon={AutomationIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'automation' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="api_server"
                            label={t('settings.api_server.title', { defaultValue: 'API Server' })}
                            Icon={() => <Server size={18} />}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'api_server' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="llm_service"
                            label={t('settings.llm.title')}
                            Icon={RobotIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'llm_service' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="shortcuts"
                            label={t('shortcuts.title')}
                            Icon={KeyboardIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'shortcuts' ? 0 : -1}
                        />
                        <SettingsTabButton
                            id="about"
                            label={t('settings.about')}
                            Icon={InfoIcon}
                            activeTab={renderedTab}
                            setActiveTab={navigateToTab}
                            tabIndex={renderedTab === 'about' ? 0 : -1}
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
                    <div ref={scrollContainerRef} className="settings-content-scroll full-height">
                        <SettingsNavigationProvider value={navigationContextValue}>
                            <div className="settings-pane-host">
                                {renderedMountedTabs.map((tab) => {
                                    const isPaneVisible = isOpen && tab === renderedTab;
                                    const isPanePrewarming = isPrewarming && tab === 'general';
                                    const paneDiagnosticsHandler = tab === 'general' && (isPaneVisible || isPanePrewarming)
                                        ? onOpenDiagnostics
                                        : undefined;

                                    return (
                                        <SettingsPaneFrame
                                            key={tab}
                                            tab={tab}
                                            isVisible={isPaneVisible}
                                        >
                                            <SettingsPaneContent
                                                tab={tab}
                                                isOpen={isPaneVisible}
                                                isActive={isPaneVisible}
                                                isPrewarming={isPanePrewarming}
                                                onOpenDiagnostics={paneDiagnosticsHandler}
                                                onOpenLlmProviderDetails={onOpenLlmProviderDetails}
                                            />
                                        </SettingsPaneFrame>
                                    );
                                })}
                            </div>
                        </SettingsNavigationProvider>
                    </div>

                </div>
            </div>
    );

    if (isPrewarming) {
        return (
            <div
                data-settings-prewarm="true"
                hidden
                aria-hidden="true"
            >
                {settingsModal}
            </div>
        );
    }

    return (
        <div className="settings-overlay" onClick={onClose}>
            {settingsModal}
        </div>
    );
}
