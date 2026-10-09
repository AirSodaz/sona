import type { DiagnosticsCoreSnapshot as CoreDiagnosticsSnapshot } from '../bindings';
import { getPlatform, TauriCommand } from '../platform';
import type { DiagnosticsCoreFactsSnapshot, DiagnosticsCoreInput } from '../types/diagnostics';
import {
  buildDiagnosticsTransportInput,
  normalizeDiagnosticsSnapshot,
} from './tauri/diagnosticsOperations';

export { buildDiagnosticsTransportInput, normalizeDiagnosticsSnapshot };

export async function getDiagnosticsCoreSnapshot(
  input: DiagnosticsCoreInput
): Promise<DiagnosticsCoreFactsSnapshot> {
  const snapshot = await getPlatform().transport.invoke<CoreDiagnosticsSnapshot>(
    TauriCommand.app.getDiagnosticsCoreSnapshot,
    { input: buildDiagnosticsTransportInput(input) }
  );
  return normalizeDiagnosticsSnapshot(snapshot);
}
