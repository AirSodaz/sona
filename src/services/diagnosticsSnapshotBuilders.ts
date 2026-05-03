import type {
  DiagnosticAction,
  DiagnosticCheck,
  DiagnosticOverviewCard,
  DiagnosticSection,
  DiagnosticStatus,
} from '../types/diagnostics';
import type { SettingsTab } from '../hooks/useSettingsLogic';
import type { RuntimeEnvironmentStatus } from '../types/runtime';
import type {
  DeviceProbeResult,
  MicrophonePermissionState,
} from './audioDeviceService';
import type { VoiceTypingReadinessSnapshot } from '../hooks/useVoiceTypingReadiness';

export type Translate = (key: string, options?: Record<string, unknown>) => string;

export interface DiagnosticTextSpec {
  key: string;
  defaultValue: string;
  params?: Record<string, unknown>;
}

export interface DiagnosticActionSpec {
  kind: DiagnosticAction['kind'];
  label: DiagnosticTextSpec;
  settingsTab?: SettingsTab;
}

export interface DiagnosticCheckSpec {
  id: string;
  title: DiagnosticTextSpec;
  description: DiagnosticTextSpec;
  status: DiagnosticStatus;
  action?: DiagnosticActionSpec;
  meta?: string;
}

export interface DiagnosticSectionSpec {
  id: string;
  title: DiagnosticTextSpec;
  description?: DiagnosticTextSpec;
  checks: DiagnosticCheckSpec[];
}

export interface DiagnosticOverviewCardSpec {
  id: string;
  title: DiagnosticTextSpec;
  description: DiagnosticTextSpec;
  status: DiagnosticStatus;
  action?: DiagnosticActionSpec;
}

export interface DiagnosticsCoreInput {
  config: {
    streamingModelPath: string;
    offlineModelPath: string;
    vadModelPath: string;
    punctuationModelPath: string;
    microphoneId: string;
  };
  permissionState: MicrophonePermissionState;
  microphoneProbe: DeviceProbeResult;
  systemAudioProbe: DeviceProbeResult;
  voiceTypingReadiness: VoiceTypingReadinessSnapshot;
}

export interface DiagnosticsCoreSnapshot {
  scannedAt: string;
  overview: DiagnosticOverviewCardSpec[];
  sections: DiagnosticSectionSpec[];
  runtimeEnvironment: RuntimeEnvironmentStatus;
}

function hydrateText(t: Translate, spec: DiagnosticTextSpec): string {
  return t(spec.key, {
    defaultValue: spec.defaultValue,
    ...(spec.params ?? {}),
  });
}

function hydrateAction(t: Translate, action?: DiagnosticActionSpec): DiagnosticAction | undefined {
  if (!action) {
    return undefined;
  }

  const label = hydrateText(t, action.label);
  switch (action.kind) {
    case 'open_settings':
      return {
        kind: 'open_settings',
        label,
        settingsTab: action.settingsTab ?? 'models',
      } as DiagnosticAction;
    case 'request_microphone_permission':
      return { kind: 'request_microphone_permission', label };
    case 'retry_voice_typing_warmup':
      return { kind: 'retry_voice_typing_warmup', label };
    case 'run_first_run_setup':
      return { kind: 'run_first_run_setup', label };
    case 'open_log_folder':
      return { kind: 'open_log_folder', label };
    default:
      return undefined;
  }
}

function hydrateMeta(t: Translate, meta?: string): string | undefined {
  if (meta === 'settings.mic_auto') {
    return t('settings.mic_auto');
  }

  return meta;
}

function hydrateCheck(t: Translate, check: DiagnosticCheckSpec): DiagnosticCheck {
  return {
    id: check.id,
    title: hydrateText(t, check.title),
    description: hydrateText(t, check.description),
    status: check.status,
    action: hydrateAction(t, check.action),
    meta: hydrateMeta(t, check.meta),
  };
}

function hydrateSection(t: Translate, section: DiagnosticSectionSpec): DiagnosticSection {
  return {
    id: section.id,
    title: hydrateText(t, section.title),
    description: section.description ? hydrateText(t, section.description) : undefined,
    checks: section.checks.map((check) => hydrateCheck(t, check)),
  };
}

function hydrateOverviewCard(
  t: Translate,
  card: DiagnosticOverviewCardSpec,
): DiagnosticOverviewCard {
  return {
    id: card.id,
    title: hydrateText(t, card.title),
    description: hydrateText(t, card.description),
    status: card.status,
    action: hydrateAction(t, card.action),
  };
}

export function hydrateDiagnosticsCoreSnapshot(
  t: Translate,
  snapshot: DiagnosticsCoreSnapshot,
) {
  return {
    scannedAt: snapshot.scannedAt,
    overview: snapshot.overview.map((card) => hydrateOverviewCard(t, card)),
    sections: snapshot.sections.map((section) => hydrateSection(t, section)),
    runtimeEnvironment: snapshot.runtimeEnvironment,
  };
}
