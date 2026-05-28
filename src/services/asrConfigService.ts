import type {
  AppConfig,
  AsrConfig,
  AsrEngine,
  AsrMode,
  AsrModelSelection,
  AsrProviderConfig,
  AsrSelectionSlot,
  ModelConfig,
  OnlineAsrProviderId,
  TextReplacementRuleSet,
  OnlineAsrProviderConfig,
} from '../types/config';
import type { ModelFileConfig } from '../types/model';
import { findSelectedModelByMode } from '../utils/modelSelection';
import { modelService, PRESET_MODELS_MAP, type ModelInfo } from './modelService';
import {
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
  VOLCENGINE_DOUBAO_PROFILE_ID,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
  DEFAULT_GROQ_WHISPER_ASR_CONFIG,
  GROQ_WHISPER_PROVIDER_ID,
  createOnlineAsrSelection,
  getOnlineAsrProviderDefinition,
  getOnlineProviderConfig,
  isOnlineAsrProviderId,
  isVolcengineFlashBatchMode,
  type OnlineAsrProviderRequest,
} from './onlineAsrProviders';

export {
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
  VOLCENGINE_DOUBAO_PROFILE_ID,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
  DEFAULT_GROQ_WHISPER_ASR_CONFIG,
  GROQ_WHISPER_PROVIDER_ID,
  isVolcengineFlashBatchMode,
};

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
  batchSegmentationMode?: 'vad' | 'whole';
  modelType: string;
  fileConfig?: ModelFileConfig;
  hotwords: string | null;
  normalizationOptions: {
    enableTimeline: boolean;
  };
  postprocessOptions: TranscriptPostprocessOptions;
  onlineProvider?: OnlineAsrProviderRequest;
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

function isLegacyOnlineEngine(engine: unknown): boolean {
  return engine === 'volcengine-doubao';
}

export interface AsrConfigServicePorts {
  modelService: typeof modelService;
  PRESET_MODELS_MAP: typeof PRESET_MODELS_MAP;
}

export class AsrConfigService {
  constructor(private readonly ports: AsrConfigServicePorts) {}

  createDefaultAsrConfig = (
    streamingModelPath = '',
    offlineModelPath = '',
  ): AsrConfig => {
    return {
      selections: {
        live: this.createLocalSherpaSelection('streaming', streamingModelPath),
        caption: this.createLocalSherpaSelection('streaming', streamingModelPath),
        voiceTyping: this.createLocalSherpaSelection('streaming', streamingModelPath),
        batch: this.createLocalSherpaSelection('offline', offlineModelPath),
      },
      providers: this.createDefaultAsrProviders(),
    };
  }

  createVolcengineDoubaoSelection = (mode: AsrMode): AsrModelSelection => {
    return createOnlineAsrSelection(VOLCENGINE_DOUBAO_PROVIDER_ID, mode);
  }

  buildPostprocessOptions = (config: AppConfig): TranscriptPostprocessOptions => {
    return {
      textReplacementSets: config.textReplacementSets || [],
      dropFinalDotSegments: true,
    };
  }

  resolveAsrTranscriptionRequest = (
    config: AppConfig,
    slot: AsrSelectionSlot,
    overrides: Partial<Pick<AsrTranscriptionRequest, 'language'>> = {},
  ): AsrTranscriptionRequest => {
    const normalizedAsr = this.normalizeAsrConfig(config);
    const selection = this.getSelection({ ...config, asr: normalizedAsr }, slot);
    const modelInfo = this.resolveModelInfo(selection);
    const rules = modelInfo
      ? this.ports.modelService.getModelRules(modelInfo.id)
      : { requiresPunctuation: false, requiresVad: false };

    const batchVadEnabled = slot !== 'batch' || config.batchVadEnabled !== false;
    const vadModel = batchVadEnabled && rules.requiresVad && config.vadModelPath
      ? config.vadModelPath
      : null;
    const punctuationModel = rules.requiresPunctuation && config.punctuationModelPath
      ? config.punctuationModelPath
      : null;

    const request: AsrTranscriptionRequest = {
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
      ...(slot === 'batch' && selection.engine === 'local-sherpa'
        ? { batchSegmentationMode: batchVadEnabled ? 'vad' : 'whole' }
        : {}),
      modelType: modelInfo?.type || 'sensevoice',
      fileConfig: modelInfo?.fileConfig,
      hotwords: this.buildHotwords(config),
      normalizationOptions: {
        enableTimeline: config.enableTimeline ?? false,
      },
      postprocessOptions: this.buildPostprocessOptions(config),
      ...(selection.engine === 'online'
        ? { onlineProvider: this.buildOnlineProviderRequest(normalizedAsr.providers!, selection) }
        : {}),
    };

    return request;
  }

