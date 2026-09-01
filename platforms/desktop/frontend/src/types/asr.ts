import type {
  AsrMode,
  OnlineAsrProviderConfig,
  OnlineAsrProviderId,
  TextReplacementRuleSet,
} from './config';
import type { ModelFileConfig } from './model';

export type OnlineAsrProviderRequest = {
  providerId: OnlineAsrProviderId;
  profileId: string;
  config: OnlineAsrProviderConfig;
};

export type LocalAsrEngine = 'sherpa-onnx' | 'llama-cpp';

export type TranscriptPostprocessOptions = {
  textReplacementSets: TextReplacementRuleSet[];
  dropFinalDotSegments: boolean;
};

export type AsrTranscriptionRequestBase = {
  mode: AsrMode;
  language: string;
  enableItn: boolean;
  normalizationOptions: {
    enableTimeline: boolean;
  };
  postprocessOptions: TranscriptPostprocessOptions;
  hotwords: string | null;
};

export type LocalAsrRequest = AsrTranscriptionRequestBase & {
  engine: 'local';
  localEngine?: LocalAsrEngine;
  modelId: string | null;
  modelPath: string;
  numThreads: number;
  punctuationModel: string | null;
  vadModel: string | null;
  vadBuffer: number;
  batchSegmentationMode?: 'vad' | 'whole';
  modelType: string;
  fileConfig?: ModelFileConfig;
  gpuAcceleration?: string;
  initialRefreshRateMs?: number | null;
  ffmpegPath?: string | null;
};

export type OnlineAsrRequest = AsrTranscriptionRequestBase & {
  engine: 'online';
  onlineProvider: OnlineAsrProviderRequest;
};

export type AsrTranscriptionRequest = LocalAsrRequest | OnlineAsrRequest;
