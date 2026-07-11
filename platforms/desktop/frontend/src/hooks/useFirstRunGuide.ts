import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import {
  type DeviceOption,
  listMicrophoneDeviceOptions,
  requestMicrophonePermission,
} from '../services/audioDeviceService';
import {
  downloadRecommendedOnboardingModels,
  getRecommendedOnboardingConfig,
  getRecommendedOnboardingModels,
} from '../services/onboardingService';
import { useConfigStore } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import type { OnboardingStep } from '../types/onboarding';
import { logger } from '../utils/logger';
import { hasRequiredOnboardingModels } from '../utils/onboarding';

export type ModelStepStatus = 'idle' | 'downloading' | 'error';
export type PermissionState = 'idle' | 'granted' | 'denied';

export interface DownloadProgressState {
  percentage: number;
  status: string;
  isFinished?: boolean;
}

function getActiveStepIndex(currentStep: OnboardingStep): number {
  switch (currentStep) {
    case 'microphone':
      return 0;
    case 'models':
      return 1;
    default:
      return 0;
  }
}

function getPreferredMicrophoneId(
  currentValue: string,
  configMicrophoneId: string | undefined,
  options: DeviceOption[],
): string {
  const preferredValue = currentValue || configMicrophoneId || 'default';
  const matchingOption = options.find((option) => option.value === preferredValue);
  if (matchingOption) {
    return matchingOption.value;
  }

  return options[0]?.value || 'default';
}

export function useFirstRunGuide() {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);
  const setMode = useTranscriptRuntimeStore((state) => state.setMode);
  const { isOpen, currentStep, setStep, defer, complete } = useOnboardingStore(
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
  const [selectedMicrophoneId, setSelectedMicrophoneId] = useState(
    config.microphoneId || 'default'
  );
  const [isLoadingDevices, setIsLoadingDevices] = useState(false);
  const [permissionState, setPermissionState] = useState<PermissionState>('idle');
  const [microphoneRefreshToken, setMicrophoneRefreshToken] = useState(0);

  const hasModelsConfigured = hasRequiredOnboardingModels(config);

  const noopClose = useCallback(() => {}, []);

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
        return getPreferredMicrophoneId(currentValue, config.microphoneId, options);
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
    } catch (error) {
      logger.error('[Onboarding] Failed to download recommended models:', error);
      setModelStepStatus('error');
      setModelError(
        error instanceof Error ? error.message : t('first_run.models.error_detail')
      );
    }
  }

  async function handleRetryPermission(): Promise<void> {
    setPermissionState('idle');
    setMicrophoneRefreshToken((currentValue) => currentValue + 1);
  }

  function handleContinueFromMicrophone(): void {
    setConfig({ microphoneId: selectedMicrophoneId });
    setStep('models');
  }

  function handleFinish(): void {
    setMode('live');
    complete();
  }

  function handleBack(): void {
    if (currentStep === 'models') {
      setStep('microphone');
    }
  }

  const activeStepIndex = getActiveStepIndex(currentStep);
  const isMicrophoneReady = permissionState === 'granted' && deviceOptions.length > 0;

  return {
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
    setStep,
  };
}
