import type {
  DiagnosticAction,
  DiagnosticCheck,
  DiagnosticOverviewCard,
  DiagnosticSection,
  DiagnosticStatus,
  DiagnosticsSnapshot,
} from '../types/diagnostics';
import type { SettingsTab } from '../hooks/useSettingsLogic';
import type {
  AsrInferenceMetric,
  AsrModelLoadMetric,
  AsrRuntimeMetricsSnapshot,
  RuntimeEnvironmentStatus,
  RuntimePathStatus,
} from '../types/runtime';
import type {
  DeviceProbeResult,
  MicrophonePermissionState,
} from './audioDeviceService';
import type { VoiceTypingReadinessSnapshot } from '../hooks/useVoiceTypingReadiness';

export type Translate = (key: string, options?: Record<string, unknown>) => string;

export interface DiagnosticsConfigFacts {
  streamingModelPath: string;
  offlineModelPath: string;
  vadModelPath: string;
  punctuationModelPath: string;
  microphoneId: string;
}

export interface ModelSummaryFacts {
  id: string;
  name: string;
}

export interface ModelRuleFacts {
  requiresVad: boolean;
  requiresPunctuation: boolean;
}

export interface DiagnosticsSelectedModelsFacts {
  live: ModelSummaryFacts | null;
  offline: ModelSummaryFacts | null;
}

export interface DiagnosticsModelRulesFacts {
  live: ModelRuleFacts | null;
  offline: ModelRuleFacts | null;
}

export interface DiagnosticsPathStatusesFacts {
  liveModel: RuntimePathStatus | null;
  offlineModel: RuntimePathStatus | null;
  vad: RuntimePathStatus | null;
  punctuation: RuntimePathStatus | null;
}

export interface DeviceProbeFacts {
  options: DeviceProbeResult['options'];
  available: boolean;
  errorMessage?: string | null;
}

export interface VoiceTypingReadinessFacts {
  state: VoiceTypingReadinessSnapshot['state'];
  lastErrorMessage: string | null;
}

export interface DiagnosticsCoreInput {
  config: DiagnosticsConfigFacts;
  permissionState: MicrophonePermissionState;
  microphoneProbe: DeviceProbeResult;
  systemAudioProbe: DeviceProbeResult;
  voiceTypingReadiness: VoiceTypingReadinessSnapshot;
}

export interface DiagnosticsCoreFactsSnapshot {
  scannedAt: string;
  config: DiagnosticsConfigFacts;
  selectedModels: DiagnosticsSelectedModelsFacts;
  modelRules: DiagnosticsModelRulesFacts;
  pathStatuses: DiagnosticsPathStatusesFacts;
  permissionState: MicrophonePermissionState;
  microphoneProbe: DeviceProbeFacts;
  systemAudioProbe: DeviceProbeFacts;
  voiceTypingReadiness: VoiceTypingReadinessFacts;
  runtimeEnvironment: RuntimeEnvironmentStatus;
  asrRuntimeMetrics: AsrRuntimeMetricsSnapshot;
  onboardingReady: boolean;
  punctuationRequired: boolean;
}

interface BuiltChecks {
  model: {
    liveModelCheck: DiagnosticCheck;
    offlineModelCheck: DiagnosticCheck;
    vadCheck: DiagnosticCheck;
    punctuationCheck: DiagnosticCheck;
  };
  input: {
    permissionCheck: DiagnosticCheck;
    microphoneCheck: DiagnosticCheck;
    systemAudioCheck: DiagnosticCheck;
  };
  runtime: {
    voiceTypingCheck: DiagnosticCheck;
    ffmpegCheck: DiagnosticCheck;
    logDirCheck: DiagnosticCheck;
  };
  asr: {
    modelMemoryCheck: DiagnosticCheck;
    liveLatencyCheck: DiagnosticCheck;
    batchLatencyCheck: DiagnosticCheck;
  };
}

