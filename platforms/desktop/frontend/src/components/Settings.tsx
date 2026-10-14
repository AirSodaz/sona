import { BarChart3, Bot, Cloud, HardDrive, Server } from 'lucide-react';
import type React from 'react';
import { Fragment, lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { isAnyDialogOpen } from '../stores/dialogStore';
import type { SettingsTab, SettingsTabInput } from '../types/settings';
import { markSettingsPerf } from '../utils/settingsPerf';
import { SettingsNavigationProvider } from './settings/SettingsNavigationContext';
import { SettingsSidebarGroupHeader, SettingsTabButton } from './settings/SettingsTabButton';
import {
  loadSettingsAboutTab,
  loadSettingsApiServerTab,
  loadSettingsAutomationTab,
  loadSettingsDashboardTab,
  loadSettingsGeneralTab,
  loadSettingsLLMServiceTab,
  loadSettingsMcpTab,
  loadSettingsMicrophoneTab,
  loadSettingsModelsPane,
  loadSettingsShortcutsTab,
  loadSettingsStorageTab,
  loadSettingsSubtitleTab,
  loadSettingsSyncTab,
  loadSettingsVocabularyTab,
  SETTINGS_TAB_GROUPS,
  SETTINGS_TABS,
} from './settings/settingsLoaders';
import './settings/Settings.css';
import type { LlmProvider } from '../types/transcript';
import {
  AutomationIcon,
  BookIcon,
  GeneralIcon,
  InfoIcon,
  KeyboardIcon,
  MicIcon,
  ModelIcon,
  RobotIcon,
  SubtitleIcon,
  XIcon,
} from './Icons';

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
const SettingsStorageTab = lazy(loadSettingsStorageTab);
const SettingsApiServerTab = lazy(loadSettingsApiServerTab);
const SettingsMcpTab = lazy(loadSettingsMcpTab);
const SettingsSyncTab = lazy(loadSettingsSyncTab);
const SettingsLLMServiceTab = lazy(loadSettingsLLMServiceTab);
const SettingsShortcutsTab = lazy(loadSettingsShortcutsTab);
const SettingsAboutTab = lazy(loadSettingsAboutTab);

const SETTINGS_TAB_META: Record<
  SettingsTab,
  { labelKey: string; defaultValue?: string; Icon: () => React.JSX.Element }
> = {
  general: { labelKey: 'settings.general', Icon: GeneralIcon },
  dashboard: {
    labelKey: 'settings.dashboard.title',
    defaultValue: 'Dashboard',
    Icon: () => <BarChart3 size={18} />,
  },
  shortcuts: { labelKey: 'shortcuts.title', Icon: KeyboardIcon },
  about: { labelKey: 'settings.about', Icon: InfoIcon },
  microphone: { labelKey: 'settings.input_device', defaultValue: 'Input Device', Icon: MicIcon },
  models: { labelKey: 'settings.model_hub', Icon: ModelIcon },
  subtitle: {
    labelKey: 'settings.subtitle_voice_typing_title',
    defaultValue: 'Subtitles & Voice Typing',
    Icon: SubtitleIcon,
  },
  vocabulary: { labelKey: 'settings.vocabulary', Icon: BookIcon },
  llm_service: { labelKey: 'settings.llm.title', Icon: RobotIcon },
  automation: { labelKey: 'settings.automation', defaultValue: 'Automation', Icon: AutomationIcon },
  storage: {
    labelKey: 'settings.storage.title',
    defaultValue: 'Storage',
    Icon: () => <HardDrive size={18} />,
  },
  sync: {
    labelKey: 'settings.sync.title',
    defaultValue: 'Cloud Sync',
    Icon: () => <Cloud size={18} />,
  },
  api_server: {
    labelKey: 'settings.api_server.title',
    defaultValue: 'API Server',
    Icon: () => <Server size={18} />,
  },
  mcp: {
    labelKey: 'settings.mcp.title',
    defaultValue: 'MCP Agents',
    Icon: () => <Bot size={18} />,
  },
};

function renderSettingsPane(
  activeTab: SettingsTab,
  isOpen: boolean,
  isActive: boolean,
  isPrewarming = false,
  onOpenDiagnostics?: () => void,
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void,
  initialTab?: SettingsTabInput
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
      return <SettingsMicrophoneTab isActiveTab={isActive} isOpen={isOpen} />;
    case 'subtitle':
      return (
        <SettingsSubtitleTab
          initialSubTab={initialTab === 'voice_typing' ? 'voice_typing' : undefined}
        />
      );
    case 'models':
      return <SettingsModelsPane isOpen={isOpen} isActive={isActive} />;
    case 'vocabulary':
      return (
        <SettingsVocabularyTab initialSubTab={initialTab === 'context' ? 'prompts' : undefined} />
      );
    case 'automation':
      return <SettingsAutomationTab />;
    case 'storage':
      return <SettingsStorageTab />;
    case 'sync':
      return <SettingsSyncTab isVisible={isOpen} isPrewarming={isPrewarming} />;
    case 'api_server':
      return <SettingsApiServerTab />;
    case 'mcp':
      return <SettingsMcpTab />;
    case 'llm_service':
      return (
        <SettingsLLMServiceTab
          isActive={isActive}
          onOpenProviderDetails={onOpenLlmProviderDetails}
        />
      );
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

function SettingsPaneContent({
  tab,
  isOpen,
  isActive,
  isPrewarming,
  onOpenDiagnostics,
  onOpenLlmProviderDetails,
  initialTab,
}: {
  tab: SettingsTab;
  isOpen: boolean;
  isActive: boolean;
  isPrewarming: boolean;
  onOpenDiagnostics?: () => void;
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
  initialTab?: SettingsTabInput;
}): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      {renderSettingsPane(
        tab,
        isOpen,
        isActive,
        isPrewarming,
        onOpenDiagnostics,
        onOpenLlmProviderDetails,
        initialTab
      )}
    </Suspense>
  );
}

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
  const [isSidebarScrolled, setIsSidebarScrolled] = useState(false);
  const isPrewarming = prewarm && !isOpen;
  const shouldRender = isOpen || isPrewarming;
  const effectiveInitialTab = isPrewarming ? 'general' : initialTab;

  const { activeTab, setActiveTab } = useSettingsLogic(isOpen, onClose, effectiveInitialTab);
  const renderedTab = isPrewarming ? 'general' : activeTab;
  const [mountedTabs, setMountedTabs] = useState<SettingsTab[]>(['general']);
  const renderedMountedTabs = useMemo(
    () => (shouldRender ? addMountedSettingsTab(mountedTabs, renderedTab) : mountedTabs),
    [mountedTabs, renderedTab, shouldRender]
  );

  const navigateToTab = useCallback(
    (nextTab: (typeof SETTINGS_TABS)[number]) => {
      markSettingsPerf('settings.tab.click', { tab: nextTab, previousTab: renderedTab });
      setActiveTab(nextTab);
      requestAnimationFrame(() => {
        const btn = document.getElementById(`settings-tab-${nextTab}`);
        btn?.focus();
      });
    },
    [renderedTab, setActiveTab]
  );

  const handleSidebarScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const scrolled = e.currentTarget.scrollTop > 4;
    setIsSidebarScrolled((prev) => (prev !== scrolled ? scrolled : prev));
  }, []);
  const navigationContextValue = {
    activeTab: renderedTab,
    navigateToTab,
  };

  // Focus management
  useFocusTrap(isOpen, onClose, modalRef);

  // Reset scroll position on active tab change
  useEffect(() => {
    if (!isOpen) return;
    if (activeTab && scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = 0;
    }
  }, [activeTab, isOpen]);
  // Ensure the active tab button in the sidebar is visible inside the scroll container
  useEffect(() => {
    if (!isOpen) return;
    const frameId = requestAnimationFrame(() => {
      const btn = document.getElementById(`settings-tab-${renderedTab}`);
      btn?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
    });
    return () => cancelAnimationFrame(frameId);
  }, [isOpen, renderedTab]);

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
        if (isAnyDialogOpen()) {
          return;
        }

        e.preventDefault();
        const currentIndex = SETTINGS_TABS.indexOf(renderedTab as (typeof SETTINGS_TABS)[number]);
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
    const currentIndex = SETTINGS_TABS.indexOf(renderedTab as (typeof SETTINGS_TABS)[number]);

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
        <div className={`settings-sidebar-header ${isSidebarScrolled ? 'is-scrolled' : ''}`}>
          <h2 id="settings-title">{t('settings.title')}</h2>
        </div>

        <div
          className="settings-tabs-container"
          role="tablist"
          aria-orientation="vertical"
          onKeyDown={handleTabKeyDown}
          onScroll={handleSidebarScroll}
        >
          {SETTINGS_TAB_GROUPS.map((group, gi) => (
            <Fragment key={group.labelKey}>
              <SettingsSidebarGroupHeader label={t(group.labelKey)} first={gi === 0} />
              {group.tabs.map((tabId) => {
                const meta = SETTINGS_TAB_META[tabId];
                return (
                  <SettingsTabButton
                    key={tabId}
                    id={tabId}
                    label={t(meta.labelKey, { defaultValue: meta.defaultValue ?? meta.labelKey })}
                    Icon={meta.Icon}
                    activeTab={renderedTab}
                    setActiveTab={navigateToTab}
                    tabIndex={renderedTab === tabId ? 0 : -1}
                  />
                );
              })}
            </Fragment>
          ))}
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
                const paneDiagnosticsHandler =
                  tab === 'general' && (isPaneVisible || isPanePrewarming)
                    ? onOpenDiagnostics
                    : undefined;

                return (
                  <SettingsPaneFrame key={tab} tab={tab} isVisible={isPaneVisible}>
                    <SettingsPaneContent
                      tab={tab}
                      isOpen={isPaneVisible}
                      isActive={isPaneVisible}
                      isPrewarming={isPanePrewarming}
                      onOpenDiagnostics={paneDiagnosticsHandler}
                      onOpenLlmProviderDetails={onOpenLlmProviderDetails}
                      initialTab={initialTab}
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
      <div data-settings-prewarm="true" hidden aria-hidden="true">
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
