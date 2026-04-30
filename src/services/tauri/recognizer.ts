import type { SpeakerProcessingConfig } from '../../types/speaker';
import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface InitRecognizerRequest {
  instanceId: string;
  modelPath: string;
  numThreads: number;
  enableItn: boolean;
  language: string;
  punctuationModel: string | null;
  vadModel: string | null;
  vadBuffer: number;
  modelType: string;
  fileConfig?: Record<string, string | undefined>;
  hotwords: string | null;
  normalizationOptions: {
    enableTimeline: boolean;
  };
}

export interface ProcessBatchFileRequest {
  filePath: string;
  saveToPath: string | null;
  modelPath: string;
  numThreads: number;
  enableItn: boolean;
  language: string;
  punctuationModel: string | null;
  vadModel: string | null;
  vadBuffer: number;
  modelType: string;
  fileConfig?: Record<string, string | undefined>;
  hotwords: string | null;
  speakerProcessing: SpeakerProcessingConfig | null;
  normalizationOptions: {
    enableTimeline: boolean;
  };
}

export async function initRecognizer(request: InitRecognizerRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.recognizer.init, request);
}

export async function startRecognizer(instanceId: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.recognizer.start, { instanceId });
}

export async function stopRecognizer(instanceId: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.recognizer.stop, { instanceId });
}

export async function flushRecognizer(instanceId: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.recognizer.flush, { instanceId });
}

export async function feedAudioChunk(instanceId: string, samples: Uint8Array): Promise<void> {
  await invokeTauri<void>(TauriCommand.recognizer.feedAudioChunk, { instanceId, samples });
}

export async function processBatchFile(
  request: ProcessBatchFileRequest,
): Promise<TranscriptSegment[]> {
  return invokeTauri<TranscriptSegment[]>(TauriCommand.recognizer.processBatchFile, request);
}
