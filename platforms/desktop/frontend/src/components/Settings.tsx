import type React from 'react';
import { Fragment, lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useSettingsLogic } from '../hooks/useSettingsLogic';
import { isAnyDialogOpen } from '../stores/dialogStore';
import { normalizeSettingsTab, type SettingsTab, type SettingsTabInput } from '../types/settings';
import { markSettingsPerf } from '../utils/settingsPerf';
import { SettingsNavigationProvider } from './settings/SettingsNavigationContext';
import { SettingsSidebarGroupHeader, SettingsTabButton } from './settings/SettingsTabButton';
import {
  loadSettingsAboutTab,
  loadSettingsAutomationTab,
  loadSettingsCapabilitiesPane,
  loadSettingsDashboardTab,
  loadSettingsGeneralTab,
  loadSettingsMicrophoneTab,
  loadSettingsShortcutsTab,
  loadSettingsStorageTab,
  loadSettingsSyncTab,
  loadSettingsVocabularyTab,
  SETTINGS_TAB_GROUPS,
  SETTINGS_TABS,
} from './settings/settingsLoaders';
import './settings/Settings.css';
import type { ToolId } from '../types/tools';
import type { LlmProvider } from '../types/transcript';
import { XIcon } from './Icons';
import { SettingsMigratedBanner } from './settings/SettingsMigratedBanner';
import type { ModelScenario } from './settings/SettingsModelsTab';
import type { SubtitleSubTab } from './settings/SettingsSubtitleTab';
import type { PromptsSubTab, VocabularySubTab } from './settings/SettingsVocabularyTab';
import {
  SettingsSearchSection,
  type SettingsSearchSectionHandle,
} from './settings/search/SettingsSearchSection';
import type { SettingsSearchHit } from './settings/search/useSettingsSearch';
import { SETTINGS_TAB_META } from './settings/settingsTabMeta';

type SettingsNavRequest = {
  requestId: number;
  targetId?: string;
};

type CapabilitiesFocus =
  | 'speech'
  | 'ai_writing'
  | 'translation'
  | 'advanced_speech'
  | 'advanced_ai'
  | ModelScenario;

type SettingsPendingSubTab =
  | (SettingsNavRequest & { tab: 'subtitle'; subTab?: SubtitleSubTab; promptsSubTab?: never })
  | (SettingsNavRequest & {
      tab: 'capabilities';
      subTab?: CapabilitiesFocus;
      promptsSubTab?: never;
    })
  | (SettingsNavRequest & {
      tab: 'vocabulary';
      subTab?: VocabularySubTab;
      promptsSubTab?: PromptsSubTab;
    })
  | (SettingsNavRequest & {
      tab: Exclude<SettingsTab, 'subtitle' | 'capabilities' | 'models' | 'llm_service' | 'vocabulary'>;
      subTab?: never;
      promptsSubTab?: never;
    })
  | null;