interface PathPolicyCheckArgs {
  id: string;
  title: string;
  selectedPath: string;
  pathStatus?: RuntimePathStatus | null;
  missingSelectionStatus: DiagnosticStatus;
  missingSelectionDescription: string;
  missingPathStatus: DiagnosticStatus;
  missingPathDescription: string;
  unknownPathStatus: DiagnosticStatus;
  unknownPathDescription: string;
  readyDescription: string;
  action?: DiagnosticAction;
  missingPathMeta?: string;
  unknownPathMeta?: string;
  readyMeta?: string;
}

function tr(
  t: Translate,
  key: string,
  defaultValue: string,
  params?: Record<string, unknown>,
): string {
  return t(key, {
    defaultValue,
    ...(params ?? {}),
  });
}

function runtimeMessage(t: Translate, message: string): string {
  return tr(t, 'diagnostics.runtime_message', message);
}

function openSettingsAction(
  t: Translate,
  settingsTab: SettingsTab,
  labelKey: string,
  defaultValue: string,
): DiagnosticAction {
  return {
    kind: 'open_settings',
    label: tr(t, labelKey, defaultValue),
    settingsTab,
  };
}

function openModelSettingsAction(t: Translate): DiagnosticAction {
  return openSettingsAction(
    t,
    'models',
    'settings.diagnostics.open_model_settings',
    'Open Model Settings',
  );
}

function openInputDeviceAction(t: Translate): DiagnosticAction {
  return openSettingsAction(
    t,
    'microphone',
    'settings.diagnostics.open_input_device',
    'Open Input Device',
  );
}

function openVoiceTypingAction(t: Translate): DiagnosticAction {
  return openSettingsAction(
    t,
    'subtitle',
    'settings.diagnostics.open_voice_typing',
    'Open Voice Typing',
  );
}

function requestMicrophonePermissionAction(t: Translate): DiagnosticAction {
  return {
    kind: 'request_microphone_permission',
    label: tr(t, 'settings.diagnostics.request_permission', 'Request Permission'),
  };
}

function retryVoiceTypingWarmupAction(t: Translate): DiagnosticAction {
  return {
    kind: 'retry_voice_typing_warmup',
    label: tr(t, 'settings.diagnostics.retry_warmup', 'Retry Warm-up'),
  };
}

function runFirstSetupAction(t: Translate): DiagnosticAction {
  return {
    kind: 'run_first_run_setup',
    label: tr(t, 'settings.diagnostics.run_first_setup', 'Run First Run Setup'),
  };
}

function openLogFolderAction(t: Translate): DiagnosticAction {
  return {
    kind: 'open_log_folder',
    label: tr(t, 'settings.about_open_logs', 'Open Log Folder'),
  };
}

function check(
  id: string,
  title: string,
  status: DiagnosticStatus,
  description: string,
  action?: DiagnosticAction,
  meta?: string,
): DiagnosticCheck {
  return {
    id,
    title,
    description,
    status,
    action,
    meta,
  };
}

function buildModelPathPolicyCheck(args: PathPolicyCheckArgs): DiagnosticCheck {
  if (args.selectedPath.trim().length === 0) {
    return check(
      args.id,
      args.title,
      args.missingSelectionStatus,
      args.missingSelectionDescription,
      args.action,
    );
  }

  if (args.pathStatus?.kind === 'missing') {
    return check(
      args.id,
      args.title,
      args.missingPathStatus,
      args.missingPathDescription,
      args.action,
      args.missingPathMeta,
    );
  }

  if (args.pathStatus?.kind === 'unknown') {
    return check(
      args.id,
      args.title,
      args.unknownPathStatus,
      args.unknownPathDescription,
      args.action,
      args.unknownPathMeta,
    );
  }

  return check(
    args.id,
    args.title,
    'ready',
    args.readyDescription,
    undefined,
    args.readyMeta,
  );
}

