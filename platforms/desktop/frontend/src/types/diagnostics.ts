import type { RuntimeEnvironmentStatus } from './runtime';
import type { SettingsTab } from './settings';
import type { VoiceTypingReadinessSnapshot } from './voiceTyping';
import type {
  DeviceProbeInput,
  DiagnosticsConfigInput,
  DiagnosticsCoreSnapshot,
  ModelRuleInput,
  ModelRulesInput,
  ModelSummaryInput,
  PathStatusesInput,
  SelectedModelsInput,
  VoiceTypingReadinessInput,
} from '../bindings';

export interface DeviceOption {
  label: string;
  value: string;
}

export type MicrophonePermissionState = PermissionState | 'unsupported';

export interface DeviceProbeResult {
  options: DeviceOption[];
  available: boolean;
  source: 'native' | 'browser' | 'fallback';
  errorMessage?: string;
}

export type DiagnosticsConfigFacts = Required<DiagnosticsConfigInput>;
export type ModelSummaryFacts = ModelSummaryInput;
export type ModelRuleFacts = ModelRuleInput;
export type DiagnosticsSelectedModelsFacts = SelectedModelsInput;
export type DiagnosticsModelRulesFacts = ModelRulesInput;
export type DiagnosticsPathStatusesFacts = PathStatusesInput;
export type DeviceProbeFacts = DeviceProbeInput;
export type VoiceTypingReadinessFacts = Omit<VoiceTypingReadinessInput, 'state'> & {
  state: VoiceTypingReadinessSnapshot['state'];
};

export interface DiagnosticsCoreInput {
  config: DiagnosticsConfigFacts;
  permissionState: MicrophonePermissionState;
  microphoneProbe: DeviceProbeResult;
  systemAudioProbe: DeviceProbeResult;
  voiceTypingReadiness: VoiceTypingReadinessSnapshot;
}

export type DiagnosticsCoreFactsSnapshot = Omit<
  DiagnosticsCoreSnapshot,
  'config' | 'permissionState' | 'voiceTypingReadiness'
> & {
  config: DiagnosticsConfigFacts;
  permissionState: MicrophonePermissionState;
  voiceTypingReadiness: VoiceTypingReadinessFacts;
};

export type DiagnosticStatus = 'ready' | 'warning' | 'missing' | 'failed' | 'info';

export type DiagnosticAction =
  | {
      kind: 'open_settings';
      label: string;
      settingsTab: SettingsTab;
    }
  | {
      kind: 'request_microphone_permission';
      label: string;
    }
  | {
      kind: 'retry_voice_typing_warmup';
      label: string;
    }
  | {
      kind: 'run_first_run_setup';
      label: string;
    }
  | {
      kind: 'open_log_folder';
      label: string;
    };

export interface DiagnosticCheck {
  id: string;
  title: string;
  description: string;
  status: DiagnosticStatus;
  action?: DiagnosticAction;
  meta?: string;
}

export interface DiagnosticSection {
  id: string;
  title: string;
  description?: string;
  checks: DiagnosticCheck[];
}

export interface DiagnosticOverviewCard {
  id: string;
  title: string;
  description: string;
  status: DiagnosticStatus;
  action?: DiagnosticAction;
}

export interface DiagnosticsSnapshot {
  scannedAt: string;
  overview: DiagnosticOverviewCard[];
  sections: DiagnosticSection[];
  runtimeEnvironment: RuntimeEnvironmentStatus;
}

export type { RuntimeEnvironmentStatus } from './runtime';
