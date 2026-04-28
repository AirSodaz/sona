import { invoke } from '@tauri-apps/api/core';
import { getResumeOnboardingStep, hasRequiredOnboardingModels } from '../utils/onboarding';
import {
  getMicrophonePermissionState,
  probeMicrophoneDeviceOptions,
  probeSystemAudioDeviceOptions,
  type MicrophonePermissionState,
} from './audioDeviceService';
import { useConfigStore } from '../stores/configStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import { findSelectedModelByMode } from '../utils/modelSelection';
import { modelService } from './modelService';
import { getPathStatusMap, isRuntimePathAvailable } from './pathStatusService';
import { resolveVoiceTypingReadinessSnapshot } from '../hooks/useVoiceTypingReadiness';
import type {
  DiagnosticAction,
  DiagnosticCheck,
  DiagnosticOverviewCard,
  DiagnosticSection,
  DiagnosticStatus,
  DiagnosticsSnapshot,
} from '../types/diagnostics';
import type { SettingsTab } from '../hooks/useSettingsLogic';
import type { RuntimeEnvironmentStatus, RuntimePathStatus } from '../types/runtime';

type Translate = (key: string, options?: Record<string, unknown>) => string;

const STATUS_PRIORITY: Record<DiagnosticStatus, number> = {
  failed: 4,
  missing: 3,
  warning: 2,
  info: 1,
  ready: 0,
};

function pickWorseStatus(...statuses: DiagnosticStatus[]): DiagnosticStatus {
  return statuses.reduce((worst, status) => (
    STATUS_PRIORITY[status] > STATUS_PRIORITY[worst] ? status : worst
  ), 'ready' as DiagnosticStatus);
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
        action: {
          kind: 'request_microphone_permission',
          label: t('settings.diagnostics.request_permission', {
            defaultValue: 'Request Permission',
          }),
        },
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
        action: {
          kind: 'request_microphone_permission',
          label: t('settings.diagnostics.request_permission', {
            defaultValue: 'Request Permission',
          }),
        },
      };
  }
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