function buildModelChecks(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
): BuiltChecks['model'] {
  const { config } = snapshot;
  const streamingPath = config.streamingModelPath.trim();
  const offlinePath = config.offlineModelPath.trim();
  const vadPath = config.vadModelPath.trim();
  const punctuationPath = config.punctuationModelPath.trim();
  const openModelSettings = openModelSettingsAction(t);

  const liveModelCheck = buildModelPathPolicyCheck({
    id: 'live-model',
    title: tr(t, 'settings.diagnostics.live_model_title', 'Live Record Model'),
    selectedPath: streamingPath,
    pathStatus: snapshot.pathStatuses.liveModel,
    missingSelectionStatus: 'missing',
    missingSelectionDescription: tr(
      t,
      'settings.diagnostics.live_model_missing',
      'No Live Record Model is selected yet.',
    ),
    missingPathStatus: 'failed',
    missingPathDescription: tr(
      t,
      'settings.diagnostics.model_path_missing',
      'The selected model path no longer exists on disk.',
    ),
    unknownPathStatus: 'info',
    unknownPathDescription: tr(
      t,
      'settings.diagnostics.model_path_unverified',
      'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
    ),
    readyDescription: tr(
      t,
      'settings.diagnostics.model_ready',
      'The selected model is configured and reachable.',
    ),
    action: openModelSettings,
    missingPathMeta: config.streamingModelPath,
    unknownPathMeta: streamingPath,
    readyMeta: snapshot.selectedModels.live?.name ?? config.streamingModelPath,
  });

  const offlineModelCheck = buildModelPathPolicyCheck({
    id: 'offline-model',
    title: tr(t, 'settings.diagnostics.offline_model_title', 'Batch Import Model'),
    selectedPath: offlinePath,
    pathStatus: snapshot.pathStatuses.offlineModel,
    missingSelectionStatus: 'missing',
    missingSelectionDescription: tr(
      t,
      'settings.diagnostics.offline_model_missing',
      'No Batch Import Model is selected yet.',
    ),
    missingPathStatus: 'failed',
    missingPathDescription: tr(
      t,
      'settings.diagnostics.model_path_missing',
      'The selected model path no longer exists on disk.',
    ),
    unknownPathStatus: 'info',
    unknownPathDescription: tr(
      t,
      'settings.diagnostics.model_path_unverified',
      'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
    ),
    readyDescription: tr(
      t,
      'settings.diagnostics.model_ready',
      'The selected model is configured and reachable.',
    ),
    action: openModelSettings,
    missingPathMeta: config.offlineModelPath,
    unknownPathMeta: offlinePath,
    readyMeta: snapshot.selectedModels.offline?.name ?? config.offlineModelPath,
  });

  const vadCheck = !snapshot.selectedModels.live
    ? check(
        'vad',
        tr(t, 'settings.diagnostics.vad_title', 'VAD Dependency'),
        'info',
        tr(
          t,
          'settings.diagnostics.vad_unknown',
          'Pick a Live Record Model first to evaluate whether a VAD model is required.',
        ),
        openModelSettings,
      )
    : !snapshot.modelRules.live?.requiresVad
      ? check(
          'vad',
          tr(t, 'settings.diagnostics.vad_title', 'VAD Dependency'),
          'ready',
          tr(
            t,
            'settings.diagnostics.vad_not_required',
            'The selected Live Record Model does not require a separate VAD model.',
          ),
        )
      : buildModelPathPolicyCheck({
          id: 'vad',
          title: tr(t, 'settings.diagnostics.vad_title', 'VAD Dependency'),
          selectedPath: vadPath,
          pathStatus: snapshot.pathStatuses.vad,
          missingSelectionStatus: 'missing',
          missingSelectionDescription: tr(
            t,
            'settings.diagnostics.vad_missing',
            'The selected Live Record Model still needs a VAD model.',
          ),
          missingPathStatus: 'failed',
          missingPathDescription: tr(
            t,
            'settings.diagnostics.model_path_missing',
            'The selected model path no longer exists on disk.',
          ),
          unknownPathStatus: 'info',
          unknownPathDescription: tr(
            t,
            'settings.diagnostics.model_path_unverified',
            'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
          ),
          readyDescription: tr(
            t,
            'settings.diagnostics.vad_ready',
            'The required VAD model is configured and reachable.',
          ),
          action: openModelSettings,
          missingPathMeta: config.vadModelPath,
          unknownPathMeta: vadPath,
        });

  const punctuationCheck = !snapshot.punctuationRequired
    ? check(
        'punctuation',
        tr(t, 'settings.diagnostics.punctuation_title', 'Punctuation Dependency'),
        'ready',
        tr(
          t,
          'settings.diagnostics.punctuation_not_required',
          'The current recognition models do not require a separate punctuation model.',
        ),
      )
    : buildModelPathPolicyCheck({
        id: 'punctuation',
        title: tr(t, 'settings.diagnostics.punctuation_title', 'Punctuation Dependency'),
        selectedPath: punctuationPath,
        pathStatus: snapshot.pathStatuses.punctuation,
        missingSelectionStatus: 'warning',
        missingSelectionDescription: tr(
          t,
          'settings.diagnostics.punctuation_warning',
          'A selected recognition model expects a punctuation model, but none is available yet.',
        ),
        missingPathStatus: 'warning',
        missingPathDescription: tr(
          t,
          'settings.diagnostics.model_path_missing',
          'The selected model path no longer exists on disk.',
        ),
        unknownPathStatus: 'warning',
        unknownPathDescription: tr(
          t,
          'settings.diagnostics.model_path_unverified',
          'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
        ),
        readyDescription: tr(
          t,
          'settings.diagnostics.punctuation_ready',
          'The required punctuation model is configured and reachable.',
        ),
        action: openModelSettings,
        missingPathMeta: punctuationPath,
        unknownPathMeta: punctuationPath,
      });

  return {
    liveModelCheck,
    offlineModelCheck,
    vadCheck,
    punctuationCheck,
  };
}

