import React, { lazy, Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { TabNavigation } from './components/TabNavigation';
import { TranscriptWorkbench } from './components/TranscriptWorkbench';
import { BatchImport } from './components/BatchImport';
import { LiveRecord } from './components/LiveRecord';
import { ProjectsView } from './components/ProjectsView';
import { GlobalDialog } from './components/GlobalDialog';
import { ErrorDialog } from './components/ErrorDialog';
import { FirstRunGuide } from './components/FirstRunGuide';
import { NotificationCenter } from './components/NotificationCenter';
import { OnboardingReminderBanner } from './components/OnboardingReminderBanner';
// import { LiveCaptionOverlay } from './components/LiveCaptionOverlay';
import { useProjectStore } from './stores/projectStore';
import { useTranscriptRuntimeStore } from './stores/transcriptRuntimeStore';
import { useOnboardingStore } from './stores/onboardingStore';
import { AutomationIcon, SettingsIcon } from './components/Icons';
import { useAppInitialization } from './hooks/useAppInitialization';
import { useAutoSaveTranscript } from './hooks/useAutoSaveTranscript';
import { useAutoUpdateCheck } from './hooks/useAutoUpdateCheck';
import { useTrayHandling } from './hooks/useTrayHandling';
import { useTranscriptionServiceSync } from './hooks/useTranscriptionServiceSync';
import { SettingsTab } from './hooks/useSettingsLogic';
import { diagnosticsService } from './services/diagnosticsService';
import { clearTranscriptSegments } from './stores/transcriptCoordinator';

const SettingsModal = lazy(async () => {
  const module = await import('./components/Settings');
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
  const [isDiagnosticsOpen, setIsDiagnosticsOpen] = useState(false);
  const [isRecoveryCenterOpen, setIsRecoveryCenterOpen] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>('general');
  const mode = useTranscriptRuntimeStore((state) => state.mode);
  const setMode = useTranscriptRuntimeStore((state) => state.setMode);
  const reopenOnboarding = useOnboardingStore((state) => state.reopen);

  const activeProjectId = useProjectStore((state) => state.activeProjectId);
  const activeProject = useProjectStore((state) => 
    state.projects.find((p) => p.id === activeProjectId) || null
  );

  const { t } = useTranslation();

  // Run application initialization logic
  const { isLoaded } = useAppInitialization();

  // Initialize auto-save hook
  useAutoSaveTranscript();

  // Run shared updater auto-check once per session when enabled
  useAutoUpdateCheck(isLoaded);

  // Keep TranscriptionService synced and preloaded in background
  useTranscriptionServiceSync();

  // Handle tray events
  useTrayHandling(setIsSettingsOpen, setSettingsInitialTab);

  const openSettingsTab = (tab: SettingsTab) => {
    setIsDiagnosticsOpen(false);
    setSettingsInitialTab(tab);
    setIsSettingsOpen(true);
  };

  const openDiagnostics = () => {
    setIsSettingsOpen(false);
    setIsDiagnosticsOpen(true);
  };

  const openRecoveryCenter = () => {
    setIsRecoveryCenterOpen(true);
  };

  const openAutomationSettings = () => {
    openSettingsTab('automation');
  };

  const runFirstRunSetupFromDiagnostics = () => {
    setIsDiagnosticsOpen(false);
    reopenOnboarding(diagnosticsService.getResumeOnboardingStep(), 'startup');
  };

  if (!isLoaded) {
    return <></>; // Wait for config and onboarding state to load
  }

  const panelTitle = getPanelTitle(mode, t);
  const isProjectsMode = mode === 'projects';

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
            onClick={() => setIsSettingsOpen(true)}
            data-tooltip={t('header.settings')}
            data-tooltip-pos="bottom-left"
            aria-label={t('header.settings')}
          >
            <SettingsIcon />
          </button>
        </div>
      </header>

      <OnboardingReminderBanner />

      {/* Main Content */}
      <main id="main-content" className="app-main">
        <div className="projects-mode-shell" style={{ display: isProjectsMode ? undefined : 'none' }}>
          <ProjectsView />
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
              </div>
              <div className="panel-content">
                <div style={{ display: mode === 'live' ? undefined : 'none', height: '100%' }}>
                  <LiveRecord />
                </div>
                {mode === 'batch' && <BatchImport />}
              </div>
            </div>

            {/* Right Panel - Editor */}
            <div className="panel panel-right">
              <TranscriptWorkbench onClose={clearTranscriptSegments} />
            </div>
          </div>
        </div>
      </main>

      {/* Live Caption Overlay - rendered at app level to survive tab switches */}
      {/* {isCaptionMode && isRecording && <LiveCaptionOverlay />} */}

      {/* Settings Modal */}
      {isSettingsOpen ? (
        <Suspense fallback={null}>
          <SettingsModal
            isOpen={isSettingsOpen}
            onClose={() => setIsSettingsOpen(false)}
            initialTab={settingsInitialTab}
            onOpenDiagnostics={openDiagnostics}
          />
        </Suspense>
      ) : null}
      {isDiagnosticsOpen ? (
        <Suspense fallback={null}>
          <DiagnosticsPanel
            isOpen={isDiagnosticsOpen}
            onClose={() => setIsDiagnosticsOpen(false)}
            onOpenSettingsTab={openSettingsTab}
            onRunFirstRunSetup={runFirstRunSetupFromDiagnostics}
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
