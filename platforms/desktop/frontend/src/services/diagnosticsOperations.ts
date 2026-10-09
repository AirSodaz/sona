import type {
  DiagnosticsCoreInput as CoreDiagnosticsInput,
  DiagnosticsCoreSnapshot as CoreDiagnosticsSnapshot,
} from '../bindings';
import { getPlatform, TauriCommand } from '../platform';
import type { DiagnosticsCoreFactsSnapshot, DiagnosticsCoreInput } from '../types/diagnostics';

function buildDiagnosticsTransportInput(input: DiagnosticsCoreInput): CoreDiagnosticsInput {
  const normalizeProbe = (probe: DiagnosticsCoreInput['microphoneProbe']) => ({
    options: probe.options.map(({ label, value }) => ({ label, value })),
    available: probe.available,
    errorMessage: probe.errorMessage ?? null,
  });

  return {
    config: input.config,
    permissionState: input.permissionState,
    microphoneProbe: normalizeProbe(input.microphoneProbe),
    systemAudioProbe: normalizeProbe(input.systemAudioProbe),
    voiceTypingReadiness: {
      state: input.voiceTypingReadiness.state,
      lastErrorMessage: input.voiceTypingReadiness.lastErrorMessage,
    },
  };
}

function normalizePermissionState(value: string): DiagnosticsCoreFactsSnapshot['permissionState'] {
  switch (value) {
    case 'denied':
    case 'granted':
    case 'prompt':
    case 'unsupported':
      return value;
    default:
      throw new Error(`Unexpected diagnostics permission state: ${value}`);
  }
}

function normalizeVoiceTypingState(
  value: string
): DiagnosticsCoreFactsSnapshot['voiceTypingReadiness']['state'] {
  switch (value) {
    case 'off':
    case 'needs_shortcut':
    case 'needs_live_model':
    case 'needs_vad':
    case 'failed':
    case 'preparing':
    case 'ready':
      return value;
    default:
      throw new Error(`Unexpected diagnostics voice typing state: ${value}`);
  }
}

export function normalizeDiagnosticsSnapshot(
  snapshot: CoreDiagnosticsSnapshot
): DiagnosticsCoreFactsSnapshot {
  return {
    ...snapshot,
    config: {
      streamingModelPath: snapshot.config.streamingModelPath,
      batchModelPath: snapshot.config.batchModelPath,
      vadModelPath: snapshot.config.vadModelPath ?? '',
      punctuationModelPath: snapshot.config.punctuationModelPath ?? '',
      microphoneId: snapshot.config.microphoneId ?? 'default',
      ffmpegEnabled: snapshot.config.ffmpegEnabled ?? false,
      ffmpegPath: snapshot.config.ffmpegPath ?? '',
    },
    permissionState: normalizePermissionState(snapshot.permissionState),
    voiceTypingReadiness: {
      state: normalizeVoiceTypingState(snapshot.voiceTypingReadiness.state),
      lastErrorMessage: snapshot.voiceTypingReadiness.lastErrorMessage,
    },
  };
}

export async function getDiagnosticsCoreSnapshot(
  input: DiagnosticsCoreInput
): Promise<DiagnosticsCoreFactsSnapshot> {
  const snapshot = await getPlatform().transport.invoke<CoreDiagnosticsSnapshot>(
    TauriCommand.app.getDiagnosticsCoreSnapshot,
    { input: buildDiagnosticsTransportInput(input) }
  );
  return normalizeDiagnosticsSnapshot(snapshot);
}
