import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckIcon, DownloadIcon, WaveformIcon } from './Icons';
import { Dropdown } from './Dropdown';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useConfigStore } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useShallow } from 'zustand/react/shallow';
import {
  DeviceOption,
  listMicrophoneDeviceOptions,
  requestMicrophonePermission,
} from '../services/audioDeviceService';
import {
  downloadRecommendedOnboardingModels,
  getRecommendedOnboardingConfig,
  getRecommendedOnboardingModels,
} from '../services/onboardingService';
import { hasRequiredOnboardingModels } from '../utils/onboarding';
import { logger } from '../utils/logger';

type ModelStepStatus = 'idle' | 'downloading' | 'error';
type PermissionState = 'idle' | 'granted' | 'denied';

interface DownloadProgressState {
  percentage: number;
  status: string;
  isFinished?: boolean;
}

interface OnboardingActionsProps {
  backLabel: string;
  laterLabel: string;
  onBack?: () => void;
  onLater: () => void;
  primaryAction: {
    disabled?: boolean;
    label: React.ReactNode;
    onClick: () => void;
  };
  secondaryActionsDisabled?: boolean;
}

function StepIndicator({
  stepNumber,
  title,
  isActive,
  isComplete,
}: {
  stepNumber: number;
  title: string;
  isActive: boolean;
  isComplete: boolean;
}): React.JSX.Element {
  return (
    <div className={`onboarding-step-chip ${isActive ? 'active' : ''} ${isComplete ? 'complete' : ''}`}>
      <div className="onboarding-step-dot" aria-hidden="true">
        {isComplete ? <CheckIcon /> : <span>{stepNumber}</span>}
      </div>
      <span>{title}</span>
    </div>
  );
}

function OnboardingActions({
  backLabel,
  laterLabel,
  onBack,
  onLater,
  primaryAction,
  secondaryActionsDisabled = false,
}: OnboardingActionsProps): React.JSX.Element {
  return (
    <div className="onboarding-actions">
      <button
        className="btn btn-secondary onboarding-actions-secondary"
        onClick={onLater}
        disabled={secondaryActionsDisabled}
      >
        {laterLabel}
      </button>
      {onBack && (
        <button
          className="btn btn-secondary"
          onClick={onBack}
          disabled={secondaryActionsDisabled}
        >
          {backLabel}
        </button>
      )}
      <button
        className="btn btn-primary"
        onClick={primaryAction.onClick}
        disabled={primaryAction.disabled}
      >
        {primaryAction.label}
      </button>
    </div>
  );
}

/**
 * Blocking first-run onboarding wizard for recommended offline transcription setup.
 */