function buildInputChecks(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
): BuiltChecks['input'] {
  const permissionCheck = (() => {
    switch (snapshot.permissionState) {
      case 'granted':
        return check(
          'microphone-permission',
          tr(t, 'settings.diagnostics.permission_title', 'Microphone Permission'),
          'ready',
          tr(
            t,
            'settings.diagnostics.permission_granted',
            'Microphone access is already granted.',
          ),
        );
      case 'denied':
        return check(
          'microphone-permission',
          tr(t, 'settings.diagnostics.permission_title', 'Microphone Permission'),
          'failed',
          tr(
            t,
            'settings.diagnostics.permission_denied',
            'Microphone access is denied, so the first live recording path cannot start.',
          ),
          requestMicrophonePermissionAction(t),
        );
      case 'unsupported':
        return check(
          'microphone-permission',
          tr(t, 'settings.diagnostics.permission_title', 'Microphone Permission'),
          'failed',
          tr(
            t,
            'settings.diagnostics.permission_unsupported',
            'This environment does not expose browser microphone permission controls.',
          ),
        );
      default:
        return check(
          'microphone-permission',
          tr(t, 'settings.diagnostics.permission_title', 'Microphone Permission'),
          'warning',
          tr(
            t,
            'settings.diagnostics.permission_prompt',
            'Microphone access has not been granted yet.',
          ),
          requestMicrophonePermissionAction(t),
        );
    }
  })();

  const microphoneId = snapshot.config.microphoneId.trim() || 'default';
  const microphoneCheck = !snapshot.microphoneProbe.available
    ? check(
        'microphone-device',
        tr(t, 'settings.diagnostics.microphone_title', 'Input Device'),
        'failed',
        snapshot.microphoneProbe.errorMessage
          ? runtimeMessage(t, snapshot.microphoneProbe.errorMessage)
          : tr(
              t,
              'settings.diagnostics.microphone_unavailable',
              'No microphone devices are currently available.',
            ),
        openInputDeviceAction(t),
      )
    : microphoneId === 'default'
      || snapshot.microphoneProbe.options.some((option) => option.value === microphoneId)
      ? check(
          'microphone-device',
          tr(t, 'settings.diagnostics.microphone_title', 'Input Device'),
          'ready',
          tr(
            t,
            'settings.diagnostics.microphone_ready',
            'The current input-device selection is still available.',
          ),
          undefined,
          microphoneId === 'default' ? tr(t, 'settings.mic_auto', 'Auto') : microphoneId,
        )
      : check(
          'microphone-device',
          tr(t, 'settings.diagnostics.microphone_title', 'Input Device'),
          'failed',
          tr(
            t,
            'settings.diagnostics.microphone_missing_selection',
            'The saved microphone selection is no longer available.',
          ),
          openInputDeviceAction(t),
          microphoneId,
        );

  const systemAudioCheck = snapshot.systemAudioProbe.available
    ? check(
        'system-audio-capture',
        tr(t, 'settings.diagnostics.system_audio_title', 'System Audio Capture'),
        'ready',
        tr(
          t,
          'settings.diagnostics.system_audio_ready',
          'System audio capture devices are available.',
        ),
      )
    : check(
        'system-audio-capture',
        tr(t, 'settings.diagnostics.system_audio_title', 'System Audio Capture'),
        'warning',
        snapshot.systemAudioProbe.errorMessage
          ? runtimeMessage(t, snapshot.systemAudioProbe.errorMessage)
          : tr(
              t,
              'settings.diagnostics.system_audio_warning',
              'Sona could not enumerate system audio capture devices right now.',
            ),
        openInputDeviceAction(t),
      );

  return {
    permissionCheck,
    microphoneCheck,
    systemAudioCheck,
  };
}

