import type {
  AsrTranscriptionRequest_Serialize as CoreAsrTranscriptionRequest,
  ModelFileConfig as CoreModelFileConfig,
  SpeakerProcessingConfig as CoreSpeakerProcessingConfig,
} from '../../bindings';
import type { AsrTranscriptionRequest } from '../asrConfigService';
import type { SpeakerProcessingConfig } from '../../types/speaker';
import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

type CoreInitRecognizerRequest = TauriCommandArgs<typeof TauriCommand.recognizer.init>;
type CoreProcessBatchFileRequest =
  TauriCommandArgs<typeof TauriCommand.recognizer.processBatchFile>;

export type InitRecognizerRequest = Omit<CoreInitRecognizerRequest, 'asrRequest'> & {
  asrRequest: AsrTranscriptionRequest;
};

export type ProcessBatchFileRequest = Omit<
  CoreProcessBatchFileRequest,
  'asrRequest' | 'speakerProcessing'
> & {
  asrRequest: AsrTranscriptionRequest;
  speakerProcessing: SpeakerProcessingConfig | null;
};

function finiteNumber(value: number, path: string): number {
  if (!Number.isFinite(value)) {
    throw new TypeError(`${path} must be a finite number`);
  }
  return value;
}

function normalizeModelFileConfig(
  config: Extract<AsrTranscriptionRequest, { engine: 'local-sherpa' }>['fileConfig'],
): CoreModelFileConfig | undefined {
  if (!config) {
    return undefined;
  }

  return {
    encoder: config.encoder ?? null,
    decoder: config.decoder ?? null,
    model: config.model ?? null,
    joiner: config.joiner ?? null,
    tokens: config.tokens ?? null,
    convFrontend: config.convFrontend ?? null,
    encoderAdaptor: config.encoderAdaptor ?? null,
    llm: config.llm ?? null,
    embedding: config.embedding ?? null,
    tokenizer: config.tokenizer ?? null,
  };
}

function normalizeAsrRequest(
  request: AsrTranscriptionRequest,
): CoreAsrTranscriptionRequest {
  const common = {
    mode: request.mode,
    language: request.language,
    enableItn: request.enableItn,
    normalizationOptions: request.normalizationOptions,
    postprocessOptions: request.postprocessOptions,
    hotwords: request.hotwords,
    speakerProcessing: null,
  };

  if (request.engine === 'online') {
    return {
      ...common,
      engine: 'online',
      onlineProvider: request.onlineProvider,
    };
  }

  const fileConfig = normalizeModelFileConfig(request.fileConfig);
  return {
    ...common,
    engine: 'local-sherpa',
    ...(request.modelId !== undefined ? { modelId: request.modelId } : {}),
    modelPath: request.modelPath,
    numThreads: request.numThreads,
    punctuationModel: request.punctuationModel,
    vadModel: request.vadModel,
    vadBuffer: finiteNumber(request.vadBuffer, 'asrRequest.vadBuffer'),
    ...(request.batchSegmentationMode !== undefined
      ? { batchSegmentationMode: request.batchSegmentationMode }
      : {}),
    modelType: request.modelType,
    ...(fileConfig ? { fileConfig } : {}),
    ...(request.gpuAcceleration !== undefined
      ? { gpuAcceleration: request.gpuAcceleration }
      : {}),
  };
}

function normalizeSpeakerProcessing(
  config: SpeakerProcessingConfig | null,
): CoreSpeakerProcessingConfig | null {
  if (!config) {
    return null;
  }

  return {
    speakerSegmentationModelPath: config.speakerSegmentationModelPath ?? null,
    speakerEmbeddingModelPath: config.speakerEmbeddingModelPath ?? null,
    speakerProfiles: config.speakerProfiles?.map((profile) => ({
      ...profile,
      samples: profile.samples.map((sample) => ({
        ...sample,
        durationSeconds: finiteNumber(
          sample.durationSeconds,
          `speakerProcessing.speakerProfiles.${profile.id}.samples.${sample.id}.durationSeconds`,
        ),
      })),
    })) ?? null,
  };
}

export async function initRecognizer(request: InitRecognizerRequest): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.init, {
    ...request,
    asrRequest: normalizeAsrRequest(request.asrRequest),
  });
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
  return invokeTauri(TauriCommand.recognizer.processBatchFile, {
    ...request,
    speakerProcessing: normalizeSpeakerProcessing(request.speakerProcessing),
    asrRequest: normalizeAsrRequest(request.asrRequest),
  });
}
