import { modelService } from './modelService';
import type { AppConfig } from '../types/config';
import type {
  VoiceTypingReadinessSnapshot,
  VoiceTypingReadinessState,
  VoiceTypingRuntimeStatus,
} from '../types/voiceTyping';
import { findSelectedModelByMode } from '../utils/modelSelection';
import { getScenarioVadModelPath } from '../utils/scenarioModels';

export function resolveVoiceTypingReadinessSnapshot(
  config: Pick<
    AppConfig,
    'voiceTypingEnabled' | 'voiceTypingShortcut' | 'streamingModelPath' | 'liveVadModelPath' | 'microphoneId'
  >,
  runtime: VoiceTypingRuntimeStatus,
): VoiceTypingReadinessSnapshot {
  const shortcutConfigured = (config.voiceTypingShortcut ?? '').trim().length > 0;
  const liveModelConfigured = (config.streamingModelPath ?? '').trim().length > 0;
  const selectedStreamingModel = liveModelConfigured
    ? findSelectedModelByMode(config.streamingModelPath ?? '', 'streaming')
    : null;
  const requiresVad = selectedStreamingModel
    ? modelService.getModelRules(selectedStreamingModel.id).requiresVad
    : false;
  const vadConfigured = !requiresVad || getScenarioVadModelPath(config, 'live').length > 0;
  const hasRuntimeFailure =
    runtime.shortcutRegistration === 'error'
    || runtime.warmup === 'error'
    || runtime.lastErrorSource !== null;

  let state: VoiceTypingReadinessState;
  if (!config.voiceTypingEnabled) {
    state = 'off';
  } else if (!shortcutConfigured) {
    state = 'needs_shortcut';
  } else if (!liveModelConfigured) {
    state = 'needs_live_model';
  } else if (!vadConfigured) {
    state = 'needs_vad';
  } else if (hasRuntimeFailure) {
    state = 'failed';
  } else if (
    runtime.shortcutRegistration !== 'ready'
    || runtime.warmup !== 'ready'
  ) {
    state = 'preparing';
  } else {
    state = 'ready';
  }

  return {
    state,
    shortcutConfigured,
    liveModelConfigured,
    requiresVad,
    vadConfigured,
    shortcutRegistration: runtime.shortcutRegistration,
    warmup: runtime.warmup,
    inputDeviceState: !config.voiceTypingEnabled
      ? 'off'
      : runtime.lastErrorSource === 'microphone'
        ? 'failed'
        : 'ready',
    runtimeState: !config.voiceTypingEnabled
      ? 'off'
      : state === 'failed'
        ? 'failed'
        : state === 'ready'
          ? 'ready'
          : 'preparing',
    lastErrorSource: runtime.lastErrorSource,
    lastErrorMessage: runtime.lastErrorMessage,
  };
}