function buildVoiceTypingCheck(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
): DiagnosticCheck {
  const title = tr(t, 'settings.diagnostics.voice_typing_title', 'Voice Typing Runtime');
  const readiness = snapshot.voiceTypingReadiness;
  switch (readiness.state) {
    case 'off':
      return check(
        'voice-typing',
        title,
        'info',
        tr(t, 'settings.diagnostics.voice_typing_off', 'Voice Typing is currently turned off.'),
        openVoiceTypingAction(t),
      );
    case 'needs_shortcut':
    case 'needs_live_model':
    case 'needs_vad': {
      const key = readiness.state === 'needs_shortcut'
        ? 'settings.voice_typing_status_summary_missing_shortcut'
        : readiness.state === 'needs_vad'
          ? 'settings.voice_typing_status_summary_missing_vad'
          : 'settings.voice_typing_status_summary_missing_model';
      return check(
        'voice-typing',
        title,
        'missing',
        readiness.lastErrorMessage
          ? runtimeMessage(t, readiness.lastErrorMessage)
          : tr(t, key, 'Voice Typing still needs setup before it can run.'),
        openVoiceTypingAction(t),
      );
    }
    case 'failed':
      return check(
        'voice-typing',
        title,
        'failed',
        readiness.lastErrorMessage
          ? runtimeMessage(t, readiness.lastErrorMessage)
          : tr(
              t,
              'settings.voice_typing_status_summary_failed',
              'Voice Typing hit a runtime problem.',
            ),
        retryVoiceTypingWarmupAction(t),
      );
    case 'preparing':
      return check(
        'voice-typing',
        title,
        'info',
        tr(
          t,
          'settings.voice_typing_status_summary_preparing',
          'Voice Typing is getting ready in the background.',
        ),
      );
    case 'ready':
    default:
      return check(
        'voice-typing',
        title,
        'ready',
        tr(
          t,
          'settings.voice_typing_status_summary_ready',
          'Voice Typing is ready to dictate into other apps.',
        ),
      );
  }
}

function buildRuntimeChecks(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
): BuiltChecks['runtime'] {
  const voiceTypingCheck = buildVoiceTypingCheck(t, snapshot);
  const ffmpegCheck = snapshot.runtimeEnvironment.ffmpegExists
    ? check(
        'ffmpeg',
        tr(t, 'settings.diagnostics.ffmpeg_title', 'FFmpeg Sidecar'),
        'ready',
        tr(t, 'settings.diagnostics.ffmpeg_ready', 'The bundled FFmpeg sidecar is present.'),
        undefined,
        snapshot.runtimeEnvironment.ffmpegPath,
      )
    : check(
        'ffmpeg',
        tr(t, 'settings.diagnostics.ffmpeg_title', 'FFmpeg Sidecar'),
        'failed',
        tr(
          t,
          'settings.diagnostics.ffmpeg_missing',
          'The bundled FFmpeg sidecar could not be found. Batch imports and media decoding may fail until the app is reinstalled.',
        ),
        openLogFolderAction(t),
        snapshot.runtimeEnvironment.ffmpegPath,
      );
  const logDirCheck = snapshot.runtimeEnvironment.logDirPath.trim().length === 0
    ? check(
        'log-dir',
        tr(t, 'settings.diagnostics.log_dir_title', 'Log Directory'),
        'failed',
        tr(
          t,
          'settings.diagnostics.log_dir_missing',
          'Sona could not resolve the runtime log directory.',
        ),
      )
    : check(
        'log-dir',
        tr(t, 'settings.diagnostics.log_dir_title', 'Log Directory'),
        'ready',
        tr(
          t,
          'settings.diagnostics.log_dir_ready',
          'Runtime logs can be resolved for troubleshooting.',
        ),
        openLogFolderAction(t),
        snapshot.runtimeEnvironment.logDirPath,
      );

  return {
    voiceTypingCheck,
    ffmpegCheck,
    logDirCheck,
  };
}

