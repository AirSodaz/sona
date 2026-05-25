import type {
  AppConfig,
  AsrConfig,
  AsrEngine,
  AsrMode,
  AsrModelSelection,
  AsrProviderConfig,
  AsrSelectionSlot,
  ModelConfig,
  TextReplacementRuleSet,
  VolcengineDoubaoAsrProviderConfig,
} from '../types/config';
import type { ModelFileConfig } from '../types/model';
import { findSelectedModelByMode } from '../utils/modelSelection';
import { modelService, PRESET_MODELS_MAP, type ModelInfo } from './modelService';

export type AsrTranscriptionRequest = {
  engine: AsrEngine;
  mode: AsrMode;
  modelId: string | null;
  modelPath: string;
  providerId?: string | null;
  profileId?: string | null;
  numThreads: number;
  enableItn: boolean;
  language: string;
  punctuationModel: string | null;
  vadModel: string | null;
  vadBuffer: number;
  modelType: string;
  fileConfig?: ModelFileConfig;
  hotwords: string | null;
  normalizationOptions: {
    enableTimeline: boolean;
  };
  postprocessOptions: TranscriptPostprocessOptions;
  volcengine?: VolcengineDoubaoAsrProviderConfig;
};

export type TranscriptPostprocessOptions = {
  textReplacementSets: TextReplacementRuleSet[];
  dropFinalDotSegments: boolean;
};

const SLOT_MODE: Record<AsrSelectionSlot, AsrMode> = {
  live: 'streaming',
  caption: 'streaming',
  voiceTyping: 'streaming',
  batch: 'offline',
};

export const VOLCENGINE_DOUBAO_PROFILE_ID = 'volcengine-doubao-default';
export const DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG: VolcengineDoubaoAsrProviderConfig = {
  apiKey: '',
  streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
  streamingResourceId: 'volc.seedasr.sauc.duration',
  batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
  batchResourceId: 'volc.bigasr.auc_turbo',
};

export function createDefaultAsrConfig(
  streamingModelPath = '',
  offlineModelPath = '',
): AsrConfig {
  return {
    selections: {
      live: createLocalSherpaSelection('streaming', streamingModelPath),
      caption: createLocalSherpaSelection('streaming', streamingModelPath),
      voiceTyping: createLocalSherpaSelection('streaming', streamingModelPath),
      batch: createLocalSherpaSelection('offline', offlineModelPath),
    },
    providers: createDefaultAsrProviders(),
  };
}

function createLocalSherpaSelection(mode: AsrMode, modelPath: string): AsrModelSelection {
  return {
    engine: 'local-sherpa',
    mode,
    modelId: null,
    modelPath,
  };
}

export function createVolcengineDoubaoSelection(mode: AsrMode): AsrModelSelection {
  return {
    engine: 'volcengine-doubao',
    mode,
    modelId: null,
    modelPath: '',
    providerId: 'volcengine-doubao',
    profileId: VOLCENGINE_DOUBAO_PROFILE_ID,
  };
}

function createDefaultAsrProviders(): AsrProviderConfig {
  return {
    volcengineDoubao: { ...DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG },
  };
}

function normalizeVolcengineConfig(
  provider: Partial<VolcengineDoubaoAsrProviderConfig> | undefined,
): VolcengineDoubaoAsrProviderConfig {
  return {
    apiKey: provider?.apiKey?.trim() ?? '',
    streamingEndpoint: provider?.streamingEndpoint?.trim()
      || DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.streamingEndpoint,
    streamingResourceId: provider?.streamingResourceId?.trim()
      || DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.streamingResourceId,
    batchEndpoint: provider?.batchEndpoint?.trim()
      || DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.batchEndpoint,
    batchResourceId: provider?.batchResourceId?.trim()
      || DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.batchResourceId,
  };
}

function normalizeAsrProviders(
  providers: Partial<AsrProviderConfig> | undefined,
): AsrProviderConfig {
  return {
    volcengineDoubao: normalizeVolcengineConfig(providers?.volcengineDoubao),
  };
}

