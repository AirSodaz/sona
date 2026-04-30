import type { VoiceTypingReadinessSnapshot } from '../hooks/useVoiceTypingReadiness';
import type { SettingsTab } from '../hooks/useSettingsLogic';
import type { AppConfig } from '../types/config';
import type {
  DiagnosticAction,
  DiagnosticCheck,
  DiagnosticOverviewCard,
  DiagnosticStatus,
} from '../types/diagnostics';
import type { RuntimeEnvironmentStatus, RuntimePathStatus } from '../types/runtime';
import type {
  DeviceProbeResult,
  MicrophonePermissionState,
} from './audioDeviceService';
import type { ModelInfo, ModelRules } from './modelService';

export type Translate = (key: string, options?: Record<string, unknown>) => string;

const STATUS_PRIORITY: Record<DiagnosticStatus, number> = {
  failed: 4,
  missing: 3,
  warning: 2,
  info: 1,
  ready: 0,
};

export interface DiagnosticsSnapshotBuildContext {
  t: Translate;
  config: AppConfig;
  trimmedPaths: {
    streamingModelPath: string;
    offlineModelPath: string;
    vadModelPath: string;
    punctuationModelPath: string;
  };
  selectedModels: {
    live: ModelInfo | null;
    offline: ModelInfo | null;
  };
  modelRules: {
    live: ModelRules | null;
    offline: ModelRules | null;
  };
  pathStatuses: {
    liveModel?: RuntimePathStatus;
    offlineModel?: RuntimePathStatus;
    vad?: RuntimePathStatus;
    punctuation?: RuntimePathStatus;
  };
  permissionState: MicrophonePermissionState;
  microphoneProbe: DeviceProbeResult;
  systemAudioProbe: DeviceProbeResult;
  voiceTypingReadiness: VoiceTypingReadinessSnapshot;
  runtimeEnvironment: RuntimeEnvironmentStatus;
  onboardingReady: boolean;
  punctuationRequired: boolean;
}

export interface ModelDiagnosticChecks {
  liveModelCheck: DiagnosticCheck;
  offlineModelCheck: DiagnosticCheck;
  vadCheck: DiagnosticCheck;
  punctuationCheck: DiagnosticCheck;
}

export interface InputDiagnosticChecks {
  permissionCheck: DiagnosticCheck;
  microphoneCheck: DiagnosticCheck;
  systemAudioCheck: DiagnosticCheck;
}

export interface RuntimeDiagnosticChecks {
  voiceTypingCheck: DiagnosticCheck;
  ffmpegCheck: DiagnosticCheck;
  logDirCheck: DiagnosticCheck;
}

export interface DiagnosticsBuiltChecks {
  model: ModelDiagnosticChecks;
  input: InputDiagnosticChecks;
  runtime: RuntimeDiagnosticChecks;
}

interface PathPolicyCheckArgs {
  context: DiagnosticsSnapshotBuildContext;
  id: string;
  title: string;
  selectedPath: string;
  pathStatus?: RuntimePathStatus;
  missingSelectionStatus: DiagnosticStatus;
  missingSelectionDescription: string;
  missingPathStatus?: DiagnosticStatus;
  unknownPathStatus?: DiagnosticStatus;
  readyDescription: string;
  action?: DiagnosticAction;
  missingPathMeta?: string;
  unknownPathMeta?: string;
  readyMeta?: string;
}

function isRuntimePathMissing(pathStatus?: RuntimePathStatus): boolean {
  return pathStatus?.kind === 'missing';
}

function isRuntimePathUnknown(pathStatus?: RuntimePathStatus): boolean {
  return pathStatus?.kind === 'unknown';
}

function buildOpenSettingsAction(label: string, settingsTab: SettingsTab): DiagnosticAction {
  return {
    kind: 'open_settings',
    label,
    settingsTab,
  };
}