function formatMetricMs(value: number | null | undefined): string {
  return Number.isFinite(value) ? `${Math.round(value as number)} ms` : 'unknown';
}

function formatMetricMb(value: number | null | undefined): string {
  return Number.isFinite(value) ? `${(value as number).toFixed(1)} MB` : 'unknown';
}

function formatSignedMetricMb(value: number | null | undefined): string {
  if (!Number.isFinite(value)) {
    return 'unknown';
  }
  return `${(value as number) >= 0 ? '+' : ''}${(value as number).toFixed(1)} MB`;
}

function formatRtf(value: number | null | undefined): string {
  return Number.isFinite(value) ? (value as number).toFixed(2) : 'unknown';
}

function describeModelLoadMetric(metric: AsrModelLoadMetric): string {
  return [
    `${metric.modelType} ${metric.recognizerKind}`,
    `RSS ${formatMetricMb(metric.processRssMb ?? metric.rssAfterMb)}`,
    `delta ${formatSignedMetricMb(metric.rssDeltaMb)}`,
    `load ${formatMetricMs(metric.loadMs)}`,
    metric.reusedFromPool ? 'reused recognizer' : 'new recognizer',
    metric.instanceId,
  ].join(' · ');
}

function describeInferenceMetric(metric: AsrInferenceMetric): string {
  const parts = [
    `stage ${metric.stage}`,
    `decode ${formatMetricMs(metric.decodeMs)}`,
  ];

  if (metric.audioExtractMs !== null && metric.audioExtractMs !== undefined) {
    parts.push(`extract ${formatMetricMs(metric.audioExtractMs)}`);
  }
  if (metric.emitLatencyMs !== null && metric.emitLatencyMs !== undefined) {
    parts.push(`latency ${formatMetricMs(metric.emitLatencyMs)}`);
  }
  if (metric.totalMs !== null && metric.totalMs !== undefined) {
    parts.push(`total ${formatMetricMs(metric.totalMs)}`);
  }
  parts.push(`RTF ${formatRtf(metric.rtf)}`);
  parts.push(`RSS ${formatMetricMb(metric.processRssMb)}`);
  if (metric.segmentCount !== null && metric.segmentCount !== undefined) {
    parts.push(`segments ${metric.segmentCount}`);
  }

  return parts.join(' · ');
}