export function FirstRunGuide(): React.JSX.Element | null {
  const { t } = useTranslation();
  const modalRef = useRef<HTMLDivElement>(null);
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);
  const setMode = useTranscriptRuntimeStore((state) => state.setMode);
  const {
    isOpen,
    currentStep,
    setStep,
    defer,
    complete,
  } = useOnboardingStore(
    useShallow((state) => ({
      isOpen: state.isOpen,
      currentStep: state.currentStep,
      setStep: state.setStep,
      defer: state.defer,
      complete: state.complete,
    }))
  );

  const recommendedModels = useMemo(() => getRecommendedOnboardingModels(), []);
  const [modelStepStatus, setModelStepStatus] = useState<ModelStepStatus>('idle');
  const [modelError, setModelError] = useState('');
  const [downloads, setDownloads] = useState<Record<string, DownloadProgressState>>({});
  const [deviceOptions, setDeviceOptions] = useState<DeviceOption[]>([
    { label: t('settings.mic_auto'), value: 'default' },
  ]);
  const [selectedMicrophoneId, setSelectedMicrophoneId] = useState(config.microphoneId || 'default');
  const [isLoadingDevices, setIsLoadingDevices] = useState(false);
  const [permissionState, setPermissionState] = useState<PermissionState>('idle');
  const [microphoneRefreshToken, setMicrophoneRefreshToken] = useState(0);

  const hasModelsConfigured = hasRequiredOnboardingModels(config);

  // Stable reference for onClose to prevent infinite focus-trap teardown loops
  const noopClose = useCallback(() => {}, []);
  useFocusTrap(isOpen, noopClose, modalRef);

  useEffect(() => {
    if (isOpen) {
      return;
    }

    queueMicrotask(() => {
      setModelStepStatus('idle');
      setModelError('');
      setDownloads({});
      setPermissionState('idle');
      setIsLoadingDevices(false);
      setSelectedMicrophoneId(config.microphoneId || 'default');
    });
  }, [config.microphoneId, isOpen]);

  useEffect(() => {
    if (!isOpen || currentStep !== 'microphone') {
      return;
    }

    let isMounted = true;

    async function prepareMicrophoneStep() {
      setIsLoadingDevices(true);

      const granted = await requestMicrophonePermission();
      if (!isMounted) {
        return;
      }
      setPermissionState(granted ? 'granted' : 'denied');

      const options = await listMicrophoneDeviceOptions(t('settings.mic_auto'));
      if (!isMounted) {
        return;
      }

      setDeviceOptions(options);
      setSelectedMicrophoneId((currentValue) => {
        const preferredValue = currentValue || config.microphoneId || 'default';
        const matchingOption = options.find((option) => option.value === preferredValue);
        return matchingOption ? matchingOption.value : options[0]?.value || 'default';
      });
      setIsLoadingDevices(false);
    }

    prepareMicrophoneStep();

    return () => {
      isMounted = false;
    };
  }, [config.microphoneId, currentStep, isOpen, microphoneRefreshToken, t]);

  async function handleModelDownload(): Promise<void> {
    setModelStepStatus('downloading');
    setModelError('');

    const initialProgress: Record<string, DownloadProgressState> = {};
    recommendedModels.forEach((model) => {
      initialProgress[model.id] = { percentage: 0, status: t('first_run.models.preparing') };
    });
    setDownloads(initialProgress);

    try {
      const paths = await downloadRecommendedOnboardingModels((update) => {
        setDownloads((previousState) => ({
          ...previousState,
          [update.modelId]: {
            percentage: update.percentage,
            status: update.status,
            isFinished: update.isFinished,
          },
        }));
      });

      setConfig(getRecommendedOnboardingConfig(paths));
      setModelStepStatus('idle');
      setStep('microphone');
    } catch (error) {
      logger.error('[Onboarding] Failed to download recommended models:', error);
      setModelStepStatus('error');
      setModelError(
        error instanceof Error ? error.message : t('first_run.models.error_detail'),
      );
    }
  }

  async function handleRetryPermission(): Promise<void> {
    setPermissionState('idle');
    setMicrophoneRefreshToken((currentValue) => currentValue + 1);
  }

  function handleContinueFromWelcome(): void {
    if (hasModelsConfigured) {
      setStep('microphone');
      return;
    }

    setStep('models');
  }

  function handleFinish(): void {
    setConfig({ microphoneId: selectedMicrophoneId });
    setMode('live');
    complete();
  }

  function handleBack(): void {
    if (currentStep === 'microphone') {
      setStep('models');
      return;
    }

    if (currentStep === 'models') {
      setStep('welcome');
    }
  }

  if (!isOpen) {
    return null;
  }

  const activeStepIndex = (() => {
    switch (currentStep) {
      case 'welcome': return 0;
      case 'models': return 1;
      case 'microphone': return 2;
      default: return 0;
    }
  })();

  const isMicrophoneReady = permissionState === 'granted' && deviceOptions.length > 0;

  let areSecondaryActionsDisabled = false;
  switch (currentStep) {
    case 'models':
      areSecondaryActionsDisabled = modelStepStatus === 'downloading';
      break;
    case 'microphone':
      areSecondaryActionsDisabled = isLoadingDevices;
      break;
  }

  return (
    <div className="settings-overlay" style={{ zIndex: 2100 }}>
      <div
        ref={modalRef}
        className="onboarding-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-title"
        tabIndex={-1}
      >
        <div className="onboarding-hero">
          <div className="onboarding-badge">
            <WaveformIcon />
            <span>{t('first_run.badge')}</span>
          </div>
          <h2 id="onboarding-title">{t('first_run.title')}</h2>
          <p>{t('first_run.description')}</p>
          <div className="onboarding-stepper" aria-label={t('first_run.stepper_label')}>
            <StepIndicator
              stepNumber={1}
              title={t('first_run.steps.welcome')}
              isActive={activeStepIndex === 0}
              isComplete={activeStepIndex > 0}
            />
            <StepIndicator
              stepNumber={2}
              title={t('first_run.steps.models')}
              isActive={activeStepIndex === 1}
              isComplete={activeStepIndex > 1}
            />
            <StepIndicator
              stepNumber={3}
              title={t('first_run.steps.microphone')}
              isActive={activeStepIndex === 2}
              isComplete={false}
            />
          </div>
        </div>

        <div className="onboarding-body">
          {currentStep === 'welcome' && (
            <section className="onboarding-panel">
              <div className="onboarding-panel-header">
                <span className="onboarding-eyebrow">{t('first_run.steps.welcome')}</span>
                <h3>{t('first_run.welcome.heading')}</h3>
                <p>{t('first_run.welcome.body')}</p>
              </div>

              <div className="onboarding-checklist" role="list">
                <div className="onboarding-checklist-item" role="listitem">
                  <CheckIcon />
                  <div>
                    <strong>{t('first_run.welcome.fast_title')}</strong>
                    <span>{t('first_run.welcome.fast_body')}</span>
                  </div>
                </div>
                <div className="onboarding-checklist-item" role="listitem">
                  <CheckIcon />
                  <div>
                    <strong>{t('first_run.welcome.private_title')}</strong>
                    <span>{t('first_run.welcome.private_body')}</span>
                  </div>
                </div>
                <div className="onboarding-checklist-item" role="listitem">
                  <CheckIcon />
                  <div>
                    <strong>{t('first_run.welcome.ready_title')}</strong>
                    <span>{t('first_run.welcome.ready_body')}</span>
                  </div>
                </div>
              </div>

              <div className="onboarding-summary-card">
                <div>
                  <span className="onboarding-summary-label">{t('first_run.welcome.recommended_path_label')}</span>
                  <strong>{t('first_run.welcome.recommended_path_value')}</strong>
                </div>
                <div>
                  <span className="onboarding-summary-label">{t('first_run.welcome.download_label')}</span>
                  <strong>{t('first_run.welcome.download_value')}</strong>
                </div>
              </div>

              <OnboardingActions
                backLabel={t('first_run.actions.back')}
                laterLabel={t('first_run.actions.later')}
                onLater={defer}
                primaryAction={{
                  label: t('first_run.actions.continue'),
                  onClick: handleContinueFromWelcome,
                }}
              />
            </section>
          )}

          {currentStep === 'models' && (
            <section className="onboarding-panel">
              <div className="onboarding-panel-header">
                <span className="onboarding-eyebrow">{t('first_run.steps.models')}</span>
                <h3>{t('first_run.models.heading')}</h3>
                <p>{t('first_run.models.body')}</p>
              </div>

              <div className="onboarding-model-list" role="list">
                {recommendedModels.map((model) => {
                  const downloadState = downloads[model.id];
                  const isDone = downloadState?.isFinished;
                  return (
                    <div className="onboarding-model-card" role="listitem" key={model.id}>
                      <div className="onboarding-model-meta">
                        <div>
                          <strong>{model.name}</strong>
                          <span>{t(model.description)}</span>
                        </div>
                        <div className="onboarding-model-badges">
                          <span className="model-tag">{model.size}</span>
                          {model.language && <span className="model-tag">{model.language.toUpperCase()}</span>}
                        </div>
                      </div>

                      {downloadState && (
                        <div className="onboarding-progress-block" aria-live="polite">
                          <div className="onboarding-progress-row">
                            <span>{downloadState.status}</span>
                            <span className={isDone ? 'success-text' : ''}>
                              {isDone ? t('first_run.models.ready') : `${downloadState.percentage}%`}
                            </span>
                          </div>
                          <div className="progress-bar-mini">
                            <div
                              className="progress-fill"
                              style={{ width: `${downloadState.percentage}%` }}
                            />
                          </div>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>

              {modelStepStatus === 'error' && (
                <div className="onboarding-inline-alert onboarding-inline-alert-error" role="alert">
                  <strong>{t('first_run.models.error')}</strong>
                  <span>{modelError || t('first_run.models.error_detail')}</span>
                </div>
              )}

              {(() => {
                let primaryActionLabel;
                if (hasModelsConfigured) {
                  primaryActionLabel = t('first_run.actions.continue');
                } else {
                  let buttonText;
                  if (modelStepStatus === 'error') {
                    buttonText = t('first_run.actions.retry');
                  } else {
                    buttonText = t('first_run.actions.download_recommended');
                  }

                  primaryActionLabel = (
                    <>
                      <DownloadIcon />
                      {buttonText}
                    </>
                  );
                }

                return (
                  <OnboardingActions
                    backLabel={t('first_run.actions.back')}
                    laterLabel={t('first_run.actions.later')}
                    onBack={handleBack}
                    onLater={defer}
                    primaryAction={{
                      disabled: modelStepStatus === 'downloading',
                      label: primaryActionLabel,
                      onClick: hasModelsConfigured ? () => setStep('microphone') : handleModelDownload,
                    }}
                    secondaryActionsDisabled={areSecondaryActionsDisabled}
                  />
                );
              })()}
            </section>
          )}

          {currentStep === 'microphone' && (
            <section className="onboarding-panel">
              <div className="onboarding-panel-header">
                <span className="onboarding-eyebrow">{t('first_run.steps.microphone')}</span>
                <h3>{t('first_run.microphone.heading')}</h3>
                <p>{t('first_run.microphone.body')}</p>
              </div>

              <div className="onboarding-summary-card">
                <div>
                  <span className="onboarding-summary-label">{t('first_run.microphone.default_source_label')}</span>
                  <strong>{t('first_run.microphone.default_source_value')}</strong>
                </div>
                <div>
                  <span className="onboarding-summary-label">{t('first_run.microphone.device_label')}</span>
                  <strong>{selectedMicrophoneId === 'default'
                    ? t('settings.mic_auto')
                    : selectedMicrophoneId}
                  </strong>
                </div>
              </div>

              <div className="settings-item" style={{ marginBottom: 'var(--spacing-lg)' }}>
                <label className="settings-label" htmlFor="onboarding-microphone-select">
                  {t('settings.microphone_selection')}
                </label>
                <Dropdown
                  id="onboarding-microphone-select"
                  value={selectedMicrophoneId}
                  onChange={(value) => setSelectedMicrophoneId(value)}
                  options={deviceOptions}
                  style={{ width: '100%' }}
                />
                <div className="settings-hint">
                  {t('first_run.microphone.device_hint')}
                </div>
              </div>

              {isLoadingDevices && (
                <div className="onboarding-inline-alert" aria-live="polite">
                  <strong>{t('first_run.microphone.loading_title')}</strong>
                  <span>{t('first_run.microphone.loading_body')}</span>
                </div>
              )}

              {permissionState === 'denied' && !isLoadingDevices && (
                <div className="onboarding-inline-alert onboarding-inline-alert-error" role="alert">
                  <strong>{t('first_run.microphone.permission_title')}</strong>
                  <span>{t('first_run.microphone.permission_body')}</span>
                </div>
              )}

              <OnboardingActions
                backLabel={t('first_run.actions.back')}
                laterLabel={t('first_run.actions.later')}
                onBack={handleBack}
                onLater={defer}
                primaryAction={{
                  disabled: isLoadingDevices || (permissionState !== 'denied' && !isMicrophoneReady),
                  label: permissionState === 'denied'
                    ? t('first_run.actions.retry_permission')
                    : t('first_run.actions.finish'),
                  onClick: permissionState === 'denied' ? handleRetryPermission : handleFinish,
                }}
                secondaryActionsDisabled={areSecondaryActionsDisabled}
              />
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