export function pickWorseStatus(...statuses: DiagnosticStatus[]): DiagnosticStatus {
  return statuses.reduce((worst, status) => (
    STATUS_PRIORITY[status] > STATUS_PRIORITY[worst] ? status : worst
  ), 'ready' as DiagnosticStatus);
}

export function buildOpenModelSettingsAction(t: Translate): DiagnosticAction {
  return buildOpenSettingsAction(
    t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
    'models',
  );
}

export function buildOpenInputDeviceAction(t: Translate): DiagnosticAction {
  return buildOpenSettingsAction(
    t('settings.diagnostics.open_input_device', { defaultValue: 'Open Input Device' }),
    'microphone',
  );
}

export function buildOpenVoiceTypingAction(t: Translate): DiagnosticAction {
  return buildOpenSettingsAction(
    t('settings.diagnostics.open_voice_typing', { defaultValue: 'Open Voice Typing' }),
    'voice_typing',
  );
}

export function buildRequestMicrophonePermissionAction(t: Translate): DiagnosticAction {
  return {
    kind: 'request_microphone_permission',
    label: t('settings.diagnostics.request_permission', {
      defaultValue: 'Request Permission',
    }),
  };
}

export function buildOpenLogFolderAction(t: Translate): DiagnosticAction {
  return {
    kind: 'open_log_folder',
    label: t('settings.about_open_logs', { defaultValue: 'Open Log Folder' }),
  };
}

export function buildRetryVoiceTypingWarmupAction(t: Translate): DiagnosticAction {
  return {
    kind: 'retry_voice_typing_warmup',
    label: t('settings.diagnostics.retry_warmup', { defaultValue: 'Retry Warm-up' }),
  };
}

export function buildRunFirstSetupAction(t: Translate): DiagnosticAction {
  return {
    kind: 'run_first_run_setup',
    label: t('settings.diagnostics.run_first_setup', { defaultValue: 'Run First Run Setup' }),
  };
}

export function buildPathUnverifiedDescription(t: Translate): string {
  return t('settings.diagnostics.model_path_unverified', {
    defaultValue: 'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
  });
}

function buildOverviewCard(
  id: string,
  title: string,
  description: string,
  statuses: DiagnosticStatus[],
  action?: DiagnosticAction,
): DiagnosticOverviewCard {
  return {
    id,
    title,
    description,
    status: pickWorseStatus(...statuses),
    action,
  };
}

function mapPermissionStatus(
  permissionState: MicrophonePermissionState,
  t: Translate,
): Pick<DiagnosticCheck, 'status' | 'description' | 'action'> {
  switch (permissionState) {
    case 'granted':
      return {
        status: 'ready',
        description: t('settings.diagnostics.permission_granted', {
          defaultValue: 'Microphone access is already granted.',
        }),
      };
    case 'denied':
      return {
        status: 'failed',
        description: t('settings.diagnostics.permission_denied', {
          defaultValue: 'Microphone access is denied, so the first live recording path cannot start.',
        }),
        action: buildRequestMicrophonePermissionAction(t),
      };
    case 'unsupported':
      return {
        status: 'failed',
        description: t('settings.diagnostics.permission_unsupported', {
          defaultValue: 'This environment does not expose browser microphone permission controls.',
        }),
      };
    case 'prompt':
    default:
      return {
        status: 'warning',
        description: t('settings.diagnostics.permission_prompt', {
          defaultValue: 'Microphone access has not been granted yet.',
        }),
        action: buildRequestMicrophonePermissionAction(t),
      };
  }
}