function getLegacyModelPath(config: AppConfig, mode: AsrMode): string {
  return mode === 'streaming'
    ? config.streamingModelPath || ''
    : config.offlineModelPath || '';
}

type AsrModelConfig = Pick<ModelConfig, 'asr' | 'streamingModelPath' | 'offlineModelPath'>;

function normalizeAsrConfig(config: AsrModelConfig): AsrConfig {
  const currentSelections = config.asr?.selections;
  return {
    selections: {
      live: normalizeSelection(currentSelections?.live, 'streaming', config.streamingModelPath || ''),
      caption: normalizeSelection(currentSelections?.caption, 'streaming', config.streamingModelPath || ''),
      voiceTyping: normalizeSelection(currentSelections?.voiceTyping, 'streaming', config.streamingModelPath || ''),
      batch: normalizeSelection(currentSelections?.batch, 'offline', config.offlineModelPath || ''),
    },
    providers: normalizeAsrProviders(config.asr?.providers),
  };
}

function normalizeSelection(
  selection: AsrModelSelection | undefined,
  mode: AsrMode,
  fallbackPath: string,
): AsrModelSelection {
  if (selection?.engine === 'volcengine-doubao') {
    return {
      engine: 'volcengine-doubao',
      mode,
      modelId: null,
      modelPath: '',
      providerId: selection.providerId || 'volcengine-doubao',
      profileId: selection.profileId || VOLCENGINE_DOUBAO_PROFILE_ID,
    };
  }

  return {
    engine: 'local-sherpa',
    mode,
    modelId: selection?.modelId ?? null,
    modelPath: selection?.modelPath?.trim() ? selection.modelPath : fallbackPath,
  };
}

function getSelection(config: AppConfig, slot: AsrSelectionSlot): AsrModelSelection {
  const mode = SLOT_MODE[slot];
  const selection = normalizeAsrConfig(config).selections[slot];
  if (selection.engine === 'volcengine-doubao') {
    return selection;
  }
  if (selection.modelPath.trim()) {
    return selection;
  }
  return createLocalSherpaSelection(mode, getLegacyModelPath(config, mode));
}

function resolveModelInfo(selection: AsrModelSelection): ModelInfo | null {
  if (selection.engine !== 'local-sherpa') {
    return null;
  }
  if (selection.modelId) {
    return PRESET_MODELS_MAP.get(selection.modelId) ?? null;
  }
  return findSelectedModelByMode(selection.modelPath, selection.mode);
}

function buildHotwords(config: AppConfig): string | null {
  const words = config.hotwordSets
    ?.filter((set) => set.enabled)
    .flatMap((set) => set.rules.map((rule) => rule.text.trim()))
    .filter(Boolean) ?? [];
  return words.length > 0 ? words.join(',') : null;
}

export function buildPostprocessOptions(config: AppConfig): TranscriptPostprocessOptions {
  return {
    textReplacementSets: config.textReplacementSets || [],
    dropFinalDotSegments: true,
  };
}

export function resolveAsrTranscriptionRequest(
  config: AppConfig,
  slot: AsrSelectionSlot,
  overrides: Partial<Pick<AsrTranscriptionRequest, 'language'>> = {},
): AsrTranscriptionRequest {
  const selection = getSelection(config, slot);
  const providers = normalizeAsrProviders(config.asr?.providers);
  const modelInfo = resolveModelInfo(selection);
  const rules = modelInfo
    ? modelService.getModelRules(modelInfo.id)
    : { requiresPunctuation: false, requiresVad: false };

  const vadModel = rules.requiresVad && config.vadModelPath
    ? config.vadModelPath
    : null;
  const punctuationModel = rules.requiresPunctuation && config.punctuationModelPath
    ? config.punctuationModelPath
    : null;

  return {
    engine: selection.engine,
    mode: selection.mode,
    modelId: selection.modelId ?? modelInfo?.id ?? null,
    modelPath: selection.modelPath,
    providerId: selection.providerId ?? null,
    profileId: selection.profileId ?? null,
    numThreads: 4,
    enableItn: config.enableITN ?? false,
    language: overrides.language || config.language || 'auto',
    punctuationModel,
    vadModel,
    vadBuffer: config.vadBufferSize || 5,
    modelType: modelInfo?.type || 'sensevoice',
    fileConfig: modelInfo?.fileConfig,
    hotwords: buildHotwords(config),
    normalizationOptions: {
      enableTimeline: config.enableTimeline ?? false,
    },
    postprocessOptions: buildPostprocessOptions(config),
    ...(selection.engine === 'volcengine-doubao'
      ? { volcengine: providers.volcengineDoubao }
      : {}),
  };
}