function buildPathUnverifiedDescription(t: Translate): string {
  return t('settings.diagnostics.model_path_unverified', {
    defaultValue: 'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
  });
}

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
      invoke<RuntimeEnvironmentStatus>('get_runtime_environment_status'),
      getPathStatusMap([streamingModelPath, offlineModelPath, vadModelPath, punctuationModelPath]),
    ]);

    const liveModel = findSelectedModelByMode(config.streamingModelPath, 'streaming');
    const offlineModel = findSelectedModelByMode(config.offlineModelPath, 'offline');
    const liveModelPathStatus = pathStatusMap[streamingModelPath];
    const offlineModelPathStatus = pathStatusMap[offlineModelPath];
    const vadPathStatus = pathStatusMap[vadModelPath];
    const punctuationPathStatus = pathStatusMap[punctuationModelPath];

    const liveModelCheck: DiagnosticCheck = !config.streamingModelPath.trim()
      ? {
          id: 'live-model',
          title: t('settings.diagnostics.live_model_title', { defaultValue: 'Live Record Model' }),
          status: 'missing',
          description: t('settings.diagnostics.live_model_missing', {
            defaultValue: 'No Live Record Model is selected yet.',
          }),
          action: buildOpenSettingsAction(
            t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
            'models',
          ),
        }
      : isRuntimePathMissing(liveModelPathStatus)
        ? {
            id: 'live-model',
            title: t('settings.diagnostics.live_model_title', { defaultValue: 'Live Record Model' }),
            status: 'failed',
            description: t('settings.diagnostics.model_path_missing', {
              defaultValue: 'The selected model path no longer exists on disk.',
            }),
            meta: config.streamingModelPath,
            action: buildOpenSettingsAction(
              t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
              'models',
            ),
          }
        : isRuntimePathUnknown(liveModelPathStatus)
          ? {
              id: 'live-model',
              title: t('settings.diagnostics.live_model_title', { defaultValue: 'Live Record Model' }),
              status: 'info',
              description: buildPathUnverifiedDescription(t),
              meta: streamingModelPath,
              action: buildOpenSettingsAction(
                t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                'models',
              ),
            }
        : {
            id: 'live-model',
            title: t('settings.diagnostics.live_model_title', { defaultValue: 'Live Record Model' }),
            status: 'ready',
            description: t('settings.diagnostics.model_ready', {
              defaultValue: 'The selected model is configured and reachable.',
            }),
            meta: liveModel?.name ?? config.streamingModelPath,
          };

    const offlineModelCheck: DiagnosticCheck = !config.offlineModelPath.trim()
      ? {
          id: 'offline-model',
          title: t('settings.diagnostics.offline_model_title', { defaultValue: 'Batch Import Model' }),
          status: 'missing',
          description: t('settings.diagnostics.offline_model_missing', {
            defaultValue: 'No Batch Import Model is selected yet.',
          }),
          action: buildOpenSettingsAction(
            t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
            'models',
          ),
        }
      : isRuntimePathMissing(offlineModelPathStatus)
        ? {
            id: 'offline-model',
            title: t('settings.diagnostics.offline_model_title', { defaultValue: 'Batch Import Model' }),
            status: 'failed',
            description: t('settings.diagnostics.model_path_missing', {
              defaultValue: 'The selected model path no longer exists on disk.',
            }),
            meta: config.offlineModelPath,
            action: buildOpenSettingsAction(
              t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
              'models',
            ),
          }
        : isRuntimePathUnknown(offlineModelPathStatus)
          ? {
              id: 'offline-model',
              title: t('settings.diagnostics.offline_model_title', { defaultValue: 'Batch Import Model' }),
              status: 'info',
              description: buildPathUnverifiedDescription(t),
              meta: offlineModelPath,
              action: buildOpenSettingsAction(
                t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                'models',
              ),
            }
        : {
            id: 'offline-model',
            title: t('settings.diagnostics.offline_model_title', { defaultValue: 'Batch Import Model' }),
            status: 'ready',
            description: t('settings.diagnostics.model_ready', {
              defaultValue: 'The selected model is configured and reachable.',
            }),
            meta: offlineModel?.name ?? config.offlineModelPath,
          };

    const liveModelRules = liveModel ? modelService.getModelRules(liveModel.id) : null;
    const vadCheck: DiagnosticCheck = !liveModel
      ? {
          id: 'vad',
          title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
          status: 'info',
          description: t('settings.diagnostics.vad_unknown', {
            defaultValue: 'Pick a Live Record Model first to evaluate whether a VAD model is required.',
          }),
          action: buildOpenSettingsAction(
            t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
            'models',
          ),
        }
      : !liveModelRules?.requiresVad
        ? {
            id: 'vad',
            title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
            status: 'ready',
            description: t('settings.diagnostics.vad_not_required', {
              defaultValue: 'The selected Live Record Model does not require a separate VAD model.',
            }),
          }
        : !config.vadModelPath?.trim()
          ? {
              id: 'vad',
              title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
              status: 'missing',
              description: t('settings.diagnostics.vad_missing', {
                defaultValue: 'The selected Live Record Model still needs a VAD model.',
              }),
              action: buildOpenSettingsAction(
                t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                'models',
              ),
            }
          : isRuntimePathMissing(vadPathStatus)
            ? {
                id: 'vad',
                title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
                status: 'failed',
                description: t('settings.diagnostics.model_path_missing', {
                  defaultValue: 'The selected model path no longer exists on disk.',
                }),
                meta: config.vadModelPath,
                action: buildOpenSettingsAction(
                  t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                  'models',
                ),
              }
            : isRuntimePathUnknown(vadPathStatus)
              ? {
                  id: 'vad',
                  title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
                  status: 'info',
                  description: buildPathUnverifiedDescription(t),
                  meta: vadModelPath,
                  action: buildOpenSettingsAction(
                    t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                    'models',
                  ),
                }
            : {
                id: 'vad',
                title: t('settings.diagnostics.vad_title', { defaultValue: 'VAD Dependency' }),
                status: 'ready',
                description: t('settings.diagnostics.vad_ready', {
                  defaultValue: 'The required VAD model is configured and reachable.',
                }),
              };

    const punctuationRequired = [liveModel, offlineModel]
      .filter((model): model is NonNullable<typeof model> => !!model)
      .some((model) => modelService.getModelRules(model.id).requiresPunctuation);
    const punctuationCheck: DiagnosticCheck = !punctuationRequired
      ? {
          id: 'punctuation',
          title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
          status: 'ready',
          description: t('settings.diagnostics.punctuation_not_required', {
            defaultValue: 'The current recognition models do not require a separate punctuation model.',
          }),
        }
      : config.punctuationModelPath?.trim() && isRuntimePathAvailable(punctuationPathStatus)
        ? {
            id: 'punctuation',
            title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
            status: 'ready',
            description: t('settings.diagnostics.punctuation_ready', {
              defaultValue: 'The required punctuation model is configured and reachable.',
            }),
          }
        : config.punctuationModelPath?.trim() && isRuntimePathMissing(punctuationPathStatus)
          ? {
              id: 'punctuation',
              title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
              status: 'warning',
              description: t('settings.diagnostics.model_path_missing', {
                defaultValue: 'The selected model path no longer exists on disk.',
              }),
              meta: punctuationModelPath,
              action: buildOpenSettingsAction(
                t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                'models',
              ),
            }
          : config.punctuationModelPath?.trim() && isRuntimePathUnknown(punctuationPathStatus)
            ? {
                id: 'punctuation',
                title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
                status: 'warning',
                description: buildPathUnverifiedDescription(t),
                meta: punctuationModelPath,
                action: buildOpenSettingsAction(
                  t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
                  'models',
                ),
              }
        : {
            id: 'punctuation',
            title: t('settings.diagnostics.punctuation_title', { defaultValue: 'Punctuation Dependency' }),
            status: 'warning',
            description: t('settings.diagnostics.punctuation_warning', {
              defaultValue: 'A selected recognition model expects a punctuation model, but none is available yet.',
            }),
            action: buildOpenSettingsAction(
              t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
              'models',
            ),
          };

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
          action: buildOpenSettingsAction(
            t('settings.diagnostics.open_input_device', { defaultValue: 'Open Input Device' }),
            'microphone',
          ),
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
            action: buildOpenSettingsAction(
              t('settings.diagnostics.open_input_device', { defaultValue: 'Open Input Device' }),
              'microphone',
            ),
          };

    const systemAudioCheck: DiagnosticCheck = systemAudioProbe.available
      ? {
          id: 'system-audio',
          title: t('settings.diagnostics.system_audio_title', { defaultValue: 'System Audio Capture' }),
          status: 'ready',
          description: t('settings.diagnostics.system_audio_ready', {
            defaultValue: 'System audio capture devices are available.',
          }),
        }
      : {
          id: 'system-audio',
          title: t('settings.diagnostics.system_audio_title', { defaultValue: 'System Audio Capture' }),
          status: 'warning',
          description: systemAudioProbe.errorMessage || t('settings.diagnostics.system_audio_warning', {
            defaultValue: 'Sona could not enumerate system audio capture devices right now.',
          }),
          action: buildOpenSettingsAction(
            t('settings.diagnostics.open_input_device', { defaultValue: 'Open Input Device' }),
            'microphone',
          ),
        };

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

    const voiceTypingCheck: DiagnosticCheck = (() => {
      switch (voiceTypingReadiness.state) {
        case 'off':
          return {
            id: 'voice-typing',
            title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
            status: 'info',
            description: t('settings.diagnostics.voice_typing_off', {
              defaultValue: 'Voice Typing is currently turned off.',
            }),
            action: buildOpenSettingsAction(
              t('settings.diagnostics.open_voice_typing', { defaultValue: 'Open Voice Typing' }),
              'voice_typing',
            ),
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
            action: buildOpenSettingsAction(
              t('settings.diagnostics.open_voice_typing', { defaultValue: 'Open Voice Typing' }),
              'voice_typing',
            ),
          };
        case 'failed':
          return {
            id: 'voice-typing',
            title: t('settings.diagnostics.voice_typing_title', { defaultValue: 'Voice Typing Runtime' }),
            status: 'failed',
            description: voiceTypingReadiness.lastErrorMessage || t('settings.voice_typing_status_summary_failed', {
              defaultValue: 'Voice Typing hit a runtime problem.',
            }),
            action: {
              kind: 'retry_voice_typing_warmup',
              label: t('settings.diagnostics.retry_warmup', { defaultValue: 'Retry Warm-up' }),
            },
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
    })();

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
          action: {
            kind: 'open_log_folder',
            label: t('settings.about_open_logs', { defaultValue: 'Open Log Folder' }),
          },
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
          action: {
            kind: 'open_log_folder',
            label: t('settings.about_open_logs', { defaultValue: 'Open Log Folder' }),
          },
        }
      : {
          id: 'log-dir',
          title: t('settings.diagnostics.log_dir_title', { defaultValue: 'Log Directory' }),
          status: 'failed',
          description: t('settings.diagnostics.log_dir_missing', {
            defaultValue: 'Sona could not resolve the runtime log directory.',
          }),
        };

    const sections: DiagnosticSection[] = [
      {
        id: 'models',
        title: t('settings.diagnostics.models_section', { defaultValue: 'Models' }),
        description: t('settings.diagnostics.models_section_description', {
          defaultValue: 'Check that local transcription models and required dependencies are present.',
        }),
        checks: [liveModelCheck, offlineModelCheck, vadCheck, punctuationCheck],
      },
      {
        id: 'input-capture',
        title: t('settings.diagnostics.input_section', { defaultValue: 'Input & Capture' }),
        description: t('settings.diagnostics.input_section_description', {
          defaultValue: 'Check permissions and the availability of input or capture devices.',
        }),
        checks: [permissionCheck, microphoneCheck, systemAudioCheck],
      },
      {
        id: 'runtime-environment',
        title: t('settings.diagnostics.runtime_section', { defaultValue: 'Runtime & Environment' }),
        description: t('settings.diagnostics.runtime_section_description', {
          defaultValue: 'Check background runtime readiness and packaged environment dependencies.',
        }),
        checks: [voiceTypingCheck, ffmpegCheck, logDirCheck],
      },
    ];

    const onboardingAction: DiagnosticAction = {
      kind: 'run_first_run_setup',
      label: t('settings.diagnostics.run_first_setup', { defaultValue: 'Run First Run Setup' }),
    };
    const liveRecordOverviewAction = (() => {
      if (liveModelCheck.status !== 'ready' || vadCheck.status !== 'ready') {
        return buildOpenSettingsAction(
          t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
          'models',
        );
      }

      if (permissionCheck.action) {
        return permissionCheck.action;
      }

      if (microphoneCheck.action) {
        return buildOpenSettingsAction(
          t('settings.diagnostics.open_input_device', { defaultValue: 'Open Input Device' }),
          'microphone',
        );
      }

      return undefined;
    })();
    const batchImportOverviewAction = (() => {
      if (offlineModelCheck.status !== 'ready' || punctuationCheck.status === 'warning') {
        return buildOpenSettingsAction(
          t('settings.diagnostics.open_model_settings', { defaultValue: 'Open Model Settings' }),
          'models',
        );
      }

      if (!runtimeEnvironment.ffmpegExists) {
        return {
          kind: 'open_log_folder' as const,
          label: t('settings.about_open_logs', { defaultValue: 'Open Log Folder' }),
        };
      }

      return undefined;
    })();

    const overview: DiagnosticOverviewCard[] = [
      buildOverviewCard(
        'first-run',
        t('settings.diagnostics.first_run_card', { defaultValue: 'First Run Setup' }),
        hasRequiredOnboardingModels(config)
          ? t('settings.diagnostics.first_run_ready', {
              defaultValue: 'Recommended local models are configured.',
            })
          : t('settings.diagnostics.first_run_missing', {
              defaultValue: 'The recommended offline setup is still incomplete.',
            }),
        [
          hasRequiredOnboardingModels(config) ? 'ready' : 'missing',
          permissionCheck.status === 'failed' ? 'warning' : permissionCheck.status,
        ],
        hasRequiredOnboardingModels(config) ? undefined : onboardingAction,
      ),
      buildOverviewCard(
        'live-record',
        t('settings.diagnostics.live_record_card', { defaultValue: 'Live Record' }),
        t('settings.diagnostics.live_record_card_description', {
          defaultValue: 'Model, VAD, permission, and microphone selection for real-time capture.',
        }),
        [liveModelCheck.status, vadCheck.status, permissionCheck.status, microphoneCheck.status],
        liveRecordOverviewAction,
      ),
      buildOverviewCard(
        'batch-import',
        t('settings.diagnostics.batch_import_card', { defaultValue: 'Batch Import' }),
        t('settings.diagnostics.batch_import_card_description', {
          defaultValue: 'Offline model and bundled media decoding support for file processing.',
        }),
        [offlineModelCheck.status, punctuationCheck.status, ffmpegCheck.status],
        batchImportOverviewAction,
      ),
      buildOverviewCard(
        'voice-typing',
        t('settings.diagnostics.voice_typing_card', { defaultValue: 'Voice Typing' }),
        t('settings.diagnostics.voice_typing_card_description', {
          defaultValue: 'Shortcut, live model reuse, and runtime warm-up for dictation.',
        }),
        [voiceTypingCheck.status],
        voiceTypingCheck.action,
      ),
    ];

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