function buildModelPathPolicyCheck({
  context,
  id,
  title,
  selectedPath,
  pathStatus,
  missingSelectionStatus,
  missingSelectionDescription,
  missingPathStatus = 'failed',
  unknownPathStatus = 'info',
  readyDescription,
  action = buildOpenModelSettingsAction(context.t),
  missingPathMeta = selectedPath,
  unknownPathMeta = selectedPath,
  readyMeta,
}: PathPolicyCheckArgs): DiagnosticCheck {
  if (!selectedPath) {
    return {
      id,
      title,
      status: missingSelectionStatus,
      description: missingSelectionDescription,
      action,
    };
  }

  if (isRuntimePathMissing(pathStatus)) {
    return {
      id,
      title,
      status: missingPathStatus,
      description: context.t('settings.diagnostics.model_path_missing', {
        defaultValue: 'The selected model path no longer exists on disk.',
      }),
      meta: missingPathMeta,
      action,
    };
  }

  if (isRuntimePathUnknown(pathStatus)) {
    return {
      id,
      title,
      status: unknownPathStatus,
      description: buildPathUnverifiedDescription(context.t),
      meta: unknownPathMeta,
      action,
    };
  }

  return {
    id,
    title,
    status: 'ready',
    description: readyDescription,
    meta: readyMeta,
  };
}

function buildVoiceTypingCheck(context: DiagnosticsSnapshotBuildContext): DiagnosticCheck {
  const { t, voiceTypingReadiness } = context;

  switch (voiceTypingReadiness.state) {
    case 'off':
      return {
        id: 'voice-typing',
        title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
        status: 'info',
        description: t('settings.diagnostics.voice_typing_off', {
          defaultValue: 'Voice Typing is currently turned off.',
        }),
        action: buildOpenVoiceTypingAction(t),
      };
    case 'needs_shortcut':
    case 'needs_live_model':
    case 'needs_vad':
      return {
        id: 'voice-typing',
        title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
        status: 'missing',
        description: voiceTypingReadiness.lastErrorMessage || t(
          voiceTypingReadiness.state === 'needs_shortcut'
            ? 'settings.voice_typing_status_summary_missing_shortcut'
            : voiceTypingReadiness.state === 'needs_vad'
              ? 'settings.voice_typing_status_summary_missing_vad'
              : 'settings.voice_typing_status_summary_missing_model',
          {
            defaultValue: 'Voice Typing still needs setup before it can run.',
          },
        ),
        action: buildOpenVoiceTypingAction(t),
      };
    case 'failed':
      return {
        id: 'voice-typing',
        title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
        status: 'failed',
        description: voiceTypingReadiness.lastErrorMessage || t('settings.voice_typing_status_summary_failed', {
          defaultValue: 'Voice Typing hit a runtime problem.',
        }),
        action: buildRetryVoiceTypingWarmupAction(t),
      };
    case 'preparing':
      return {
        id: 'voice-typing',
        title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
        status: 'info',
        description: t('settings.voice_typing_status_summary_preparing', {
          defaultValue: 'Voice Typing is getting ready in the background.',
        }),
      };
    case 'ready':
    default:
      return {
        id: 'voice-typing',
        title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
        status: 'ready',
        description: t('settings.voice_typing_status_summary_ready', {
          defaultValue: 'Voice Typing is ready to dictate into other apps.',
        }),
      };
  }
}

function buildLiveRecordOverviewAction(checks: DiagnosticsBuiltChecks, t: Translate): DiagnosticAction | undefined {
  if (checks.model.liveModelCheck.status !== 'ready' || checks.model.vadCheck.status !== 'ready') {
    return buildOpenModelSettingsAction(t);
  }

  if (checks.input.permissionCheck.action) {
    return checks.input.permissionCheck.action;
  }

  if (checks.input.microphoneCheck.action) {
    return buildOpenInputDeviceAction(t);
  }

  return undefined;
}

function buildBatchImportOverviewAction(
  context: DiagnosticsSnapshotBuildContext,
  checks: DiagnosticsBuiltChecks,
): DiagnosticAction | undefined {
  if (checks.model.offlineModelCheck.status !== 'ready' || checks.model.punctuationCheck.status === 'warning') {
    return buildOpenModelSettingsAction(context.t);
  }

  if (!context.runtimeEnvironment.ffmpegExists) {
    return buildOpenLogFolderAction(context.t);
  }

  return undefined;
}

