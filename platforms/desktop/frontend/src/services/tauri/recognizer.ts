import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

export type InitRecognizerRequest = TauriCommandArgs<typeof TauriCommand.recognizer.init>;

export type ProcessBatchFileRequest =
  TauriCommandArgs<typeof TauriCommand.recognizer.processBatchFile>;

export async function initRecognizer(request: InitRecognizerRequest): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.init, request);
}

export async function startRecognizer(instanceId: string): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.start, { instanceId });
}

export async function stopRecognizer(instanceId: string): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.stop, { instanceId });
}

export async function flushRecognizer(instanceId: string): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.flush, { instanceId });
}

export async function feedAudioChunk(instanceId: string, samples: Uint8Array): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.feedAudioChunk, { instanceId, samples });
}

export async function processBatchFile(
  request: ProcessBatchFileRequest,
): Promise<TranscriptSegment[]> {
  return invokeTauri(TauriCommand.recognizer.processBatchFile, request);
}
