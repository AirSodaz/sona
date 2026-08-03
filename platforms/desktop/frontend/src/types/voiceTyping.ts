export type VoiceTypingRuntimeLifecycleStatus = 'idle' | 'preparing' | 'ready' | 'error';

export type VoiceTypingRuntimeErrorSource =
  | 'shortcut_registration'
  | 'warmup'
  | 'microphone'
  | 'session';

export interface VoiceTypingRuntimeStatus {
  shortcutRegistration: Exclude<VoiceTypingRuntimeLifecycleStatus, 'preparing'>;
  warmup: VoiceTypingRuntimeLifecycleStatus;
  lastErrorSource: VoiceTypingRuntimeErrorSource | null;
  lastErrorMessage: string | null;
  updatedAt: number | null;
}

export type VoiceTypingReadinessState =
  | 'off'
  | 'needs_shortcut'
  | 'needs_live_model'
  | 'needs_vad'
  | 'failed'
  | 'preparing'
  | 'ready';

export interface VoiceTypingReadinessSnapshot {
  state: VoiceTypingReadinessState;
  shortcutConfigured: boolean;
  liveModelConfigured: boolean;
  requiresVad: boolean;
  vadConfigured: boolean;
  shortcutRegistration: 'idle' | 'ready' | 'error';
  warmup: 'idle' | 'preparing' | 'ready' | 'error';
  inputDeviceState: 'off' | 'ready' | 'failed';
  runtimeState: 'off' | 'preparing' | 'ready' | 'failed';
  lastErrorSource: VoiceTypingRuntimeErrorSource | null;
  lastErrorMessage: string | null;
}