export function buildModelChecks(context: DiagnosticsSnapshotBuildContext): ModelDiagnosticChecks {
  const { t } = context;

  const liveModelCheck = buildModelPathPolicyCheck({
    context,
    id: 'live-model',
    title: t('settings.diagnostics.live_model_title', { defaultValue: 'Live Record Model' }),
    selectedPath: context.trimmedPaths.streamingModelPath,
    pathStatus: context.pathStatuses.liveModel,
    missingSelectionStatus: 'missing',
    missingSelectionDescription: t('settings.diagnostics.live_model_missing', {
      defaultValue: 'No Live Record Model is selected yet.',
    }),
    readyDescription: t('settings.diagnostics.model_ready', {
      defaultValue: 'The selected model is configured and reachable.',
    }),
    missingPathMeta: context.config.streamingModelPath,
    unknownPathMeta: context.trimmedPaths.streamingModelPath,
    readyMeta: context.selectedModels.live?.name ?? context.config.streamingModelPath,
  });

  const offlineModelCheck = buildModelPathPolicyCheck({
    context,
    id: 'offline-model',
    title: t('settings.diagnostics.offline_model_title', { defaultValue: 'Batch Import Model' }),
    selectedPath: context.trimmedPaths.offlineModelPath,
    pathStatus: context.pathStatuses.offlineModel,
    missingSelectionStatus: 'missing',
    missingSelectionDescription: t('settings.diagnostics.offline_model_missing', {
      defaultValue: 'No Batch Import Model is selected yet.',
    }),
    readyDescription: t('settings.diagnostics.model_ready', {
      defaultValue: 'The selected model is configured and reachable.',
    }),
    missingPathMeta: context.config.offlineModelPath,
    unknownPathMeta: context.trimmedPaths.offlineModelPath,
    readyMeta: context.selectedModels.offline?.name ?? context.config.offlineModelPath,
  });

  const vadCheck: DiagnosticCheck = !context.selectedModels.live
    ? {
        id: 'vad',
        title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
        status: 'info',
        description: t('settings.diagnostics.vad_unknown', {
          defaultValue: 'Pick a Live Record Model first to evaluate whether a VAD model is required.',
        }),
        action: buildOpenModelSettingsAction(t),
      }
    : !context.modelRules.live?.requiresVad
      ? {
          id: 'vad',
          title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
          status: 'ready',
          description: t('settings.diagnostics.vad_not_required', {
            defaultValue: 'The selected Live Record Model does not require a separate VAD model.',
          }),
        }
      : buildModelPathPolicyCheck({
          context,
          id: 'vad',
          title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
          selectedPath: context.trimmedPaths.vadModelPath,
          pathStatus: context.pathStatuses.vad,
          missingSelectionStatus: 'missing',
          missingSelectionDescription: t('settings.diagnostics.vad_missing', {
            defaultValue: 'The selected Live Record Model still needs a VAD model.',
          }),
          readyDescription: t('settings.diagnostics.vad_ready', {
            defaultValue: 'The required VAD model is configured and reachable.',
          }),
          missingPathMeta: context.config.vadModelPath,
          unknownPathMeta: context.trimmedPaths.vadModelPath,
        });

  const punctuationCheck: DiagnosticCheck = !context.punctuationRequired
    ? {
        id: 'punctuation',
        title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
        status: 'ready',
        description: t('settings.diagnostics.punctuation_not_required', {
          defaultValue: 'The current recognition models do not require a separate punctuation model.',
        }),
      }
    : buildModelPathPolicyCheck({
        context,
        id: 'punctuation',
        title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
        selectedPath: context.trimmedPaths.punctuationModelPath,
        pathStatus: context.pathStatuses.punctuation,
        missingSelectionStatus: 'warning',
        missingSelectionDescription: t('settings.diagnostics.punctuation_warning', {
          defaultValue: 'A selected recognition model expects a punctuation model, but none is available yet.',
        }),
        missingPathStatus: 'warning',
        unknownPathStatus: 'warning',
        readyDescription: t('settings.diagnostics.punctuation_ready', {
          defaultValue: 'The required punctuation model is configured and reachable.',
        }),
        missingPathMeta: context.trimmedPaths.punctuationModelPath,
        unknownPathMeta: context.trimmedPaths.punctuationModelPath,
      });

  return {
    liveModelCheck,
    offlineModelCheck,
    vadCheck,
    punctuationCheck,
  };
}

