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
  hydrateDiagnosticsCoreSnapshot,
  type Translate,
} from './diagnosticsSnapshotBuilders';

export const diagnosticsService = {
  async collectSnapshot(t: Translate): Promise<DiagnosticsSnapshot> {
    const config = useConfigStore.getState().config;
    const voiceTypingRuntime = useVoiceTypingRuntimeStore.getState();
    const [permissionState, microphoneProbe, systemAudioProbe] = await Promise.all([
      getMicrophonePermissionState(),
      probeMicrophoneDeviceOptions(t('settings.mic_auto')),
      probeSystemAudioDeviceOptions(t('settings.mic_auto')),
    ]);

    const voiceTypingReadiness = resolveVoiceTypingReadinessSnapshot(
      {
        voiceTypingEnabled: config.voiceTypingEnabled ?? false,
        voiceTypingShortcut: config.voiceTypingShortcut ?? '',
        streamingModelPath: config.streamingModelPath ?? '',
        vadModelPath: config.vadModelPath ?? '',
        microphoneId: config.microphoneId ?? 'default',
      },
      voiceTypingRuntime,
    );

    const coreSnapshot = await getDiagnosticsCoreSnapshot({
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

    return hydrateDiagnosticsCoreSnapshot(t, coreSnapshot);
  },

  getResumeOnboardingStep() {
    const config = useConfigStore.getState().config;
    const state = useOnboardingStore.getState().persistedState;
    return getResumeOnboardingStep(config, 'startup', state);
  },
};
