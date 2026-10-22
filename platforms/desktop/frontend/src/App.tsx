import type React from 'react';
import { lazy, Suspense, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CreateWorkspacePanel } from './components/CreateWorkspacePanel';
import { ErrorDialog } from './components/ErrorDialog';
import { FirstRunGuide } from './components/FirstRunGuide';
import { GlobalDialog } from './components/GlobalDialog';
import { SettingsIcon } from './components/Icons';
import { NotificationCenter } from './components/NotificationCenter';
import { ProjectSelectorDropdown } from './components/ProjectSelectorDropdown';
import { ProjectsView } from './components/ProjectsView';
import { SyncHeaderPill } from './components/SyncHeaderPill';
import { preloadAllSettingsTabs, preloadSettingsTab } from './components/settings/settingsLoaders';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptionOptions } from './components/TranscriptionOptions';
import { ToolsView } from './components/tools/ToolsView';
import { TranscriptWorkbench } from './components/transcript/TranscriptWorkbench';
import { useAgentControlSync } from './hooks/useAgentControlSync';
import { useAppInitialization } from './hooks/useAppInitialization';
import { useAutoSaveTranscript } from './hooks/useAutoSaveTranscript';
import { useAutoUpdateCheck } from './hooks/useAutoUpdateCheck';
import { useNumberStepperHoverEffect } from './hooks/useNumberStepperHoverEffect';
import { useTranscriptionServiceSync } from './hooks/useTranscriptionServiceSync';
import { useTrayHandling } from './hooks/useTrayHandling';
import { diagnosticsService } from './services/diagnosticsService';
import { buildLlmConfigPatch, createLlmSettings } from './services/llm/state';
import { useAutomationStore } from './stores/automationStore';
import { useBatchQueueStore } from './stores/batchQueueStore';
import { useLlmAssistantConfig, useSetConfig } from './stores/configStore';
import { useOnboardingStore } from './stores/onboardingStore';
import { clearActiveTranscriptSession } from './stores/transcriptCoordinator';
// import { LiveCaptionOverlay } from './components/LiveCaptionOverlay';
import { useTranscriptPlaybackStore } from './stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from './stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from './stores/transcriptSessionStore';
import type { SettingsTab } from './types/settings';
import type { LlmProvider } from './types/transcript';
import { getSettingsPerfErrorDetail, markSettingsPerf } from './utils/settingsPerf';

let settingsModulePromise: Promise<typeof import('./components/Settings')> | null = null;

function loadSettingsModule() {
  if (!settingsModulePromise) {
    settingsModulePromise = import('./components/Settings');
  }

  return settingsModulePromise;
}

const SettingsModal = lazy(async () => {
  const module = await loadSettingsModule();
  return { default: module.Settings };
});

const DiagnosticsPanel = lazy(async () => {
  const module = await import('./components/DiagnosticsModal');
  return { default: module.DiagnosticsModal };
});

const RecoveryCenterPanel = lazy(async () => {
  const module = await import('./components/RecoveryCenterModal');
  return { default: module.RecoveryCenterModal };
});

const ProviderDetailsPanel = lazy(async () => {
  const module = await import('./components/settings/llm/ProviderDetailsModal');
  return { default: module.ProviderDetailsModal };
});

type ActivePanelModal =
  | { kind: 'diagnostics'; origin: 'settings' | 'standalone' }
  | { kind: 'provider_details'; origin: 'settings'; provider: LlmProvider }
  | null;

/**
 * Helper to determine the title of the left panel based on the current mode.
 */
function getPanelTitle(
  mode: string,
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  switch (mode) {
    case 'create':
      return t('panel.create', { defaultValue: '新建' });
    case 'live':
      return t('panel.live_record');
    case 'projects':
      return t('panel.projects');
    case 'tools':
      return t('panel.tools', { defaultValue: 'Tools' });
    case 'batch':
    default:
      return t('panel.batch_import');
  }
}

