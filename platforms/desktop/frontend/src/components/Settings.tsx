import { Search, X } from 'lucide-react';
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
import { XIcon } from './Icons';
import type { SubtitleSubTab } from './settings/SettingsSubtitleTab';
import type { VocabularySubTab } from './settings/SettingsVocabularyTab';
import { SettingsSearchResults } from './settings/search/SettingsSearchResults';
import { type SettingsSearchHit, useSettingsSearch } from './settings/search/useSettingsSearch';
import { SETTINGS_TAB_META } from './settings/settingsTabMeta';

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

function renderSettingsPane(
  activeTab: SettingsTab,
  isOpen: boolean,
  isActive: boolean,
  isPrewarming = false,
  onOpenDiagnostics?: () => void,
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void,
  initialTab?: SettingsTabInput,
  pendingSubTab?: { tab: SettingsTab; subTab: string } | null
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
          initialSubTab={
            pendingSubTab?.tab === 'subtitle'
              ? (pendingSubTab.subTab as SubtitleSubTab)
              : initialTab === 'voice_typing'
                ? 'voice_typing'
                : undefined
          }
        />
      );
    case 'models':
      return <SettingsModelsPane isOpen={isOpen} isActive={isActive} />;
    case 'vocabulary':
      return (
        <SettingsVocabularyTab
          initialSubTab={
            pendingSubTab?.tab === 'vocabulary'
              ? (pendingSubTab.subTab as VocabularySubTab)
              : initialTab === 'context'
                ? 'prompts'
                : undefined
          }
        />
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
  pendingSubTab,
}: {
  tab: SettingsTab;
  isOpen: boolean;
  isActive: boolean;
  isPrewarming: boolean;
  onOpenDiagnostics?: () => void;
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
  initialTab?: SettingsTabInput;
  pendingSubTab?: { tab: SettingsTab; subTab: string } | null;
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
        initialTab,
        pendingSubTab
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
      setPendingSubTab(null);
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
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [pendingSubTab, setPendingSubTab] = useState<{ tab: SettingsTab; subTab: string } | null>(
    null
  );
  const isMac =
    typeof navigator !== 'undefined' &&
    /Mac|iPhone|iPad|iPod/i.test(navigator.platform || navigator.userAgent || '');
  const { hits } = useSettingsSearch(searchQuery);
  const highlightTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const scrollRafRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
      if (highlightTimeoutRef.current) {
        clearTimeout(highlightTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!isOpen) {
      setSearchQuery('');
      setActiveIndex(0);
      setPendingSubTab(null);
    }
  }, [isOpen]);

  const handleSelectResult = useCallback(
    (hit: SettingsSearchHit) => {
      navigateToTab(hit.tab);
      setPendingSubTab(hit.subTab ? { tab: hit.tab, subTab: hit.subTab } : null);

      const targetId = hit.targetId;
      if (!targetId) return;

      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }

      const startTime = performance.now();
      const maxDurationMs = 2000;

      const findAndScroll = () => {
        const el = document.getElementById(targetId);
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'center' });
          el.classList.add('settings-item--highlighted');

          if (highlightTimeoutRef.current) {
            clearTimeout(highlightTimeoutRef.current);
          }
          highlightTimeoutRef.current = setTimeout(() => {
            el.classList.remove('settings-item--highlighted');
          }, 1600);
        } else if (performance.now() - startTime < maxDurationMs) {
          scrollRafRef.current = requestAnimationFrame(findAndScroll);
        }
      };

      scrollRafRef.current = requestAnimationFrame(findAndScroll);
    },
    [navigateToTab]
  );

  const handleSearchInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (hits.length > 0) {
          setActiveIndex((prev) => (prev + 1) % hits.length);
        }
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (hits.length > 0) {
          setActiveIndex((prev) => (prev - 1 + hits.length) % hits.length);
        }
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (hits.length > 0 && hits[activeIndex]) {
          handleSelectResult(hits[activeIndex]);
        }
      } else if (e.key === 'Escape') {
        if (searchQuery.trim().length > 0) {
          e.preventDefault();
          e.stopPropagation();
          setSearchQuery('');
          setActiveIndex(0);
        }
      }
    },
    [activeIndex, handleSelectResult, hits, searchQuery]
  );

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
      // If a dialog is open on top of settings, don't handle shortcuts
      if (isAnyDialogOpen()) {
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        e.stopPropagation();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
        return;
      }

      if (e.ctrlKey && e.key === 'Tab') {
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
      onKeyDown={
        isOpen
          ? (e) => {
              if (e.key === 'Escape' && searchQuery.trim().length > 0 && !isAnyDialogOpen()) {
                e.preventDefault();
                e.stopPropagation();
                setSearchQuery('');
                setActiveIndex(0);
                searchInputRef.current?.focus();
              }
            }
          : undefined
      }
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
          <div className="settings-search-bar">
            <Search size={14} className="settings-search-icon" aria-hidden="true" />
            <input
              ref={searchInputRef}
              type="text"
              role="searchbox"
              className="settings-search-input"
              aria-label={t('settings.search_placeholder', { defaultValue: 'Search settings...' })}
              placeholder={t('settings.search_placeholder', { defaultValue: 'Search settings...' })}
              value={searchQuery}
              onChange={(e) => {
                setSearchQuery(e.target.value);
                setActiveIndex(0);
              }}
              onKeyDown={handleSearchInputKeyDown}
            />
            {searchQuery ? (
              <button
                type="button"
                className="settings-search-clear"
                onClick={() => {
                  setSearchQuery('');
                  setActiveIndex(0);
                  searchInputRef.current?.focus();
                }}
                aria-label={t('common.clear_search', { defaultValue: 'Clear search' })}
              >
                <X size={12} />
              </button>
            ) : (
              <kbd className="settings-search-shortcut">{isMac ? '⌘F' : 'Ctrl F'}</kbd>
            )}
          </div>
        </div>

        {searchQuery.trim().length > 0 ? (
          <SettingsSearchResults
            hits={hits}
            searchQuery={searchQuery}
            activeIndex={activeIndex}
            onSelectResult={handleSelectResult}
            onHoverResult={setActiveIndex}
          />
        ) : (
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
        )}
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
                      pendingSubTab={pendingSubTab}
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
