import { getResumeOnboardingStep } from '../utils/onboarding';
import {
  getMicrophonePermissionState,
  probeMicrophoneDeviceOptions,
  probeSystemAudioDeviceOptions,
} from './audioDeviceService';
import { useConfigStore } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import { resolveVoiceTypingReadinessSnapshot } from '../hooks/useVoiceTypingReadiness';
import type {
  DiagnosticsSnapshot,
} from '../types/diagnostics';
import { getDiagnosticsCoreSnapshot } from './tauri/app';
import {
  buildDiagnosticsSnapshot,
  type Translate,
} from './diagnosticsSnapshotBuilders';

export interface DiagnosticsServicePorts {
  useConfigStore: typeof useConfigStore;
  useOnboardingStore: typeof useOnboardingStore;
  useVoiceTypingRuntimeStore: typeof useVoiceTypingRuntimeStore;
  getMicrophonePermissionState: typeof getMicrophonePermissionState;
  probeMicrophoneDeviceOptions: typeof probeMicrophoneDeviceOptions;
  probeSystemAudioDeviceOptions: typeof probeSystemAudioDeviceOptions;
  resolveVoiceTypingReadinessSnapshot: typeof resolveVoiceTypingReadinessSnapshot;
  getDiagnosticsCoreSnapshot: typeof getDiagnosticsCoreSnapshot;
  buildDiagnosticsSnapshot: typeof buildDiagnosticsSnapshot;
  getResumeOnboardingStep: typeof getResumeOnboardingStep;
}

export class DiagnosticsService {
  constructor(private readonly ports: DiagnosticsServicePorts) {}

  collectSnapshot = async (t: Translate): Promise<DiagnosticsSnapshot> => {
    const config = this.ports.useConfigStore.getState().config;
    const voiceTypingRuntime = this.ports.useVoiceTypingRuntimeStore.getState();
    const [permissionState, microphoneProbe, systemAudioProbe] = await Promise.all([
      this.ports.getMicrophonePermissionState(),
      this.ports.probeMicrophoneDeviceOptions(t('settings.mic_auto')),
      this.ports.probeSystemAudioDeviceOptions(t('settings.mic_auto')),
    ]);

    const voiceTypingReadiness = this.ports.resolveVoiceTypingReadinessSnapshot(
      {
        voiceTypingEnabled: config.voiceTypingEnabled ?? false,
        voiceTypingShortcut: config.voiceTypingShortcut ?? '',
        streamingModelPath: config.streamingModelPath ?? '',
        vadModelPath: config.vadModelPath ?? '',
        microphoneId: config.microphoneId ?? 'default',
      },
      voiceTypingRuntime,
    );

    const coreSnapshot = await this.ports.getDiagnosticsCoreSnapshot({
      config: {
        streamingModelPath: config.streamingModelPath,
        offlineModelPath: config.offlineModelPath,
        vadModelPath: config.vadModelPath ?? '',
        punctuationModelPath: config.punctuationModelPath ?? '',
        microphoneId: config.microphoneId ?? 'default',
      },
      permissionState,
      microphoneProbe,
      systemAudioProbe,
      voiceTypingReadiness,
    });

    return this.ports.buildDiagnosticsSnapshot(t, coreSnapshot);
  }

  getResumeOnboardingStep = () => {
    const config = this.ports.useConfigStore.getState().config;
    const state = this.ports.useOnboardingStore.getState().persistedState;
    return this.ports.getResumeOnboardingStep(config, 'startup', state);
  }
}

export function createDiagnosticsService(ports: DiagnosticsServicePorts): DiagnosticsService {
  return new DiagnosticsService(ports);
}

export const diagnosticsService = createDiagnosticsService({
  useConfigStore,
  useOnboardingStore,
  useVoiceTypingRuntimeStore,
  getMicrophonePermissionState,
  probeMicrophoneDeviceOptions,
  probeSystemAudioDeviceOptions,
  resolveVoiceTypingReadinessSnapshot,
  getDiagnosticsCoreSnapshot,
  buildDiagnosticsSnapshot,
  getResumeOnboardingStep,
});