function App(): React.JSX.Element | null {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [shouldPrewarmSettings, setShouldPrewarmSettings] = useState(false);
  const [isDiagnosticsOpen, setIsDiagnosticsOpen] = useState(false);
  const [isRecoveryCenterOpen, setIsRecoveryCenterOpen] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>('general');
  const [activePanelModal, setActivePanelModal] = useState<ActivePanelModal>(null);
  const mode = useTranscriptRuntimeStore((state) => state.mode);
  const setMode = useTranscriptRuntimeStore((state) => state.setMode);
  const isProjectsMode = mode === 'projects';
  const ingestionFocus = useTranscriptRuntimeStore((state) => state.ingestionFocus);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const segmentsLength = useTranscriptSessionStore((state) => state.segments.length);
  const audioUrl = useTranscriptPlaybackStore((state) => state.audioUrl);
  const reopenOnboarding = useOnboardingStore((state) => state.reopen);

  const { t } = useTranslation();
  const llmConfig = useLlmAssistantConfig();
  const updateConfig = useSetConfig();

  // Run application initialization logic
  const { isLoaded } = useAppInitialization();

  // Initialize auto-save hook
  useAutoSaveTranscript();

  // Run shared updater auto-check once per session when enabled
  useAutoUpdateCheck(isLoaded);

  // Keep TranscriptionService synced and preloaded in background
  useTranscriptionServiceSync();

  useNumberStepperHoverEffect();

  const preloadSettings = useCallback((tab: SettingsTab = 'general') => {
    void preloadSettingsTab(tab);
  }, []);

  const preloadAllSettings = useCallback(() => {
    markSettingsPerf('settings.preload.all.start');
    void loadSettingsModule()
      .then(() => preloadAllSettingsTabs())
      .then(() => {
        markSettingsPerf('settings.preload.all.end');
        markSettingsPerf('settings.prewarm.hidden.request');
        setShouldPrewarmSettings(true);
      })
      .catch((error) => {
        markSettingsPerf('settings.preload.all.fail', getSettingsPerfErrorDetail(error));
      });
  }, []);

  const setPreloadedSettingsInitialTab = useCallback(
    (tab: SettingsTab) => {
      preloadSettings(tab);
      setSettingsInitialTab(tab);
    },
    [preloadSettings]
  );

  // Handle tray events
  useTrayHandling(setIsSettingsOpen, setPreloadedSettingsInitialTab);

  // Synchronize background Agent / MCP lifecycle events
  useAgentControlSync();
  useEffect(() => {
    if (!isLoaded) return;

    preloadAllSettings();
  }, [isLoaded, preloadAllSettings]);

  const openDefaultSettings = () => {
    markSettingsPerf('settings.open.default.click', { tab: 'general', source: 'header' });
    preloadSettings('general');
    setSettingsInitialTab('general');
    setIsSettingsOpen(true);
  };

  const openSettingsTab = (tab: SettingsTab) => {
    markSettingsPerf('settings.open.tab.click', { tab });
    preloadSettings(tab);
    setActivePanelModal((current) => (current?.origin === 'settings' ? null : current));
    setSettingsInitialTab(tab);
    setIsSettingsOpen(true);
  };

  const openDiagnostics = () => {
    const origin = isSettingsOpen ? 'settings' : 'standalone';
    setActivePanelModal({
      kind: 'diagnostics',
      origin,
    });
    if (origin === 'standalone') {
      setIsDiagnosticsOpen(true);
    }
  };

  const closeDiagnostics = () => {
    setIsDiagnosticsOpen(false);
    setActivePanelModal((current) => (current?.kind === 'diagnostics' ? null : current));
  };

  const openProviderDetailsFromSettings = (provider?: LlmProvider) => {
    const nextProvider = provider ?? llmConfig.llmSettings?.activeProvider ?? 'open_ai';
    setActivePanelModal({
      kind: 'provider_details',
      origin: 'settings',
      provider: nextProvider,
    });
  };

  const handlePanelBack = () => {
    setActivePanelModal(null);
  };

  const openRecoveryCenter = () => {
    setIsRecoveryCenterOpen(true);
  };

  const openAutomationSettings = () => {
    useAutomationStore.getState().setFocusTagId(null);
    openSettingsTab('automation');
  };

  const openAutomationSettingsForTag = (tagId: string) => {
    useAutomationStore.getState().setFocusTagId(tagId);
    openSettingsTab('automation');
  };

  const runFirstRunSetupFromDiagnostics = () => {
    setIsDiagnosticsOpen(false);
    reopenOnboarding(diagnosticsService.getResumeOnboardingStep(), 'startup');
  };

  const closeTranscriptSession = () => {
    if (mode === 'create' && ingestionFocus === 'batch') {
      useBatchQueueStore.getState().setActiveItem(null);
    } else {
      clearActiveTranscriptSession({ clearAudio: true });
    }
  };

  if (!isLoaded) {
    return null; // Wait for config and onboarding state to load
  }

  const panelTitle = getPanelTitle(mode, t);
  const isToolsMode = mode === 'tools';
  const hasActiveTranscript = Boolean(sourceHistoryId || segmentsLength > 0 || audioUrl);
  const shouldShowTranscriptHost = isProjectsMode ? hasActiveTranscript : !isToolsMode;
  const appMainClassName = [
    'app-main',
    isProjectsMode ? 'app-main--projects' : isToolsMode ? 'app-main--tools' : 'app-main--workspace',
    shouldShowTranscriptHost ? 'app-main--with-transcript' : 'app-main--without-transcript',
  ].join(' ');

  return (
    <div className="app">
      <a href="#main-content" className="skip-link">
        {t('common.skip_to_content')}
      </a>
      {/* Header */}
      <header className="app-header">
        <div className="app-logo">
          <img src="/sona.svg" alt="Sona" className="app-logo-img" width={28} height={28} />
          <ProjectSelectorDropdown onOpenProjects={() => setMode('projects')} />
        </div>

        <TabNavigation />

        <div className="header-actions">
          <SyncHeaderPill onOpenSyncSettings={() => openSettingsTab('sync')} />
          <NotificationCenter
            onOpenRecoveryCenter={openRecoveryCenter}
            onOpenAutomationSettings={openAutomationSettings}
          />
          <button
            className="btn btn-icon"
            onClick={openDefaultSettings}
            onPointerEnter={() => preloadSettings('general')}
            onFocus={() => preloadSettings('general')}
            data-tooltip={t('header.settings')}
            data-tooltip-pos="bottom-left"
            aria-label={t('header.settings')}
          >
            <SettingsIcon />
          </button>
        </div>
      </header>

      {/* Main Content */}
      <main id="main-content" className={appMainClassName}>
        <div
          className="projects-mode-shell"
          style={{ display: isProjectsMode ? undefined : 'none' }}
        >
          <ProjectsView
            isActive={isProjectsMode}
            onOpenAutomationSettings={openAutomationSettingsForTag}
          />
        </div>
        <div className="tools-mode-shell" style={{ display: isToolsMode ? undefined : 'none' }}>
          <ToolsView isActive={isToolsMode} />
        </div>
        <div
          className="workspace-mode-shell"
          style={{ display: !isProjectsMode && !isToolsMode ? undefined : 'none' }}
        >
          <div className="panel-container">
            {/* Left Panel - Input */}
            <div className="panel panel-left">
              <div className="panel-header">
                <h2>{panelTitle}</h2>
              </div>
              <div className="panel-content">
                <CreateWorkspacePanel />
              </div>
              <div className="panel-footer">
                <TranscriptionOptions
                  surface={ingestionFocus}
                  disabled={ingestionFocus === 'live' && isRecording}
                />
              </div>
            </div>

            {/* Right Panel - Editor */}
            <div className="panel panel-right" aria-hidden="true" />
          </div>
        </div>
        <div
          className={`persistent-transcript-host projects-detail-pane ${shouldShowTranscriptHost ? '' : 'is-hidden'}`}
          aria-hidden={shouldShowTranscriptHost ? undefined : true}
        >
          <TranscriptWorkbench onClose={closeTranscriptSession} />
        </div>
      </main>

      {/* Live Caption Overlay - rendered at app level to survive tab switches */}
      {/* {isCaptionMode && isRecording && <LiveCaptionOverlay />} */}

      {/* Settings Modal */}
      {isSettingsOpen || shouldPrewarmSettings ? (
        <Suspense fallback={null}>
          <SettingsModal
            isOpen={isSettingsOpen}
            prewarm={shouldPrewarmSettings && !isSettingsOpen}
            onClose={() => setIsSettingsOpen(false)}
            initialTab={isSettingsOpen ? settingsInitialTab : 'general'}
            onOpenDiagnostics={openDiagnostics}
            onOpenLlmProviderDetails={openProviderDetailsFromSettings}
          />
        </Suspense>
      ) : null}
      {isDiagnosticsOpen || activePanelModal?.kind === 'diagnostics' ? (
        <Suspense fallback={null}>
          <DiagnosticsPanel
            isOpen={isDiagnosticsOpen || activePanelModal?.kind === 'diagnostics'}
            origin={
              activePanelModal?.kind === 'diagnostics' ? activePanelModal.origin : 'standalone'
            }
            onBack={
              activePanelModal?.kind === 'diagnostics' && activePanelModal.origin === 'settings'
                ? handlePanelBack
                : undefined
            }
            onClose={closeDiagnostics}
            onOpenSettingsTab={openSettingsTab}
            onRunFirstRunSetup={runFirstRunSetupFromDiagnostics}
          />
        </Suspense>
      ) : null}
      {activePanelModal?.kind === 'provider_details' ? (
        <Suspense fallback={null}>
          <ProviderDetailsPanel
            provider={activePanelModal.provider}
            config={llmConfig}
            isOpen={true}
            origin="settings"
            onBack={handlePanelBack}
            onClose={() => setActivePanelModal(null)}
            applyLlmSettings={(nextLlmSettings) => {
              updateConfig(buildLlmConfigPatch(nextLlmSettings ?? createLlmSettings()));
            }}
            t={(key) => t(key)}
          />
        </Suspense>
      ) : null}
      {isRecoveryCenterOpen ? (
        <Suspense fallback={null}>
          <RecoveryCenterPanel
            isOpen={isRecoveryCenterOpen}
            onClose={() => setIsRecoveryCenterOpen(false)}
          />
        </Suspense>
      ) : null}
      <GlobalDialog />
      <ErrorDialog />
      <FirstRunGuide />
    </div>
  );
}

export default App;