  isAsrRequestConfigured = (request: AsrTranscriptionRequest): boolean => {
    if (request.engine === 'local-sherpa') {
      return Boolean(request.modelPath.trim());
    }

    if (request.engine === 'online') {
      const onlineProvider = request.onlineProvider;
      const definition = getOnlineAsrProviderDefinition(onlineProvider?.providerId);
      return Boolean(
        definition
        && onlineProvider
        && definition.isConfigured(onlineProvider.config as never, request.mode),
      );
    }

    return false;
  }

  syncOnlineAsrSelectionFields = (
    config: ModelConfig,
    slot: AsrSelectionSlot,
    providerId: OnlineAsrProviderId,
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
    asr.selections[slot] = createOnlineAsrSelection(providerId, SLOT_MODE[slot]);
    return { asr };
  }

  syncStreamingOnlineAsrSelectionFields = (
    config: ModelConfig,
    providerId: OnlineAsrProviderId,
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
    asr.selections.live = createOnlineAsrSelection(providerId, 'streaming');
    asr.selections.caption = createOnlineAsrSelection(providerId, 'streaming');
    asr.selections.voiceTyping = createOnlineAsrSelection(providerId, 'streaming');
    return { asr };
  }

  syncOnlineAsrProviderConfig = <TProvider extends OnlineAsrProviderId>(
    config: ModelConfig,
    providerId: TProvider,
    updates: Record<string, unknown>,
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
    const existing = getOnlineProviderConfig(asr.providers, providerId);
    const definition = getOnlineAsrProviderDefinition(providerId);
    
    asr.providers = {
      ...asr.providers,
      online: {
        ...(asr.providers?.online ?? {}),
        [providerId]: definition
          ? definition.normalizeConfig({
              ...(existing as Record<string, unknown>),
              ...updates,
            })
          : {
              ...(existing as Record<string, unknown>),
              ...updates,
            },
      },
    };
    return { asr };
  }

  syncVolcengineDoubaoSelectionFields = (
    config: ModelConfig,
    slot: AsrSelectionSlot,
  ): Partial<AppConfig> => {
    return this.syncOnlineAsrSelectionFields(config, slot, VOLCENGINE_DOUBAO_PROVIDER_ID);
  }

  syncStreamingVolcengineDoubaoSelectionFields = (
    config: ModelConfig,
  ): Partial<AppConfig> => {
    return this.syncStreamingOnlineAsrSelectionFields(config, VOLCENGINE_DOUBAO_PROVIDER_ID);
  }

  syncVolcengineDoubaoProviderConfig = (
    config: ModelConfig,
    updates: Partial<OnlineAsrProviderConfig>,
  ): Partial<AppConfig> => {
    return this.syncOnlineAsrProviderConfig(config, VOLCENGINE_DOUBAO_PROVIDER_ID, updates);
  }