export function isAsrRequestConfigured(request: AsrTranscriptionRequest): boolean {
  if (request.engine === 'local-sherpa') {
    return Boolean(request.modelPath.trim());
  }

  if (request.engine === 'volcengine-doubao') {
    const volcengine = request.volcengine;
    if (!volcengine?.apiKey.trim()) {
      return false;
    }
    if (request.mode === 'streaming') {
      return Boolean(
        volcengine.streamingEndpoint.trim()
        && volcengine.streamingResourceId.trim(),
      );
    }
    return Boolean(
      volcengine.batchEndpoint.trim()
      && volcengine.batchResourceId.trim(),
    );
  }

  return false;
}

export function syncVolcengineDoubaoSelectionFields(
  config: AsrModelConfig,
  slot: AsrSelectionSlot,
): Partial<AppConfig> {
  const asr = normalizeAsrConfig(config);
  asr.selections[slot] = createVolcengineDoubaoSelection(SLOT_MODE[slot]);
  return { asr };
}

export function syncStreamingVolcengineDoubaoSelectionFields(
  config: AsrModelConfig,
): Partial<AppConfig> {
  const asr = normalizeAsrConfig(config);
  asr.selections.live = createVolcengineDoubaoSelection('streaming');
  asr.selections.caption = createVolcengineDoubaoSelection('streaming');
  asr.selections.voiceTyping = createVolcengineDoubaoSelection('streaming');
  return { asr };
}

export function syncVolcengineDoubaoProviderConfig(
  config: AsrModelConfig,
  updates: Partial<VolcengineDoubaoAsrProviderConfig>,
): Partial<AppConfig> {
  const asr = normalizeAsrConfig(config);
  asr.providers = {
    ...normalizeAsrProviders(asr.providers),
    volcengineDoubao: normalizeVolcengineConfig({
      ...asr.providers?.volcengineDoubao,
      ...updates,
    }),
  };
  return { asr };
}

export function syncLegacyAsrSelectionFields(
  config: AsrModelConfig,
  slot: AsrSelectionSlot,
  updates: Pick<AsrModelSelection, 'modelId' | 'modelPath'>,
): Partial<AppConfig> {
  const asr = normalizeAsrConfig(config);
  const mode = SLOT_MODE[slot];
  asr.selections[slot] = {
    engine: 'local-sherpa',
    mode,
    modelId: updates.modelId ?? null,
    modelPath: updates.modelPath,
  };

  const patch: Partial<AppConfig> = { asr };
  if (mode === 'streaming') {
    patch.streamingModelPath = updates.modelPath;
  } else {
    patch.offlineModelPath = updates.modelPath;
  }
  return patch;
}

export function syncStreamingAsrSelectionFields(
  config: AsrModelConfig,
  updates: Pick<AsrModelSelection, 'modelId' | 'modelPath'>,
): Partial<AppConfig> {
  const livePatch = syncLegacyAsrSelectionFields(config, 'live', updates);
  const captionPatch = syncLegacyAsrSelectionFields(
    { ...config, ...livePatch },
    'caption',
    updates,
  );
  const voiceTypingPatch = syncLegacyAsrSelectionFields(
    { ...config, ...livePatch, ...captionPatch },
    'voiceTyping',
    updates,
  );

  return {
    ...livePatch,
    asr: voiceTypingPatch.asr,
  };
}