export function buildInputChecks(context: DiagnosticsSnapshotBuildContext): InputDiagnosticChecks {
  const { config, microphoneProbe, permissionState, systemAudioProbe, t } = context;
  const inputDeviceAction = buildOpenInputDeviceAction(t);

  const permissionCheck: DiagnosticCheck = {
    id: 'microphone-permission',
    title: t('settings.diagnostics.permission_title', { defaultValue: 'Microphone Permission' }),
    ...mapPermissionStatus(permissionState, t),
  };

  const microphoneCheck: DiagnosticCheck = !microphoneProbe.available
    ? {
        id: 'microphone-device',
        title: t('settings.diagnostics.microphone_title', { defaultValue: 'Input Device' }),
        status: 'failed',
        description: microphoneProbe.errorMessage || t('settings.diagnostics.microphone_unavailable', {
          defaultValue: 'No microphone devices are currently available.',
        }),
        action: inputDeviceAction,
      }
    : config.microphoneId === 'default' || microphoneProbe.options.some((option) => option.value === config.microphoneId)
      ? {
          id: 'microphone-device',
          title: t('settings.diagnostics.microphone_title', { defaultValue: 'Input Device' }),
          status: 'ready',
          description: t('settings.diagnostics.microphone_ready', {
            defaultValue: 'The current input-device selection is still available.',
          }),
          meta: config.microphoneId === 'default'
            ? t('settings.mic_auto')
            : config.microphoneId,
        }
      : {
          id: 'microphone-device',
          title: t('settings.diagnostics.microphone_title', { defaultValue: 'Input Device' }),
          status: 'failed',
          description: t('settings.diagnostics.microphone_missing_selection', {
            defaultValue: 'The saved microphone selection is no longer available.',
          }),
          meta: config.microphoneId,
          action: inputDeviceAction,
        };

  const systemAudioCheck: DiagnosticCheck = systemAudioProbe.available
    ? {
        id: 'system-audio-capture',
        title: t('settings.diagnostics.system_audio_title', { defaultValue: 'System Audio Capture' }),
        status: 'ready',
        description: t('settings.diagnostics.system_audio_ready', {
          defaultValue: 'System audio capture devices are available.',
        }),
      }
    : {
        id: 'system-audio-capture',
        title: t('settings.diagnostics.system_audio_title', { defaultValue: 'System Audio Capture' }),
        status: 'warning',
        description: systemAudioProbe.errorMessage || t('settings.diagnostics.system_audio_warning', {
          defaultValue: 'Sona could not enumerate system audio capture devices right now.',
        }),
        action: inputDeviceAction,
      };

  return {
    permissionCheck,
    microphoneCheck,
    systemAudioCheck,
  };
}

