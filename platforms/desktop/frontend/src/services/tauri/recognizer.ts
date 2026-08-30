import type {
  AsrTranscriptionRequest_Serialize as CoreAsrTranscriptionRequest,
  ModelFileConfig as CoreModelFileConfig,
  SpeakerProcessingConfig as CoreSpeakerProcessingConfig,
} from '../../bindings';
import type { AsrTranscriptionRequest } from '../../types/asr';
import type { SpeakerProcessingConfig } from '../../types/speaker';
import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

type CoreProcessBatchFileRequest =
  TauriCommandArgs<typeof TauriCommand.recognizer.processBatchFile>;

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
  config: Extract<AsrTranscriptionRequest, { engine: 'local' }>['fileConfig'],
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
    mmproj: config.mmproj ?? null,
    preprocessor: config.preprocessor ?? null,
    uncachedDecoder: config.uncachedDecoder ?? null,
    cachedDecoder: config.cachedDecoder ?? null,
    mergedDecoder: config.mergedDecoder ?? null,
  };
}

export function normalizeAsrRequest(
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
    engine: 'local',
    localEngine: request.localEngine ?? 'sherpa-onnx',
    modelId: request.modelId ?? null,
    modelPath: request.modelPath,
    numThreads: request.numThreads,
    punctuationModel: request.punctuationModel,
    vadModel: request.vadModel,
    vadBuffer: finiteNumber(request.vadBuffer, 'asrRequest.vadBuffer'),
    batchSegmentationMode: request.batchSegmentationMode ?? 'vad',
    modelType: request.modelType,
    fileConfig: fileConfig ?? null,
    gpuAcceleration: request.gpuAcceleration ?? null,
    initialRefreshRateMs: request.initialRefreshRateMs ?? null,
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

export async function prepareLiveTranscription(
  asrRequest: AsrTranscriptionRequest,
): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.prepareLive, {
    asrRequest: normalizeAsrRequest(asrRequest),
  });
}

export async function createExternalLiveSource() {
  return invokeTauri(TauriCommand.recognizer.createExternalSource);
}

export async function startExternalLiveTranscription(request: {
  consumerId: string;
  sourceToken: string;
  gain: number;
  asrRequest: AsrTranscriptionRequest;
}) {
  return invokeTauri(TauriCommand.recognizer.startExternalLive, {
    ...request,
    asrRequest: normalizeAsrRequest(request.asrRequest),
  });
}

export async function feedExternalLiveSource(
  sourceToken: string,
  samples: Uint8Array,
): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.feedExternalSource, { sourceToken, samples });
}

export async function retireExternalLiveSource(sourceToken: string): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.retireExternalSource, { sourceToken });
}

export interface StartNativeLiveTranscriptionRequest {
  consumerId: string;
  sourceKind: 'system' | 'microphone';
  deviceName: string | null;
  outputPath: string | null;
  gain: number;
  asrRequest: AsrTranscriptionRequest;
}

export async function startNativeLiveTranscription(
  request: StartNativeLiveTranscriptionRequest,
) {
  return invokeTauri(TauriCommand.recognizer.startNativeLive, {
    ...request,
    asrRequest: normalizeAsrRequest(request.asrRequest),
  });
}

export async function pauseNativeLiveTranscription(
  consumerId: string,
  sourceKind: 'system' | 'microphone',
): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.pauseNativeLive, { consumerId, sourceKind });
}

export async function resumeNativeLiveTranscription(request: {
  consumerId: string;
  sourceKind: 'system' | 'microphone';
  gain: number;
  asrRequest: AsrTranscriptionRequest;
}) {
  return invokeTauri(TauriCommand.recognizer.resumeNativeLive, {
    ...request,
    asrRequest: normalizeAsrRequest(request.asrRequest),
  });
}

export async function stopNativeLiveTranscription(
  consumerId: string,
  sourceKind: 'system' | 'microphone',
): Promise<string> {
  return invokeTauri(TauriCommand.recognizer.stopNativeLive, { consumerId, sourceKind });
}

export async function stopLiveTranscription(consumerId: string): Promise<void> {
  await invokeTauri(TauriCommand.recognizer.stopLive, { consumerId });
}

export async function getLiveTranscriptionMetrics() {
  return invokeTauri(TauriCommand.recognizer.getLiveMetrics);
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
