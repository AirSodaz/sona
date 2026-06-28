import React, { lazy, Suspense, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Type } from 'lucide-react';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptWorkbench } from './components/transcript/TranscriptWorkbench';
import { BatchImport } from './components/BatchImport';
import { LiveRecord } from './components/LiveRecord';
import { ProjectsView } from './components/ProjectsView';
import { GlobalDialog } from './components/GlobalDialog';
import { ErrorDialog } from './components/ErrorDialog';
import { FirstRunGuide } from './components/FirstRunGuide';
import { NotificationCenter } from './components/NotificationCenter';
// import { LiveCaptionOverlay } from './components/LiveCaptionOverlay';
import { useProjectStore } from './stores/projectStore';
import { useTranscriptPlaybackStore } from './stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from './stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from './stores/transcriptSessionStore';
import { useOnboardingStore } from './stores/onboardingStore';
import { useBatchQueueStore } from './stores/batchQueueStore';
import { AutomationIcon, SettingsIcon } from './components/Icons';
import { useAppInitialization } from './hooks/useAppInitialization';
import { useAutoSaveTranscript } from './hooks/useAutoSaveTranscript';
import { useAutoUpdateCheck } from './hooks/useAutoUpdateCheck';
import { useTrayHandling } from './hooks/useTrayHandling';
import { useTranscriptionServiceSync } from './hooks/useTranscriptionServiceSync';
import { SettingsTab } from './hooks/useSettingsLogic';
import { preloadAllSettingsTabs, preloadSettingsTab } from './components/settings/settingsLoaders';
import { diagnosticsService } from './services/diagnosticsService';
import { clearActiveTranscriptSession } from './stores/transcriptCoordinator';
import { getSettingsPerfErrorDetail, markSettingsPerf } from './utils/settingsPerf';
import { useLlmAssistantConfig, useSetConfig } from './stores/configStore';
import { buildLlmConfigPatch, createLlmSettings } from './services/llm/state';
import type { LlmProvider } from './types/transcript';

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
function getPanelTitle(mode: string, t: (key: string) => string): string {
  switch (mode) {
    case 'live':
      return t('panel.live_record');
    case 'projects':
      return t('panel.projects');
    case 'batch':
    default:
      return t('panel.batch_import');
  }
}