export function buildRuntimeChecks(context: DiagnosticsSnapshotBuildContext): RuntimeDiagnosticChecks {
  const { runtimeEnvironment, t } = context;

  const voiceTypingCheck = buildVoiceTypingCheck(context);
  const openLogFolderAction = buildOpenLogFolderAction(t);

  const ffmpegCheck: DiagnosticCheck = runtimeEnvironment.ffmpegExists
    ? {
        id: 'ffmpeg',
        title: t('settings.diagnostics.ffmpeg_title', { defaultValue: 'FFmpeg Sidecar' }),
        status: 'ready',
        description: t('settings.diagnostics.ffmpeg_ready', {
          defaultValue: 'The bundled FFmpeg sidecar is present.',
        }),
        meta: runtimeEnvironment.ffmpegPath,
      }
    : {
        id: 'ffmpeg',
        title: t('settings.diagnostics.ffmpeg_title', { defaultValue: 'FFmpeg Sidecar' }),
        status: 'failed',
        description: t('settings.diagnostics.ffmpeg_missing', {
          defaultValue: 'The bundled FFmpeg sidecar could not be found. Batch imports and media decoding may fail until the app is reinstalled.',
        }),
        meta: runtimeEnvironment.ffmpegPath,
        action: openLogFolderAction,
      };

  const logDirCheck: DiagnosticCheck = runtimeEnvironment.logDirPath.trim()
    ? {
        id: 'log-dir',
        title: t('settings.diagnostics.log_dir_title', { defaultValue: 'Log Directory' }),
        status: 'ready',
        description: t('settings.diagnostics.log_dir_ready', {
          defaultValue: 'Runtime logs can be resolved for troubleshooting.',
        }),
        meta: runtimeEnvironment.logDirPath,
        action: openLogFolderAction,
      }
    : {
        id: 'log-dir',
        title: t('settings.diagnostics.log_dir_title', { defaultValue: 'Log Directory' }),
        status: 'failed',
        description: t('settings.diagnostics.log_dir_missing', {
          defaultValue: 'Sona could not resolve the runtime log directory.',
        }),
      };

  return {
    voiceTypingCheck,
    ffmpegCheck,
    logDirCheck,
  };
}

export function buildOverviewCards(
  context: DiagnosticsSnapshotBuildContext,
  checks: DiagnosticsBuiltChecks,
): DiagnosticOverviewCard[] {
  const { t } = context;

  return [
    buildOverviewCard(
      'first-run',
      t('settings.diagnostics.first_run_card', { defaultValue: 'First Run Setup' }),
      context.onboardingReady
        ? t('settings.diagnostics.first_run_ready', {
            defaultValue: 'Recommended local models are configured.',
          })
        : t('settings.diagnostics.first_run_missing', {
            defaultValue: 'The recommended offline setup is still incomplete.',
          }),
      [
        context.onboardingReady ? 'ready' : 'missing',
        checks.input.permissionCheck.status === 'failed' ? 'warning' : checks.input.permissionCheck.status,
      ],
      context.onboardingReady ? undefined : buildRunFirstSetupAction(t),
    ),
    buildOverviewCard(
      'live-record',
      t('settings.diagnostics.live_record_card', { defaultValue: 'Live Record' }),
      t('settings.diagnostics.live_record_card_description', {
        defaultValue: 'Model, VAD, permission, and microphone selection for real-time capture.',
      }),
      [
        checks.model.liveModelCheck.status,
        checks.model.vadCheck.status,
        checks.input.permissionCheck.status,
        checks.input.microphoneCheck.status,
      ],
      buildLiveRecordOverviewAction(checks, t),
    ),
    buildOverviewCard(
      'batch-import',
      t('settings.diagnostics.batch_import_card', { defaultValue: 'Batch Import' }),
      t('settings.diagnostics.batch_import_card_description', {
        defaultValue: 'Offline model and bundled media decoding support for file processing.',
      }),
      [
        checks.model.offlineModelCheck.status,
        checks.model.punctuationCheck.status,
        checks.runtime.ffmpegCheck.status,
      ],
      buildBatchImportOverviewAction(context, checks),
    ),
    buildOverviewCard(
      'voice-typing',
      t('settings.diagnostics.voice_typing_card', { defaultValue: 'Voice Typing' }),
      t('settings.diagnostics.voice_typing_card_description', {
        defaultValue: 'Shortcut, live model reuse, and runtime warm-up for dictation.',
      }),
      [checks.runtime.voiceTypingCheck.status],
      checks.runtime.voiceTypingCheck.action,
    ),
  ];
}
