import type React from 'react';
import { useTranslation } from 'react-i18next';
import { type ModelStepStatus, useFirstRunGuide } from '../hooks/useFirstRunGuide';
import type { OnboardingStep } from '../types/onboarding';
import { Dropdown } from './Dropdown';
import { CheckIcon, DownloadIcon } from './Icons';
import { LanguageBadges } from './LanguageBadges';

type OnboardingTranslate = (key: string) => string;

function getSecondaryActionsDisabled(
  currentStep: OnboardingStep,
  isLoadingDevices: boolean
): boolean {
  switch (currentStep) {
    case 'microphone':
      return isLoadingDevices;
    default:
      return false;
  }
}

function getSelectedMicrophoneLabel(
  selectedMicrophoneId: string,
  defaultMicrophoneLabel: string
): string {
  if (selectedMicrophoneId === 'default') {
    return defaultMicrophoneLabel;
  }

  return selectedMicrophoneId;
}

function getModelActionButtonText(
  modelStepStatus: ModelStepStatus,
  t: OnboardingTranslate
): string {
  if (modelStepStatus === 'error') {
    return t('first_run.actions.retry');
  }

  return t('first_run.actions.download_recommended');
}

function getModelPrimaryActionLabel({
  hasModelsConfigured,
  modelStepStatus,
  t,
}: {
  hasModelsConfigured: boolean;
  modelStepStatus: ModelStepStatus;
  t: OnboardingTranslate;
}): React.ReactNode {
  if (hasModelsConfigured || modelStepStatus === 'downloading') {
    return t('first_run.actions.continue');
  }

  return (
    <>
      <DownloadIcon />
      {getModelActionButtonText(modelStepStatus, t)}
    </>
  );
}

const STEPS: OnboardingStep[] = ['models', 'microphone'];

function StepNav({
  currentStep,
  t,
}: {
  currentStep: OnboardingStep;
  t: OnboardingTranslate;
}): React.JSX.Element {
  const activeIndex = STEPS.indexOf(currentStep);

  return (
    <nav className="welcome-steps" aria-label={t('first_run.stepper_label')}>
      {STEPS.map((step, index) => {
        const isActive = index === activeIndex;
        const isComplete = index < activeIndex;
        const state = isActive ? 'active' : isComplete ? 'complete' : 'upcoming';

        return (
          <div
            key={step}
            className={`welcome-step-item welcome-step-${state}`}
            aria-current={isActive ? 'step' : undefined}
          >
            <div className="welcome-step-indicator" aria-hidden="true">
              {isComplete ? <CheckIcon /> : <span>{index + 1}</span>}
            </div>
            <div className="welcome-step-text">
              <span className="welcome-step-label">{t(`first_run.steps.${step}`)}</span>
            </div>
          </div>
        );
      })}
    </nav>
  );
}

/**
 * Blocking first-run onboarding wizard for recommended offline transcription setup.
 * Full-screen split layout: branded hero panel on the left, step content on the right.
 */