function buildAsrPerformanceChecks(
  t: Translate,
  metrics: AsrRuntimeMetricsSnapshot,
): BuiltChecks['asr'] {
  const modelMemoryCheck = metrics.modelLoad
    ? check(
        'asr-model-memory',
        tr(t, 'settings.diagnostics.asr_model_memory_title', 'Model memory'),
        'ready',
        tr(
          t,
          'settings.diagnostics.asr_model_memory_ready',
          `Last model load: ${metrics.modelLoad.modelType} (${metrics.modelLoad.recognizerKind}).`,
          {
            modelType: metrics.modelLoad.modelType,
            recognizerKind: metrics.modelLoad.recognizerKind,
          },
        ),
        undefined,
        describeModelLoadMetric(metrics.modelLoad),
      )
    : check(
        'asr-model-memory',
        tr(t, 'settings.diagnostics.asr_model_memory_title', 'Model memory'),
        'info',
        tr(
          t,
          'settings.diagnostics.asr_model_memory_empty',
          'No ASR runtime metrics have been captured yet.',
        ),
      );

  const liveLatencyCheck = metrics.liveInference
    ? check(
        'asr-live-latency',
        tr(t, 'settings.diagnostics.asr_live_latency_title', 'Live transcription latency'),
        'ready',
        tr(
          t,
          'settings.diagnostics.asr_live_latency_ready',
          `Last live inference from ${metrics.liveInference.instanceId ?? 'unknown instance'}.`,
          { instanceId: metrics.liveInference.instanceId ?? 'unknown instance' },
        ),
        undefined,
        describeInferenceMetric(metrics.liveInference),
      )
    : check(
        'asr-live-latency',
        tr(t, 'settings.diagnostics.asr_live_latency_title', 'Live transcription latency'),
        'info',
        tr(
          t,
          'settings.diagnostics.asr_live_latency_empty',
          'No live transcription latency has been captured yet.',
        ),
      );

  const batchLatencyCheck = metrics.batchInference
    ? check(
        'asr-batch-latency',
        tr(t, 'settings.diagnostics.asr_batch_latency_title', 'Batch transcription latency'),
        'ready',
        tr(
          t,
          'settings.diagnostics.asr_batch_latency_ready',
          'Last batch transcription run completed.',
        ),
        undefined,
        describeInferenceMetric(metrics.batchInference),
      )
    : check(
        'asr-batch-latency',
        tr(t, 'settings.diagnostics.asr_batch_latency_title', 'Batch transcription latency'),
        'info',
        tr(
          t,
          'settings.diagnostics.asr_batch_latency_empty',
          'No batch transcription latency has been captured yet.',
        ),
      );

  return {
    modelMemoryCheck,
    liveLatencyCheck,
    batchLatencyCheck,
  };
}

function statusPriority(status: DiagnosticStatus): number {
  switch (status) {
    case 'failed':
      return 4;
    case 'missing':
      return 3;
    case 'warning':
      return 2;
    case 'info':
      return 1;
    case 'ready':
    default:
      return 0;
  }
}

function pickWorseStatus(statuses: DiagnosticStatus[]): DiagnosticStatus {
  return statuses.reduce<DiagnosticStatus>(
    (worst, status) => (statusPriority(status) > statusPriority(worst) ? status : worst),
    'ready',
  );
}

function overviewCard(
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
    status: pickWorseStatus(statuses),
    action,
  };
}

function liveRecordOverviewAction(t: Translate, checks: BuiltChecks): DiagnosticAction | undefined {
  if (checks.model.liveModelCheck.status !== 'ready' || checks.model.vadCheck.status !== 'ready') {
    return openModelSettingsAction(t);
  }
  if (checks.input.permissionCheck.action) {
    return checks.input.permissionCheck.action;
  }
  if (checks.input.microphoneCheck.action) {
    return openInputDeviceAction(t);
  }
  return undefined;
}

function batchImportOverviewAction(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
  checks: BuiltChecks,
): DiagnosticAction | undefined {
  if (
    checks.model.offlineModelCheck.status !== 'ready'
    || checks.model.punctuationCheck.status === 'warning'
  ) {
    return openModelSettingsAction(t);
  }
  if (!snapshot.runtimeEnvironment.ffmpegExists) {
    return openLogFolderAction(t);
  }
  return undefined;
}

