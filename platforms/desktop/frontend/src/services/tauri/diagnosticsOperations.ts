import type {
  DiagnosticsCoreInput as CoreDiagnosticsInput,
  DiagnosticsCoreSnapshot as CoreDiagnosticsSnapshot,
} from '../../bindings';
import type { DiagnosticsCoreFactsSnapshot, DiagnosticsCoreInput } from '../../types/diagnostics';

export function buildDiagnosticsTransportInput(input: DiagnosticsCoreInput): CoreDiagnosticsInput {
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
    permissionState: snapshot.permissionState,
    voiceTypingReadiness: {
      state: snapshot.voiceTypingReadiness.state,
      lastErrorMessage: snapshot.voiceTypingReadiness.lastErrorMessage,
    },
  };
}