function App(): React.JSX.Element {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [shouldPrewarmSettings, setShouldPrewarmSettings] = useState(false);
  const [isDiagnosticsOpen, setIsDiagnosticsOpen] = useState(false);
  const [isRecoveryCenterOpen, setIsRecoveryCenterOpen] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>('general');
  const [activePanelModal, setActivePanelModal] = useState<ActivePanelModal>(null);
  const mode = useTranscriptRuntimeStore((state) => state.mode);
  const setMode = useTranscriptRuntimeStore((state) => state.setMode);
  const isProjectsMode = mode === 'projects';
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const segmentsLength = useTranscriptSessionStore((state) => state.segments.length);
  const audioUrl = useTranscriptPlaybackStore((state) => state.audioUrl);
  const reopenOnboarding = useOnboardingStore((state) => state.reopen);

  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => 
    state.projects.find((p) => p.id === activeProjectId) || null
  );

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

  const setPreloadedSettingsInitialTab = useCallback((tab: SettingsTab) => {
    preloadSettings(tab);
    setSettingsInitialTab(tab);
  }, [preloadSettings]);

  // Handle tray events
  useTrayHandling(setIsSettingsOpen, setPreloadedSettingsInitialTab);

  useEffect(() => {
    if (!isLoaded) return;

    preloadAllSettings();
  }, [isLoaded, preloadAllSettings]);

  const openDefaultSettings = useCallback(() => {
    markSettingsPerf('settings.open.default.click', { tab: 'general', source: 'header' });
    preloadSettings('general');
    setSettingsInitialTab('general');
    setIsSettingsOpen(true);
  }, [preloadSettings]);

  const openSettingsTab = useCallback((tab: SettingsTab) => {
    markSettingsPerf('settings.open.tab.click', { tab });
    preloadSettings(tab);
    setActivePanelModal((current) => (current?.origin === 'settings' ? null : current));
    setSettingsInitialTab(tab);
    setIsSettingsOpen(true);
  }, [preloadSettings]);

  const openDiagnostics = useCallback(() => {
    const origin = isSettingsOpen ? 'settings' : 'standalone';
    setActivePanelModal({
      kind: 'diagnostics',
      origin,
    });
    if (origin === 'standalone') {
      setIsDiagnosticsOpen(true);
    }
  }, [isSettingsOpen]);

  const closeDiagnostics = useCallback(() => {
    setIsDiagnosticsOpen(false);
    setActivePanelModal((current) => (current?.kind === 'diagnostics' ? null : current));
  }, []);

  const openProviderDetailsFromSettings = useCallback((provider?: LlmProvider) => {
    const nextProvider = provider ?? llmConfig.llmSettings?.activeProvider ?? 'open_ai';
    setActivePanelModal({
      kind: 'provider_details',
      origin: 'settings',
      provider: nextProvider,
    });
  }, [llmConfig.llmSettings?.activeProvider]);

  const handlePanelBack = useCallback(() => {
    setActivePanelModal(null);
  }, []);

  const openRecoveryCenter = useCallback(() => {
    setIsRecoveryCenterOpen(true);
  }, []);

  const openAutomationSettings = useCallback(() => {
    openSettingsTab('automation');
  }, [openSettingsTab]);

  const openVoiceTypingSettings = useCallback(() => {
    openSettingsTab('voice_typing');
  }, [openSettingsTab]);

  const runFirstRunSetupFromDiagnostics = useCallback(() => {
    setIsDiagnosticsOpen(false);
    reopenOnboarding(diagnosticsService.getResumeOnboardingStep(), 'startup');
  }, [reopenOnboarding]);

  const closeTranscriptSession = useCallback(() => {
    if (mode === 'batch') {
      useBatchQueueStore.getState().setActiveItem(null);
    } else {
      clearActiveTranscriptSession({ clearAudio: true });
    }
  }, [mode]);

  if (!isLoaded) {
    return <></>; // Wait for config and onboarding state to load
  }

  const panelTitle = getPanelTitle(mode, t);
  const hasActiveTranscript = Boolean(sourceHistoryId || segmentsLength > 0 || audioUrl);
  const shouldShowTranscriptHost = !isProjectsMode || hasActiveTranscript;
  const appMainClassName = [
    'app-main',
    isProjectsMode ? 'app-main--projects' : 'app-main--workspace',
    shouldShowTranscriptHost ? 'app-main--with-transcript' : 'app-main--without-transcript',
  ].join(' ');

  return (
    <div className="app">
      <a href="#main-content" className="skip-link">{t('common.skip_to_content')}</a>
      {/* Header */}
      <header className="app-header">
        <div className="app-logo">
          <h1>Sona</h1>
          {activeProject && (
            <span 
              className="current-project-tag" 
              onClick={() => setMode('projects')}
              title={t('projects.open_projects', { defaultValue: 'Open Workspace' })}
            >
              {activeProject.name}
            </span>
          )}
        </div>

        <TabNavigation />

        <div className="header-actions">
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
        <div className="projects-mode-shell" style={{ display: isProjectsMode ? undefined : 'none' }}>
          <ProjectsView isActive={isProjectsMode} />
        </div>
        <div className="workspace-mode-shell" style={{ display: !isProjectsMode ? undefined : 'none' }}>
          <div className="panel-container">
            {/* Left Panel - Input */}
            <div className="panel panel-left">
              <div className="panel-header">
                <h2>{panelTitle}</h2>
                {mode === 'batch' && (
                  <button
                    type="button"
                    className="btn btn-icon projects-rail-create"
                    onClick={openAutomationSettings}
                    aria-label={t('automation.open_settings', { defaultValue: 'Open Automation' })}
                    data-tooltip={t('automation.open_settings', { defaultValue: 'Open Automation' })}
                    data-tooltip-pos="bottom"
                  >
                    <AutomationIcon width={18} height={18} />
                  </button>
                )}
                {mode === 'live' && (
                  <button
                    type="button"
                    className="btn btn-icon projects-rail-create"
                    onClick={openVoiceTypingSettings}
                    aria-label={t('voice_typing.open_settings', { defaultValue: 'Open Voice Typing' })}
                    data-tooltip={t('voice_typing.open_settings', { defaultValue: 'Open Voice Typing' })}
                    data-tooltip-pos="bottom"
                  >
                    <Type size={18} aria-hidden="true" />
                  </button>
                )}
              </div>
              <div className="panel-content">
                <div style={{ display: mode === 'live' ? undefined : 'none', height: '100%' }}>
                  <LiveRecord />
                </div>
                {mode === 'batch' && <BatchImport />}
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
            origin={activePanelModal?.kind === 'diagnostics' ? activePanelModal.origin : 'standalone'}
            onBack={activePanelModal?.kind === 'diagnostics' && activePanelModal.origin === 'settings' ? handlePanelBack : undefined}
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