  syncLegacyAsrSelectionFields = (
    config: ModelConfig,
    slot: AsrSelectionSlot,
    updates: Pick<AsrModelSelection, 'modelId' | 'modelPath'>,
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
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

  syncStreamingAsrSelectionFields = (
    config: ModelConfig,
    updates: Pick<AsrModelSelection, 'modelId' | 'modelPath'>,
  ): Partial<AppConfig> => {
    const livePatch = this.syncLegacyAsrSelectionFields(config, 'live', updates);
    const captionPatch = this.syncLegacyAsrSelectionFields(
      { ...config, ...livePatch },
      'caption',
      updates,
    );
    const voiceTypingPatch = this.syncLegacyAsrSelectionFields(
      { ...config, ...livePatch, ...captionPatch },
      'voiceTyping',
      updates,
    );

    return {
      ...livePatch,
      asr: voiceTypingPatch.asr,
    };
  }

  // --- Private helpers ---

  private createLocalSherpaSelection = (mode: AsrMode, modelPath: string): AsrModelSelection => {
    return {
      engine: 'local-sherpa',
      mode,
      modelId: null,
      modelPath,
    };
  }

  private createDefaultAsrProviders = (): AsrProviderConfig => {
    return {
      online: {
        [VOLCENGINE_DOUBAO_PROVIDER_ID]: { ...DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG },
        [GROQ_WHISPER_PROVIDER_ID]: { ...DEFAULT_GROQ_WHISPER_ASR_CONFIG },
      },
    };
  }

  private normalizeAsrProviders = (
    providers: Partial<AsrProviderConfig> | undefined,
  ): AsrProviderConfig => {
    return {
      online: {
        [VOLCENGINE_DOUBAO_PROVIDER_ID]: getOnlineProviderConfig(providers, VOLCENGINE_DOUBAO_PROVIDER_ID),
        [GROQ_WHISPER_PROVIDER_ID]: getOnlineProviderConfig(providers, GROQ_WHISPER_PROVIDER_ID),
      },
    };
  }

  private getLegacyModelPath = (config: AppConfig, mode: AsrMode): string => {
    return mode === 'streaming'
      ? config.streamingModelPath || ''
      : config.offlineModelPath || '';
  }

  private normalizeAsrConfig = (config: ModelConfig): AsrConfig => {
    const currentSelections = config.asr?.selections;
    return {
      selections: {
        live: this.normalizeSelection(currentSelections?.live, 'streaming', config.streamingModelPath || ''),
        caption: this.normalizeSelection(currentSelections?.caption, 'streaming', config.streamingModelPath || ''),
        voiceTyping: this.normalizeSelection(currentSelections?.voiceTyping, 'streaming', config.streamingModelPath || ''),
        batch: this.normalizeSelection(currentSelections?.batch, 'offline', config.offlineModelPath || ''),
      },
      providers: this.normalizeAsrProviders(config.asr?.providers),
    };
  }

  private normalizeSelection = (
    selection: AsrModelSelection | undefined,
    mode: AsrMode,
    fallbackPath: string,
  ): AsrModelSelection => {
    const rawSelection = selection as ({ engine?: string } & Partial<AsrModelSelection>) | undefined;
    if (rawSelection && (rawSelection.engine === 'online' || isLegacyOnlineEngine(rawSelection.engine))) {
      const providerId = rawSelection.providerId || '';
      const definition = getOnlineAsrProviderDefinition(providerId);
      return {
        engine: 'online',
        mode,
        modelId: null,
        modelPath: '',
        providerId,
        profileId: rawSelection.profileId || definition?.profileId || providerId,
      };
    }

    return {
      engine: 'local-sherpa',
      mode,
      modelId: selection?.modelId ?? null,
      modelPath: selection?.modelPath?.trim() ? selection.modelPath : fallbackPath,
    };
  }

  private getSelection = (config: AppConfig, slot: AsrSelectionSlot): AsrModelSelection => {
    const mode = SLOT_MODE[slot];
    const selection = this.normalizeAsrConfig(config).selections[slot];
    if (selection.engine === 'online') {
      return selection;
    }
    if (selection.modelPath.trim()) {
      return selection;
    }
    return this.createLocalSherpaSelection(mode, this.getLegacyModelPath(config, mode));
  }

  private resolveModelInfo = (selection: AsrModelSelection): ModelInfo | null => {
    if (selection.engine !== 'local-sherpa') {
      return null;
    }
    if (selection.modelId) {
      return this.ports.PRESET_MODELS_MAP.get(selection.modelId) ?? null;
    }
    return findSelectedModelByMode(selection.modelPath, selection.mode === 'batch' ? 'offline' : selection.mode);
  }

  private buildHotwords = (config: AppConfig): string | null => {
    const words = config.hotwordSets
      ?.filter((set) => set.enabled)
      .flatMap((set) => set.rules.map((rule) => rule.text.trim()))
      .filter(Boolean) ?? [];
    return words.length > 0 ? words.join(',') : null;
  }

  private buildOnlineProviderRequest = (
    providers: AsrProviderConfig,
    selection: AsrModelSelection,
  ): OnlineAsrProviderRequest | undefined => {
    if (!isOnlineAsrProviderId(selection.providerId)) {
      return undefined;
    }
    const definition = getOnlineAsrProviderDefinition(selection.providerId);
    if (!definition) {
      return undefined;
    }
    return {
      providerId: selection.providerId,
      profileId: selection.profileId || definition.profileId,
      config: getOnlineProviderConfig(providers, selection.providerId),
    };
  }
}

export function createAsrConfigService(ports: AsrConfigServicePorts): AsrConfigService {
  return new AsrConfigService(ports);
}

export const asrConfigService = createAsrConfigService({
  modelService,
  PRESET_MODELS_MAP,
});

// For backwards compatibility and ease of import migration, we also export the unbound methods.
// Wait, actually, let's export them unbound if possible, or just export the instance.
// Since the prompt asks to convert Pattern C to Pattern D, we will replace usages.
export const {
  createDefaultAsrConfig,
  createVolcengineDoubaoSelection,
  buildPostprocessOptions,
  resolveAsrTranscriptionRequest,
  isAsrRequestConfigured,
  syncOnlineAsrSelectionFields,
  syncStreamingOnlineAsrSelectionFields,
  syncOnlineAsrProviderConfig,
  syncVolcengineDoubaoSelectionFields,
  syncStreamingVolcengineDoubaoSelectionFields,
  syncVolcengineDoubaoProviderConfig,
  syncLegacyAsrSelectionFields,
  syncStreamingAsrSelectionFields,
} = asrConfigService;