function createPendingSubTab(
  hit: SettingsSearchHit,
  requestId: number
): Exclude<SettingsPendingSubTab, null> {
  const base = { requestId, targetId: hit.targetId };
  switch (hit.tab) {
    case 'subtitle':
      return { ...base, tab: 'subtitle', subTab: hit.subTab };
    case 'capabilities':
      return {
        ...base,
        tab: 'capabilities',
        subTab: hit.subTab,
      };
    case 'vocabulary':
      return {
        ...base,
        tab: 'vocabulary',
        subTab: hit.subTab,
        promptsSubTab: hit.promptsSubTab,
      };
    default:
      return { ...base, tab: hit.tab };
  }
}
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
const SettingsCapabilitiesPane = lazy(loadSettingsCapabilitiesPane);
const SettingsVocabularyTab = lazy(loadSettingsVocabularyTab);
const SettingsAutomationTab = lazy(loadSettingsAutomationTab);
const SettingsStorageTab = lazy(loadSettingsStorageTab);
const SettingsSyncTab = lazy(loadSettingsSyncTab);
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
  pendingSubTab?: SettingsPendingSubTab,
  migratedSubtitleTool: ToolId = initialTab === 'voice_typing' ? 'voice_typing' : 'subtitles',
  onClose?: () => void
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
        <SettingsMigratedBanner
          tool={
            pendingSubTab?.tab === 'subtitle' && pendingSubTab.subTab
              ? (pendingSubTab.subTab as ToolId)
              : migratedSubtitleTool
          }
          onClose={onClose}
        />
      );
    case 'capabilities':
    case 'models':
    case 'llm_service':
      return (
        <SettingsCapabilitiesPane
          isOpen={isOpen}
          isActive={isActive}
          initialFocus={
            pendingSubTab?.tab === 'capabilities' ? pendingSubTab.subTab : undefined
          }
          focusRequestId={
            pendingSubTab?.tab === 'capabilities' ? pendingSubTab.requestId : undefined
          }
          onOpenLlmProviderDetails={onOpenLlmProviderDetails}
        />
      );
    case 'vocabulary':
      return (
        <SettingsVocabularyTab
          initialSubTab={
            pendingSubTab?.tab === 'vocabulary'
              ? pendingSubTab.subTab
              : initialTab === 'context'
                ? 'prompts'
                : undefined
          }
          initialPromptsSubTab={
            pendingSubTab?.tab === 'vocabulary' ? pendingSubTab.promptsSubTab : undefined
          }
          subTabRequestId={
            pendingSubTab?.tab === 'vocabulary' ? pendingSubTab.requestId : undefined
          }
        />
      );
    case 'automation':
      return <SettingsAutomationTab />;
    case 'storage':
      return <SettingsStorageTab />;
    case 'sync':
      return (
        <SettingsSyncTab
          isVisible={isOpen}
          isPrewarming={isPrewarming}
          navRequest={pendingSubTab?.tab === 'sync' ? pendingSubTab : undefined}
        />
      );
    case 'api_server':
      return <SettingsMigratedBanner tool="api_server" onClose={onClose} />;
    case 'mcp':
      return <SettingsMigratedBanner tool="mcp" onClose={onClose} />;
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
  const ordered = SETTINGS_TABS.filter((candidate) => next.has(candidate));
  for (const t of next) {
    if (!ordered.includes(t)) {
      ordered.push(t);
    }
  }
  return ordered;
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
  migratedSubtitleTool,
  onClose,
}: {
  tab: SettingsTab;
  isOpen: boolean;
  isActive: boolean;
  isPrewarming: boolean;
  onOpenDiagnostics?: () => void;
  onOpenLlmProviderDetails?: (provider: LlmProvider) => void;
  initialTab?: SettingsTabInput;
  pendingSubTab?: SettingsPendingSubTab;
  migratedSubtitleTool?: ToolId;
  onClose?: () => void;
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
        pendingSubTab,
        migratedSubtitleTool,
        onClose
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
  const isRenderedTabInList = (SETTINGS_TABS as readonly string[]).includes(renderedTab);
  const [prevInitialTab, setPrevInitialTab] = useState(initialTab);
  const [migratedSubtitleTool, setMigratedSubtitleTool] = useState<ToolId>(() =>
    initialTab === 'voice_typing' ? 'voice_typing' : 'subtitles'
  );
  if (prevInitialTab !== initialTab) {
    setPrevInitialTab(initialTab);
    if (initialTab === 'voice_typing') {
      setMigratedSubtitleTool('voice_typing');
    } else if (initialTab === 'subtitle') {
      setMigratedSubtitleTool('subtitles');
    }
  }
  const [mountedTabs, setMountedTabs] = useState<SettingsTab[]>(['general']);
  const renderedMountedTabs = useMemo(
    () => (shouldRender ? addMountedSettingsTab(mountedTabs, renderedTab) : mountedTabs),
    [mountedTabs, renderedTab, shouldRender]
  );

  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchSectionRef = useRef<SettingsSearchSectionHandle>(null);
  const navRequestIdRef = useRef(0);
  const [pendingSubTab, setPendingSubTab] = useState<SettingsPendingSubTab>(null);
  const isMac =
    typeof navigator !== 'undefined' &&
    /Mac|iPhone|iPad|iPod/i.test(navigator.platform || navigator.userAgent || '');
  const highlightTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const highlightedElementRef = useRef<HTMLElement | null>(null);
  const scrollRafRef = useRef<number | null>(null);

  const clearHighlight = useCallback(() => {
    if (highlightTimeoutRef.current) {
      clearTimeout(highlightTimeoutRef.current);
      highlightTimeoutRef.current = null;
    }
    if (highlightedElementRef.current) {
      highlightedElementRef.current.classList.remove('settings-item--highlighted');
      highlightedElementRef.current = null;
    }
    document.querySelectorAll('.settings-item--highlighted').forEach((node) => {
      node.classList.remove('settings-item--highlighted');
    });
  }, []);

  const applyHighlight = useCallback((el: HTMLElement) => {
    if (highlightTimeoutRef.current) {
      clearTimeout(highlightTimeoutRef.current);
      highlightTimeoutRef.current = null;
    }
    const prefersReducedMotion =
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    el.scrollIntoView({ behavior: prefersReducedMotion ? 'auto' : 'smooth', block: 'center' });
    el.classList.add('settings-item--highlighted');
    highlightedElementRef.current = el;
    highlightTimeoutRef.current = setTimeout(() => {
      el.classList.remove('settings-item--highlighted');
      if (highlightedElementRef.current === el) {
        highlightedElementRef.current = null;
      }
      highlightTimeoutRef.current = null;
    }, 1600);
  }, []);

  const navigateToTab = useCallback(
    (nextTab: SettingsTab) => {
      const resolvedTab = normalizeSettingsTab(nextTab);
      markSettingsPerf('settings.tab.click', { tab: resolvedTab, previousTab: renderedTab });
      setActiveTab(resolvedTab);
      setPendingSubTab(null);
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
        scrollRafRef.current = null;
      }
      clearHighlight();
      requestAnimationFrame(() => {
        const btn = document.getElementById(`settings-tab-${resolvedTab}`);
        btn?.focus();
      });
    },
    [clearHighlight, renderedTab, setActiveTab]
  );

  const handleSidebarScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const scrolled = e.currentTarget.scrollTop > 4;
    setIsSidebarScrolled((prev) => (prev !== scrolled ? scrolled : prev));
  }, []);
  const navigationContextValue = useMemo(
    () => ({
      activeTab: renderedTab,
      navigateToTab,
    }),
    [renderedTab, navigateToTab]
  );

  useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
      clearHighlight();
    };
  }, [clearHighlight]);
  useEffect(() => {
    if (!isOpen) {
      setPendingSubTab(null);
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
        scrollRafRef.current = null;
      }
      clearHighlight();
    }
  }, [clearHighlight, isOpen]);

  const handleSelectResult = useCallback(
    (hit: SettingsSearchHit) => {
      if (hit.tab === 'subtitle') {
        setMigratedSubtitleTool((hit.subTab as ToolId) || 'subtitles');
      }
      const resolvedTab = normalizeSettingsTab(hit.tab);
      navigateToTab(resolvedTab);
      navRequestIdRef.current += 1;
      setPendingSubTab(createPendingSubTab(hit, navRequestIdRef.current));

      const targetId = hit.targetId;
      if (!targetId) return;

      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
        scrollRafRef.current = null;
      }

      clearHighlight();

      const startTime = performance.now();
      const maxDurationMs = 2000;

      const findAndScroll = () => {
        const modal = modalRef.current;
        let el: HTMLElement | null = null;
        if (hit.tab === 'subtitle' || hit.tab === 'api_server' || hit.tab === 'mcp') {
          const tool = hit.tab === 'subtitle' ? hit.subTab || 'subtitles' : hit.tab;
          el = modal?.querySelector<HTMLElement>(`#settings-panel-migrated-${tool}`) ?? null;
        } else {
          el = modal?.querySelector<HTMLElement>(`#${targetId}`) ?? null;
          const isSyncDisabledCardPresent =
            hit.tab === 'sync' && Boolean(modal?.querySelector('.sync-disabled-feature-card'));
          if (!el && hit.tab === 'sync' && isSyncDisabledCardPresent) {
            el = modal?.querySelector<HTMLElement>('#settings-sync-toggle') ?? null;
          }
        }
        if (el && !el.closest('[hidden]')) {
          scrollRafRef.current = null;
          applyHighlight(el);
        } else if (performance.now() - startTime < maxDurationMs) {
          scrollRafRef.current = requestAnimationFrame(findAndScroll);
        } else {
          scrollRafRef.current = null;
          const fallbackEl = hit.fallbackTargetId
            ? (modal?.querySelector<HTMLElement>(`#${hit.fallbackTargetId}`) ?? null)
            : null;
          if (fallbackEl && !fallbackEl.closest('[hidden]')) {
            applyHighlight(fallbackEl);
          }
        }
      };

      scrollRafRef.current = requestAnimationFrame(findAndScroll);
    },
    [applyHighlight, clearHighlight, navigateToTab]
  );

  const handleQueryEmptied = useCallback(() => {
    setPendingSubTab(null);
  }, []);

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

      const isSearchShortcut = isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
      if (isSearchShortcut && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'f') {
        const activeElement = document.activeElement;
        const isEditable =
          activeElement instanceof HTMLInputElement ||
          activeElement instanceof HTMLTextAreaElement ||
          (activeElement instanceof HTMLElement && activeElement.isContentEditable);
        if (isEditable && activeElement !== searchInputRef.current) {
          return;
        }
        e.preventDefault();
        searchSectionRef.current?.focusAndSelect();
        return;
      }

      if (e.ctrlKey && e.key === 'Tab') {
        e.preventDefault();
        const currentIndex = SETTINGS_TABS.indexOf(renderedTab as (typeof SETTINGS_TABS)[number]);
        const nextIndex = e.shiftKey
          ? currentIndex <= 0
            ? SETTINGS_TABS.length - 1
            : currentIndex - 1
          : currentIndex === -1
            ? 0
            : (currentIndex + 1) % SETTINGS_TABS.length;

        const nextTab = SETTINGS_TABS[nextIndex];
        navigateToTab(nextTab);
      }
    };

    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown);
  }, [isMac, isOpen, navigateToTab, renderedTab]);

  const handleTabKeyDown = (e: React.KeyboardEvent) => {
    const currentIndex = SETTINGS_TABS.indexOf(renderedTab as (typeof SETTINGS_TABS)[number]);

    let nextIndex = -1;
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        nextIndex = currentIndex === -1 ? 0 : (currentIndex + 1) % SETTINGS_TABS.length;
        break;
      case 'ArrowUp':
        e.preventDefault();
        nextIndex = currentIndex <= 0 ? SETTINGS_TABS.length - 1 : currentIndex - 1;
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
      <div
        className="settings-sidebar"
        onKeyDown={
          isOpen
            ? (e) => {
                if (e.key === 'Escape' && !isAnyDialogOpen()) {
                  if (searchSectionRef.current?.clearIfSearching()) {
                    e.preventDefault();
                    e.stopPropagation();
                  }
                }
              }
            : undefined
        }
      >
        <SettingsSearchSection
          ref={searchSectionRef}
          inputRef={searchInputRef}
          isOpen={isOpen}
          isMac={isMac}
          isSidebarScrolled={isSidebarScrolled}
          onSelectResult={handleSelectResult}
          onQueryEmptied={handleQueryEmptied}
        >
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
                      tabIndex={
                        renderedTab === tabId ||
                        (!isRenderedTabInList && tabId === SETTINGS_TABS[0])
                          ? 0
                          : -1
                      }
                    />
                  );
                })}
              </Fragment>
            ))}
          </div>
        </SettingsSearchSection>
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
                      migratedSubtitleTool={migratedSubtitleTool}
                      onClose={onClose}
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