function buildOverviewCards(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
  checks: BuiltChecks,
): DiagnosticOverviewCard[] {
  return [
    overviewCard(
      'first-run',
      tr(t, 'settings.diagnostics.first_run_card', 'First Run Setup'),
      snapshot.onboardingReady
        ? tr(t, 'settings.diagnostics.first_run_ready', 'Recommended local models are configured.')
        : tr(
            t,
            'settings.diagnostics.first_run_missing',
            'The recommended offline setup is still incomplete.',
          ),
      [
        snapshot.onboardingReady ? 'ready' : 'missing',
        checks.input.permissionCheck.status === 'failed'
          ? 'warning'
          : checks.input.permissionCheck.status,
      ],
      snapshot.onboardingReady ? undefined : runFirstSetupAction(t),
    ),
    overviewCard(
      'live-record',
      tr(t, 'settings.diagnostics.live_record_card', 'Live Record'),
      tr(
        t,
        'settings.diagnostics.live_record_card_description',
        'Model, VAD, permission, and microphone selection for real-time capture.',
      ),
      [
        checks.model.liveModelCheck.status,
        checks.model.vadCheck.status,
        checks.input.permissionCheck.status,
        checks.input.microphoneCheck.status,
      ],
      liveRecordOverviewAction(t, checks),
    ),
    overviewCard(
      'batch-import',
      tr(t, 'settings.diagnostics.batch_import_card', 'Batch Import'),
      tr(
        t,
        'settings.diagnostics.batch_import_card_description',
        'Offline model and bundled media decoding support for file processing.',
      ),
      [
        checks.model.offlineModelCheck.status,
        checks.model.punctuationCheck.status,
        checks.runtime.ffmpegCheck.status,
      ],
      batchImportOverviewAction(t, snapshot, checks),
    ),
    overviewCard(
      'voice-typing',
      tr(t, 'settings.diagnostics.voice_typing_card', 'Voice Typing'),
      tr(
        t,
        'settings.diagnostics.voice_typing_card_description',
        'Shortcut, live model reuse, and runtime warm-up for dictation.',
      ),
      [checks.runtime.voiceTypingCheck.status],
      checks.runtime.voiceTypingCheck.action,
    ),
  ];
}

function buildSections(t: Translate, checks: BuiltChecks): DiagnosticSection[] {
  return [
    {
      id: 'models',
      title: tr(t, 'settings.diagnostics.models_section', 'Models'),
      description: tr(
        t,
        'settings.diagnostics.models_section_description',
        'Check that local transcription models and required dependencies are present.',
      ),
      checks: [
        checks.model.liveModelCheck,
        checks.model.offlineModelCheck,
        checks.model.vadCheck,
        checks.model.punctuationCheck,
      ],
    },
    {
      id: 'input-capture',
      title: tr(t, 'settings.diagnostics.input_section', 'Input & Capture'),
      description: tr(
        t,
        'settings.diagnostics.input_section_description',
        'Check permissions and the availability of input or capture devices.',
      ),
      checks: [
        checks.input.permissionCheck,
        checks.input.microphoneCheck,
        checks.input.systemAudioCheck,
      ],
    },
    {
      id: 'runtime-environment',
      title: tr(t, 'settings.diagnostics.runtime_section', 'Runtime & Environment'),
      description: tr(
        t,
        'settings.diagnostics.runtime_section_description',
        'Check background runtime readiness and packaged environment dependencies.',
      ),
      checks: [
        checks.runtime.voiceTypingCheck,
        checks.runtime.ffmpegCheck,
        checks.runtime.logDirCheck,
      ],
    },
    {
      id: 'asr-performance',
      title: tr(t, 'settings.diagnostics.asr_performance_section', 'ASR Performance'),
      description: tr(
        t,
        'settings.diagnostics.asr_performance_section_description',
        'Review recent local ASR model memory and transcription latency samples.',
      ),
      checks: [
        checks.asr.modelMemoryCheck,
        checks.asr.liveLatencyCheck,
        checks.asr.batchLatencyCheck,
      ],
    },
  ];
}

export function buildDiagnosticsSnapshot(
  t: Translate,
  snapshot: DiagnosticsCoreFactsSnapshot,
): DiagnosticsSnapshot {
  const checks: BuiltChecks = {
    model: buildModelChecks(t, snapshot),
    input: buildInputChecks(t, snapshot),
    runtime: buildRuntimeChecks(t, snapshot),
    asr: buildAsrPerformanceChecks(t, snapshot.asrRuntimeMetrics),
  };

  return {
    scannedAt: snapshot.scannedAt,
    overview: buildOverviewCards(t, snapshot, checks),
    sections: buildSections(t, checks),
    runtimeEnvironment: snapshot.runtimeEnvironment,
  };
}
