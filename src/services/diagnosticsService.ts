import { getResumeOnboardingStep, hasRequiredOnboardingModels } from '../utils/onboarding';
import {
  getMicrophonePermissionState,
  probeMicrophoneDeviceOptions,
  probeSystemAudioDeviceOptions,
} from './audioDeviceService';
import { useConfigStore } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import { findSelectedModelByMode } from '../utils/modelSelection';
import { modelService } from './modelService';
import { getPathStatusMap } from './pathStatusService';
import { resolveVoiceTypingReadinessSnapshot } from '../hooks/useVoiceTypingReadiness';
import type {
  DiagnosticSection,
  DiagnosticsSnapshot,
} from '../types/diagnostics';
import { getRuntimeEnvironmentStatus } from './tauri/app';
import {
  buildInputChecks,
  buildModelChecks,
  buildOverviewCards,
  buildRuntimeChecks,
  type DiagnosticsBuiltChecks,
  type DiagnosticsSnapshotBuildContext,
  type Translate,
} from './diagnosticsSnapshotBuilders';

export const diagnosticsService = {
  async collectSnapshot(t: Translate): Promise<DiagnosticsSnapshot> {
    const config = useConfigStore.getState().config;
    const voiceTypingRuntime = useVoiceTypingRuntimeStore.getState();
    const streamingModelPath = config.streamingModelPath.trim();
    const offlineModelPath = config.offlineModelPath.trim();
    const vadModelPath = (config.vadModelPath || '').trim();
    const punctuationModelPath = (config.punctuationModelPath || '').trim();

    const [permissionState, microphoneProbe, systemAudioProbe, runtimeEnvironment, pathStatusMap] = await Promise.all([
      getMicrophonePermissionState(),
      probeMicrophoneDeviceOptions(t('settings.mic_auto')),
      probeSystemAudioDeviceOptions(t('settings.mic_auto')),
      getRuntimeEnvironmentStatus(),
      getPathStatusMap([streamingModelPath, offlineModelPath, vadModelPath, punctuationModelPath]),
    ]);

    const liveModel = findSelectedModelByMode(config.streamingModelPath, 'streaming');
    const offlineModel = findSelectedModelByMode(config.offlineModelPath, 'offline');
    const liveModelRules = liveModel ? modelService.getModelRules(liveModel.id) : null;
    const offlineModelRules = offlineModel ? modelService.getModelRules(offlineModel.id) : null;
    const liveModelPathStatus = pathStatusMap[streamingModelPath];
    const offlineModelPathStatus = pathStatusMap[offlineModelPath];
    const vadPathStatus = pathStatusMap[vadModelPath];
    const punctuationPathStatus = pathStatusMap[punctuationModelPath];

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

    const context: DiagnosticsSnapshotBuildContext = {
      t,
      config,
      trimmedPaths: {
        streamingModelPath,
        offlineModelPath,
        vadModelPath,
        punctuationModelPath,
      },
      selectedModels: {
        live: liveModel,
        offline: offlineModel,
      },
      modelRules: {
        live: liveModelRules,
        offline: offlineModelRules,
      },
      pathStatuses: {
        liveModel: liveModelPathStatus,
        offlineModel: offlineModelPathStatus,
        vad: vadPathStatus,
        punctuation: punctuationPathStatus,
      },
      permissionState,
      microphoneProbe,
      systemAudioProbe,
      voiceTypingReadiness,
      runtimeEnvironment,
      onboardingReady: hasRequiredOnboardingModels(config),
      punctuationRequired: [liveModelRules, offlineModelRules].some((rules) => rules?.requiresPunctuation ?? false),
    };

    const checks: DiagnosticsBuiltChecks = {
      model: buildModelChecks(context),
      input: buildInputChecks(context),
      runtime: buildRuntimeChecks(context),
    };

    const sections: DiagnosticSection[] = [
      {
        id: 'models',
        title: t('settings.diagnostics.models_section', { defaultValue: 'Models' }),
        description: t('settings.diagnostics.models_section_description', {
          defaultValue: 'Check that local transcription models and required dependencies are present.',
        }),
        checks: [
          checks.model.liveModelCheck,
          checks.model.offlineModelCheck,
          checks.model.vadCheck,
          checks.model.punctuationCheck,
        ],
      },
      {
        id: 'input-capture',
        title: t('settings.diagnostics.input_section', { defaultValue: 'Input & Capture' }),
        description: t('settings.diagnostics.input_section_description', {
          defaultValue: 'Check permissions and the availability of input or capture devices.',
        }),
        checks: [
          checks.input.permissionCheck,
          checks.input.microphoneCheck,
          checks.input.systemAudioCheck,
        ],
      },
      {
        id: 'runtime-environment',
        title: t('settings.diagnostics.runtime_section', { defaultValue: 'Runtime & Environment' }),
        description: t('settings.diagnostics.runtime_section_description', {
          defaultValue: 'Check background runtime readiness and packaged environment dependencies.',
        }),
        checks: [
          checks.runtime.voiceTypingCheck,
          checks.runtime.ffmpegCheck,
          checks.runtime.logDirCheck,
        ],
      },
    ];

    const overview = buildOverviewCards(context, checks);

    return {
      scannedAt: new Date().toISOString(),
      overview,
      sections,
      runtimeEnvironment,
    };
  },

  getResumeOnboardingStep() {
    const config = useConfigStore.getState().config;
    const state = useOnboardingStore.getState().persistedState;
    return getResumeOnboardingStep(config, 'startup', state);
  },
};
