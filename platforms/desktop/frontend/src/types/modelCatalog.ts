import presetModelsData from '../../../../../core/src/models/preset-models.json';
import type { ModelFileConfig } from './model';

export type TimestampSupportHint = 'token' | 'segment' | 'unknown';

/** How a model handles ASR language selection; drives client language pickers. */
export type LanguageMode = 'selectable' | 'auto' | 'fixed' | 'none';
export type ModelMode = 'live' | 'streaming' | 'batch';

export interface ModelRules {
  requiresVad: boolean;
  requiresPunctuation: boolean;
  timestampSupportHint?: TimestampSupportHint;
  initialRefreshRateMs?: number;
}

export interface ModelArtifact {
  url: string;
  filename: string;
  sha256?: string;
  sizeBytes?: number;
}

/**
 * Curated 1-5 quality hints rendered as dot meters on model cards. Values are
 * relative within the preset catalog, not benchmark scores, and are optional
 * so companion models can omit dimensions that make no sense for them.
 */
export interface ModelRatings {
  accuracy?: number;
  speed?: number;
}

/**
 * Curated family-level marks shown as tags and used by the label filter:
 * `accurate` for high-accuracy families, `lite` for lightweight ones.
 */
export type ModelLabel = 'accurate' | 'lite';

export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  type:
    | 'zipformer'
    | 'sensevoice'
    | 'paraformer'
    | 'punctuation'
    | 'vad'
    | 'itn'
    | 'whisper'
    | 'funasr-nano'
    | 'fire-red-asr'
    | 'dolphin'
    | 'qwen3-asr'
    | 'parakeet-tdt'
    | 'moonshine'
    | 'speaker-segmentation'
    | 'speaker-embedding'
    | 'omnilingual';
  modes?: ModelMode[];
  /** All recognizable languages, sorted ascending ISO 639 codes (`yue` = Cantonese). */
  languages: string[];
  languageMode: LanguageMode;
  size: string;
  artifacts?: ModelArtifact[];
  isRecommended?: boolean;
  isArchive?: boolean;
  filename?: string;
  engine: 'sherpa-onnx' | 'llama-cpp';
  ratings?: ModelRatings;
  labels?: ModelLabel[];
  rules?: ModelRules;
  fileConfig?: ModelFileConfig;
  groupId?: string;
  versionLabel?: string;
}

export type ModelCatalogSectionType =
  | 'asr'
  | 'punctuation'
  | 'vad'
  | 'speaker-segmentation'
  | 'speaker-embedding';

export interface ModelCatalogModel extends ModelInfo {
  installPath: string;
  downloadPath: string;
  isInstalled: boolean;
  rules: ModelRules;
}

export interface ModelCatalogGroup {
  key: string;
  models: ModelCatalogModel[];
}

export interface ModelCatalogSection {
  type: ModelCatalogSectionType;
  groups: ModelCatalogGroup[];
}

export interface ModelSelectionOption {
  id: string;
  label: string;
  installPath: string;
  isInstalled: boolean;
}

export interface ModelCatalogSelectionOptions {
  streaming: ModelSelectionOption[];
  batch: ModelSelectionOption[];
  speakerSegmentation: ModelSelectionOption[];
  speakerEmbedding: ModelSelectionOption[];
}

export type ModelDependencyConfigKey = 'vadModelPath' | 'punctuationModelPath';

export interface ModelDependencyRequest {
  modelId: string;
  configKey: ModelDependencyConfigKey;
  installPath: string;
  isInstalled: boolean;
}

export interface ModelCatalogPathMatchToken {
  id: string;
  token: string;
}

export interface ModelCatalogRestoreDefaults {
  streamingModelPath?: string;
  batchModelPath?: string;
  vadModelPath?: string;
  punctuationModelPath?: string;
  speakerSegmentationModelPath?: string;
  speakerEmbeddingModelPath?: string;
  enableITN: boolean;
  batchVadEnabled?: boolean;
  vadBufferSize: number;
  maxConcurrent: number;
}

export interface ModelCatalogSnapshot {
  modelsDir: string;
  models: ModelCatalogModel[];
  sections: ModelCatalogSection[];
  selectionOptions: ModelCatalogSelectionOptions;
  modelPathById: Record<string, string>;
  modelIdByNormalizedPath: Record<string, string>;
  pathMatchTokens: ModelCatalogPathMatchToken[];
  dependencyRequestsByModelId: Record<string, ModelDependencyRequest[]>;
  restoreDefaults: ModelCatalogRestoreDefaults;
}

export interface ModelSelectionPaths {
  streamingModelPath: string;
  batchModelPath: string;
  speakerSegmentationModelPath: string;
  speakerEmbeddingModelPath: string;
}

export interface ModelCatalogSelectedIds {
  streaming: string | null;
  batch: string | null;
  speakerSegmentation: string | null;
  speakerEmbedding: string | null;
}

/** Selected preset-model ids for per-scenario (live/batch) companion models. */
export type ScenarioSelectedModelIds = Record<
  | 'liveSpeakerSegmentation'
  | 'batchSpeakerSegmentation'
  | 'liveSpeakerEmbedding'
  | 'batchSpeakerEmbedding'
  | 'livePunctuation'
  | 'batchPunctuation'
  | 'liveVad'
  | 'batchVad',
  string | null
>;

export const EMPTY_SCENARIO_SELECTED_MODEL_IDS: ScenarioSelectedModelIds = {
  liveSpeakerSegmentation: null,
  batchSpeakerSegmentation: null,
  liveSpeakerEmbedding: null,
  batchSpeakerEmbedding: null,
  livePunctuation: null,
  batchPunctuation: null,
  liveVad: null,
  batchVad: null,
};

export const DEFAULT_MODEL_RULES: ModelRules = {
  requiresVad: true,
  requiresPunctuation: false,
};

export const PRESET_MODELS: ModelInfo[] = presetModelsData as ModelInfo[];

export const PRESET_MODELS_MAP: Map<string, ModelInfo> = new Map(
  PRESET_MODELS.map((model) => [model.id, model])
);

/**
 * Snapshot models arrive from the Rust core without curated metadata, so
 * callers fall back to the shared preset JSON bundled with the frontend. Once
 * the core forwards these fields, the snapshot value wins.
 */
export function resolveModelRatings(model: ModelInfo): ModelRatings | undefined {
  return model.ratings ?? PRESET_MODELS_MAP.get(model.id)?.ratings;
}

/** Same fallback strategy as resolveModelRatings, for family labels. */
export function resolveModelLabels(model: ModelInfo): ModelLabel[] | undefined {
  return model.labels ?? PRESET_MODELS_MAP.get(model.id)?.labels;
}

export type ProgressCallback = (percentage: number, status: string, isFinished?: boolean) => void;