export function FirstRunGuide(): React.JSX.Element | null {
  const { t } = useTranslation();

  const {
    isOpen,
    currentStep,
    defer,
    recommendedModels,
    modelStepStatus,
    modelError,
    downloads,
    deviceOptions,
    selectedMicrophoneId,
    setSelectedMicrophoneId,
    isLoadingDevices,
    permissionState,
    hasModelsConfigured,
    isMicrophoneReady,
    handleModelDownload,
    handleRetryPermission,
    handleContinueFromModels,
    handleFinish,
    handleBack,
  } = useFirstRunGuide();

  if (!isOpen) {
    return null;
  }

  const areSecondaryActionsDisabled = getSecondaryActionsDisabled(currentStep, isLoadingDevices);

  const modelPrimaryActionLabel = getModelPrimaryActionLabel({
    hasModelsConfigured,
    modelStepStatus,
    t,
  });

  const selectedMicrophoneLabel = getSelectedMicrophoneLabel(
    selectedMicrophoneId,
    t('settings.mic_auto')
  );

  return (
    <div className="welcome-screen" role="dialog" aria-label={t('first_run.title')}>
      {/* Left: Brand panel */}
      <div className="welcome-brand">
        <div className="welcome-brand-content">
          <div className="welcome-brand-logo">
            <img src="/sona.svg" alt="" width={48} height={48} />
          </div>
          <h1 className="welcome-brand-title">{t('first_run.title')}</h1>
          <p className="welcome-brand-subtitle">{t('first_run.description')}</p>
        </div>
        <div className="welcome-brand-motif" aria-hidden="true">
          <svg viewBox="0 0 200 400" fill="none">
            <path
              d="M144 50 C90 50 20 70 20 135 C20 205 144 185 144 255 C144 325 80 355 24 325"
              stroke="currentColor"
              strokeWidth="20"
              strokeLinecap="round"
              strokeLinejoin="round"
              opacity="0.07"
            />
          </svg>
        </div>
      </div>

      {/* Right: Content panel */}
      <div className="welcome-content">
        <div className="welcome-content-inner">
          <StepNav currentStep={currentStep} t={t} />

          <div className="welcome-panel-wrapper">
            {currentStep === 'microphone' && (
              <section className="welcome-panel" key="microphone">
                <div className="welcome-panel-header">
                  <h2>{t('first_run.microphone.heading')}</h2>
                  <p>{t('first_run.microphone.body')}</p>
                </div>

                <div className="welcome-summary-card">
                  <div>
                    <span className="welcome-summary-label">
                      {t('first_run.microphone.default_source_label')}
                    </span>
                    <strong>{t('first_run.microphone.default_source_value')}</strong>
                  </div>
                  <div>
                    <span className="welcome-summary-label">
                      {t('first_run.microphone.device_label')}
                    </span>
                    <strong>{selectedMicrophoneLabel}</strong>
                  </div>
                </div>

                <div className="welcome-field">
                  <label className="welcome-field-label" htmlFor="onboarding-microphone-select">
                    {t('settings.microphone_selection')}
                  </label>
                  <Dropdown
                    id="onboarding-microphone-select"
                    value={selectedMicrophoneId}
                    onChange={(value) => setSelectedMicrophoneId(value)}
                    options={deviceOptions}
                    style={{ width: '100%' }}
                  />
                  <div className="welcome-field-hint">{t('first_run.microphone.device_hint')}</div>
                </div>

                {isLoadingDevices && (
                  <div className="welcome-alert" aria-live="polite">
                    <strong>{t('first_run.microphone.loading_title')}</strong>
                    <span>{t('first_run.microphone.loading_body')}</span>
                  </div>
                )}

                {permissionState === 'denied' && !isLoadingDevices && (
                  <div className="welcome-alert welcome-alert-error" role="alert">
                    <strong>{t('first_run.microphone.permission_title')}</strong>
                    <span>{t('first_run.microphone.permission_body')}</span>
                  </div>
                )}
              </section>
            )}

            {currentStep === 'models' && (
              <section className="welcome-panel" key="models">
                <div className="welcome-panel-header">
                  <h2>{t('first_run.models.heading')}</h2>
                  <p>{t('first_run.models.body')}</p>
                </div>

                <div className="welcome-model-list" role="list">
                  {recommendedModels.map((model) => {
                    const downloadState = downloads[model.id];
                    const isDone = downloadState?.isFinished;
                    return (
                      <div className="welcome-model-card" role="listitem" key={model.id}>
                        <div className="welcome-model-meta">
                          <div className="welcome-model-name-row">
                            <strong>{model.name}</strong>
                            <div className="welcome-model-badges">
                              <span className="model-tag">{model.size}</span>
                              <LanguageBadges languages={model.languages} />
                            </div>
                          </div>
                          <span>{t(model.description)}</span>
                        </div>

                        {downloadState && (
                          <div className="welcome-progress">
                            <div className="welcome-progress-info" aria-live="polite">
                              <span className="welcome-progress-status">
                                {downloadState.status}
                              </span>
                              <span className={isDone ? 'welcome-progress-done' : ''}>
                                {isDone
                                  ? t('first_run.models.ready')
                                  : `${Math.round(downloadState.percentage)}%`}
                              </span>
                            </div>
                            <div
                              className="welcome-progress-bar"
                              role="progressbar"
                              aria-valuenow={Math.round(downloadState.percentage)}
                              aria-valuemin={0}
                              aria-valuemax={100}
                              aria-label={`${t('common.download')} ${model.name}`}
                            >
                              <div
                                className="welcome-progress-fill"
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
                  <div className="welcome-alert welcome-alert-error" role="alert">
                    <strong>{t('first_run.models.error')}</strong>
                    <span>{modelError || t('first_run.models.error_detail')}</span>
                  </div>
                )}
              </section>
            )}
          </div>
        </div>

        {/* Footer actions */}
        <div className="welcome-footer">
          <button
            className="btn btn-secondary"
            style={{ marginInlineEnd: 'auto' }}
            onClick={defer}
            disabled={areSecondaryActionsDisabled}
          >
            {t('first_run.actions.later')}
          </button>
          {currentStep === 'microphone' && (
            <button
              className="btn btn-secondary"
              onClick={handleBack}
              disabled={areSecondaryActionsDisabled}
            >
              {t('first_run.actions.back')}
            </button>
          )}
          <button
            className="btn btn-primary"
            onClick={
              currentStep === 'models'
                ? hasModelsConfigured || modelStepStatus === 'downloading'
                  ? handleContinueFromModels
                  : handleModelDownload
                : permissionState === 'denied'
                  ? handleRetryPermission
                  : handleFinish
            }
            disabled={
              currentStep === 'models'
                ? false
                : isLoadingDevices || (permissionState !== 'denied' && !isMicrophoneReady)
            }
          >
            {currentStep === 'models'
              ? modelPrimaryActionLabel
              : permissionState === 'denied'
                ? t('first_run.actions.retry_permission')
                : t('first_run.actions.finish')}
          </button>
        </div>
      </div>
    </div>
  );
}
