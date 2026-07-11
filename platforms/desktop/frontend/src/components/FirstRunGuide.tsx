import React from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';
import { CheckIcon, DownloadIcon } from './Icons';
import { Modal } from './Modal';
import { useFirstRunGuide, type ModelStepStatus } from '../hooks/useFirstRunGuide';
import type { OnboardingStep } from '../types/onboarding';

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

type OnboardingTranslate = (key: string) => string;



function getSecondaryActionsDisabled(
  currentStep: OnboardingStep,
  modelStepStatus: ModelStepStatus,
  isLoadingDevices: boolean,
): boolean {
  switch (currentStep) {
    case 'models':
      return modelStepStatus === 'downloading';
    case 'microphone':
      return isLoadingDevices;
    default:
      return false;
  }
}



function getSelectedMicrophoneLabel(
  selectedMicrophoneId: string,
  defaultMicrophoneLabel: string,
): string {
  if (selectedMicrophoneId === 'default') {
    return defaultMicrophoneLabel;
  }

  return selectedMicrophoneId;
}

function getModelActionButtonText(
  modelStepStatus: ModelStepStatus,
  t: OnboardingTranslate,
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
  if (hasModelsConfigured) {
    return t('first_run.actions.finish');
  }

  return (
    <>
      <DownloadIcon />
      {getModelActionButtonText(modelStepStatus, t)}
    </>
  );
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
    <>
      <button
        className="btn btn-secondary"
        style={{ marginInlineEnd: 'auto' }}
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
    </>
  );
}

/**
 * Blocking first-run onboarding wizard for recommended offline transcription setup.
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
    activeStepIndex,
    isMicrophoneReady,
    noopClose,
    handleModelDownload,
    handleRetryPermission,
    handleContinueFromMicrophone,
    handleFinish,
    handleBack,
  } = useFirstRunGuide();

  if (!isOpen) {
    return null;
  }

  const areSecondaryActionsDisabled = getSecondaryActionsDisabled(
    currentStep,
    modelStepStatus,
    isLoadingDevices,
  );
  const modelPrimaryActionLabel = getModelPrimaryActionLabel({
    hasModelsConfigured,
    modelStepStatus,
    t,
  });
  const selectedMicrophoneLabel = getSelectedMicrophoneLabel(
    selectedMicrophoneId,
    t('settings.mic_auto'),
  );

  let footer: React.ReactNode = null;
  if (currentStep === 'microphone') {
    footer = (
      <OnboardingActions
        backLabel={t('first_run.actions.back')}
        laterLabel={t('first_run.actions.later')}
        onLater={defer}
        primaryAction={{
          disabled: isLoadingDevices || (permissionState !== 'denied' && !isMicrophoneReady),
          label: permissionState === 'denied'
            ? t('first_run.actions.retry_permission')
            : t('first_run.actions.continue'),
          onClick: permissionState === 'denied' ? handleRetryPermission : handleContinueFromMicrophone,
        }}
        secondaryActionsDisabled={areSecondaryActionsDisabled}
      />
    );
  } else if (currentStep === 'models') {
    footer = (
      <OnboardingActions
        backLabel={t('first_run.actions.back')}
        laterLabel={t('first_run.actions.later')}
        onBack={handleBack}
        onLater={defer}
        primaryAction={{
          disabled: modelStepStatus === 'downloading',
          label: modelPrimaryActionLabel,
          onClick: hasModelsConfigured ? handleFinish : handleModelDownload,
        }}
        secondaryActionsDisabled={areSecondaryActionsDisabled}
      />
    );
  }

  return (
    <Modal
      isOpen={isOpen}
      onClose={noopClose}
      closeOnEsc={false}
      closeOnOverlayClick={false}
      hideCloseButton
      size="lg"
      className="onboarding-modal"
      title={
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', whiteSpace: 'nowrap' }}>
          <img src="/sona.svg" alt="" width={22} height={22} />
          <span>{t('first_run.title')}</span>
        </div>
      }
      footer={footer}
    >
      <div className="onboarding-body">
        <p style={{ color: 'var(--color-text-secondary)', marginBottom: 'var(--spacing-lg)' }}>
          {t('first_run.description')}
        </p>
        <div className="onboarding-stepper" aria-label={t('first_run.stepper_label')} style={{ marginBottom: 'var(--spacing-xl)' }}>
          <StepIndicator
            stepNumber={1}
            title={t('first_run.steps.microphone')}
            isActive={activeStepIndex === 0}
            isComplete={activeStepIndex > 0}
          />
          <StepIndicator
            stepNumber={2}
            title={t('first_run.steps.models')}
            isActive={activeStepIndex === 1}
            isComplete={activeStepIndex > 1}
          />
        </div>

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
                <strong>{selectedMicrophoneLabel}</strong>
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
                      <div className="onboarding-model-name-row">
                        <strong>{model.name}</strong>
                        <div className="onboarding-model-badges">
                          <span className="model-tag">{model.size}</span>
                          {model.language && <span className="model-tag">{model.language.toUpperCase()}</span>}
                        </div>
                      </div>
                      <span>{t(model.description)}</span>
                    </div>

                    {downloadState && (
                      <div className="progress-container-mini">
                        <div className="progress-info-mini" aria-live="polite">
                          <span style={{ color: 'var(--color-text-muted)', fontSize: '0.75rem' }}>{downloadState.status}</span>
                          <span className={isDone ? 'success-text' : ''}>
                            {isDone ? t('first_run.models.ready') : `${Math.round(downloadState.percentage)}%`}
                          </span>
                        </div>
                        <div
                          className="progress-bar-mini"
                          role="progressbar"
                          aria-valuenow={Math.round(downloadState.percentage)}
                          aria-valuemin={0}
                          aria-valuemax={100}
                          aria-label={`${t('common.download')} ${model.name}`}
                        >
                          <div className="progress-fill" style={{ width: `${downloadState.percentage}%` }} />
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
          </section>
        )}
      </div>
    </Modal>
  );
}
