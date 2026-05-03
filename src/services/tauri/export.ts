import { TauriCommand } from './commands';
import type { TauriCommandArgs, TauriCommandResult } from './contracts';
import { invokeTauri } from './invoke';

export type ExportTranscriptFileRequest = TauriCommandArgs<
  typeof TauriCommand.export.transcriptFile
>;
export type ExportTranscriptFileResult = TauriCommandResult<
  typeof TauriCommand.export.transcriptFile
>;

export async function exportTranscriptFile(
  request: ExportTranscriptFileRequest,
): Promise<ExportTranscriptFileResult> {
  return invokeTauri(TauriCommand.export.transcriptFile, request);
}
