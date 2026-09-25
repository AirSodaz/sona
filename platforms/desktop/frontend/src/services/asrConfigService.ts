import type {
  AsrTranscriptionRequest,
  AsrTranscriptionRequestBase,
  OnlineAsrProviderRequest,
  TranscriptPostprocessOptions,
} from '../types/asr';
import type {
  AppConfig,
  AsrConfig,
  AsrMode,
  AsrModelSelection,
  AsrProviderConfig,
  AsrScenario,
  AsrSelectionSlot,
  ModelConfig,
  OnlineAsrProviderConfig,
  OnlineAsrProviderId,
  TextReplacementRuleSet,
} from '../types/config';
import type { ModelInfo } from '../types/modelCatalog';
import type { EffectivePipelineSnapshot, ProjectPipelineConfig } from '../types/project';
import { coerceLanguage, type LanguageCapable } from '../utils/languages';
import { findSelectedModelByMode } from '../utils/modelSelection';
import {
  getScenarioAlignmentModelPath,
  getScenarioPunctuationModelPath,
  getScenarioVadBufferSize,
  getScenarioVadModelPath,
} from '../utils/scenarioModels';
import {
  getTermsForProject,
  parseYamlDictionary,
  VOICE_TYPING_SCOPE_ID,
} from '../utils/yamlDictionaryParser';
import { modelService, PRESET_MODELS_MAP } from './modelService';
import {
  createOnlineAsrSelection,
  DEFAULT_GROQ_WHISPER_ASR_CONFIG,
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  GROQ_WHISPER_PROVIDER_ID,
  getOnlineAsrProviderDefinition,
  getOnlineProviderConfig,
  isOnlineAsrProviderId,
  isVolcengineFlashBatchMode,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
  VOLCENGINE_DOUBAO_PROFILE_ID,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
} from './onlineAsrProviders';

export type {
  AsrTranscriptionRequest,
  AsrTranscriptionRequestBase,
  LocalAsrRequest,
  OnlineAsrRequest,
  TranscriptPostprocessOptions,
} from '../types/asr';
export {
  DEFAULT_GROQ_WHISPER_ASR_CONFIG,
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  GROQ_WHISPER_PROVIDER_ID,
  isVolcengineFlashBatchMode,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
  VOLCENGINE_DOUBAO_PROFILE_ID,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
};

export function isLlamaCppBatchRequest(request: AsrTranscriptionRequest): boolean {
  return (
    request.engine === 'local' && request.mode === 'batch' && request.localEngine === 'llama-cpp'
  );
}

const SLOT_MODE: Record<AsrSelectionSlot, AsrMode> = {
  live: 'streaming',
  caption: 'streaming',
  voiceTyping: 'streaming',
  batch: 'batch',
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

  createDefaultAsrConfig = (streamingModelPath = '', batchModelPath = ''): AsrConfig => {
    return {
      selections: {
        live: this.createLocalSelection('streaming', streamingModelPath),
        caption: this.createLocalSelection('streaming', streamingModelPath),
        voiceTyping: this.createLocalSelection('streaming', streamingModelPath),
        batch: this.createLocalSelection('batch', batchModelPath),
      },
      providers: this.createDefaultAsrProviders(),
    };
  };

  createVolcengineDoubaoSelection = (mode: AsrMode): AsrModelSelection => {
    return createOnlineAsrSelection(VOLCENGINE_DOUBAO_PROVIDER_ID, mode);
  };

  buildPostprocessOptions = (
    config: AppConfig,
    slot?: AsrSelectionSlot
  ): TranscriptPostprocessOptions => {
    if (config.dictionaryContent?.trim()) {
      const parsed = parseYamlDictionary(config.dictionaryContent);
      const projectId = slot === 'voiceTyping' ? VOICE_TYPING_SCOPE_ID : undefined;
      const terms = getTermsForProject(parsed, projectId);
      const textReplacementSets: TextReplacementRuleSet[] = [];

      if (terms.replacements.length > 0) {
        textReplacementSets.push({
          id: 'unified-dictionary-replacements',
          name: 'Unified Dictionary',
          enabled: true,
          ignoreCase: false,
          rules: terms.replacements.map((r, i) => ({
            id: `rep_${i}`,
            from: r.from,
            to: r.to,
          })),
        });
      }

      return {
        textReplacementSets,
        dropFinalDotSegments: true,
      };
    }

    return {
      textReplacementSets: config.textReplacementSets || [],
      dropFinalDotSegments: true,
    };
  };

  /**
   * Resolves the language capability of whatever model currently backs `slot`
   * (local model or online provider), driving language pickers and coercion.
   */
  resolveActiveLanguageCapability = (
    config: AppConfig,
    slot: AsrSelectionSlot
  ): LanguageCapable | null => {
    const selection = this.getSelection({ ...config, asr: this.normalizeAsrConfig(config) }, slot);
    if (selection.engine === 'online') {
      const definition = getOnlineAsrProviderDefinition(selection.providerId);
      if (!definition) {
        return null;
      }
      return {
        languages: definition.manifestEntry.languages as string[],
        languageMode: definition.manifestEntry.languageMode as LanguageCapable['languageMode'],
      };
    }
    const modelInfo = this.resolveModelInfo(selection);
    if (!modelInfo) {
      return null;
    }
    return { languages: modelInfo.languages, languageMode: modelInfo.languageMode };
  };

  /** Coerces a persisted language selection onto the active model's real capabilities. */
  coerceConfiguredLanguage = (
    config: AppConfig,
    slot: AsrSelectionSlot,
    configured: string | null | undefined
  ): string => {
    return coerceLanguage(this.resolveActiveLanguageCapability(config, slot), configured);
  };

  resolveAsrTranscriptionRequest = (
    config: AppConfig,
    slot: AsrSelectionSlot,
    overrides: Partial<
      Pick<AsrTranscriptionRequest, 'language' | 'hotwords' | 'postprocessOptions'>
    > = {}
  ): AsrTranscriptionRequest => {
    const normalizedAsr = this.normalizeAsrConfig(config);
    const selection = this.getSelection({ ...config, asr: normalizedAsr }, slot);
    const baseRequest: AsrTranscriptionRequestBase = {
      mode: selection.mode,
      language: overrides.language || this.coerceConfiguredLanguage(config, slot, config.language),
      enableItn: config.enableITN ?? false,
      normalizationOptions: {
        enableTimeline: config.enableTimeline ?? false,
      },
      postprocessOptions:
        overrides.postprocessOptions || this.buildPostprocessOptions(config, slot),
      hotwords:
        overrides.hotwords !== undefined ? overrides.hotwords : this.buildHotwords(config, slot),
    };

    if (selection.engine === 'online') {
      return {
        ...baseRequest,
        engine: 'online',
        onlineProvider: this.buildOnlineProviderRequest(
          normalizedAsr.providers!,
          selection
        ) as OnlineAsrProviderRequest,
      };
    }

    const modelInfo = this.resolveModelInfo(selection);
    const rules = modelInfo
      ? this.ports.modelService.getModelRules(modelInfo.id)
      : { requiresPunctuation: false, requiresVad: false };

    const scenario: AsrScenario = slot === 'batch' ? 'batch' : 'live';
    const modelType = (modelInfo?.type || '').toLowerCase();
    const idLower = (selection.modelId || '').toLowerCase();
    const pathLower = (selection.modelPath || '').toLowerCase();
    const isExempt =
      modelType === 'qwen3-asr' ||
      modelType === 'parakeet-tdt' ||
      idLower.includes('qwen3-asr') ||
      idLower.includes('qwen3_asr') ||
      idLower.includes('parakeet-tdt') ||
      idLower.includes('parakeet_tdt') ||
      pathLower.includes('qwen3-asr') ||
      pathLower.includes('qwen3_asr') ||
      pathLower.includes('parakeet-tdt') ||
      pathLower.includes('parakeet_tdt');
    const hasBatchModel = Boolean(selection.modelId || selection.modelPath);
    const isBatchVadForced =
      scenario === 'batch' && selection.engine === 'local' && hasBatchModel && !isExempt;
    const batchVadEnabled =
      isBatchVadForced || scenario !== 'batch' || config.batchVadEnabled !== false;
    const vadModelPath = getScenarioVadModelPath(config, scenario);
    const punctuationModelPath = getScenarioPunctuationModelPath(config, scenario);
    const vadModel =
      batchVadEnabled && (rules.requiresVad || isBatchVadForced) && vadModelPath
        ? vadModelPath
        : null;
    const punctuationModel =
      rules.requiresPunctuation && punctuationModelPath ? punctuationModelPath : null;
    const alignmentModelPath = getScenarioAlignmentModelPath(config, scenario);
    const alignmentModel = alignmentModelPath ? alignmentModelPath : null;
    return {
      ...baseRequest,
      engine: 'local',
      localEngine: modelInfo?.engine ?? 'sherpa-onnx',
      modelId: selection.modelId ?? modelInfo?.id ?? null,
      modelPath: selection.modelPath,
      numThreads: 4,
      punctuationModel,
      alignmentModel,
      vadModel,
      vadBuffer: getScenarioVadBufferSize(config, scenario),
      ...(scenario === 'batch' ? { batchSegmentationMode: batchVadEnabled ? 'vad' : 'whole' } : {}),
      modelType: modelInfo?.type || 'sensevoice',
      fileConfig: modelInfo?.fileConfig,
      gpuAcceleration: config.gpuAcceleration ?? 'auto',
      ffmpegPath: config.ffmpegPath || undefined,
    };
  };

  isAsrRequestConfigured = (request: AsrTranscriptionRequest): boolean => {
    if (request.engine === 'local') {
      return Boolean(request.modelPath.trim());
    }

    if (request.engine === 'online') {
      const onlineProvider = request.onlineProvider;
      const definition = getOnlineAsrProviderDefinition(onlineProvider?.providerId);
      return Boolean(
        definition &&
          onlineProvider &&
          definition.isConfigured(onlineProvider.config as never, request.mode)
      );
    }

    return false;
  };

  syncOnlineAsrSelectionFields = (
    config: ModelConfig,
    slot: AsrSelectionSlot,
    providerId: OnlineAsrProviderId
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
    asr.selections[slot] = createOnlineAsrSelection(providerId, SLOT_MODE[slot]);
    return { asr };
  };

  syncStreamingOnlineAsrSelectionFields = (
    config: ModelConfig,
    providerId: OnlineAsrProviderId
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
    asr.selections.live = createOnlineAsrSelection(providerId, 'streaming');
    asr.selections.caption = createOnlineAsrSelection(providerId, 'streaming');
    asr.selections.voiceTyping = createOnlineAsrSelection(providerId, 'streaming');
    return { asr };
  };

  syncOnlineAsrProviderConfig = <TProvider extends OnlineAsrProviderId>(
    config: ModelConfig,
    providerId: TProvider,
    updates: Record<string, unknown>
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
  };

  syncVolcengineDoubaoSelectionFields = (
    config: ModelConfig,
    slot: AsrSelectionSlot
  ): Partial<AppConfig> => {
    return this.syncOnlineAsrSelectionFields(config, slot, VOLCENGINE_DOUBAO_PROVIDER_ID);
  };

  syncStreamingVolcengineDoubaoSelectionFields = (config: ModelConfig): Partial<AppConfig> => {
    return this.syncStreamingOnlineAsrSelectionFields(config, VOLCENGINE_DOUBAO_PROVIDER_ID);
  };

  syncVolcengineDoubaoProviderConfig = (
    config: ModelConfig,
    updates: Partial<OnlineAsrProviderConfig>
  ): Partial<AppConfig> => {
    return this.syncOnlineAsrProviderConfig(config, VOLCENGINE_DOUBAO_PROVIDER_ID, updates);
  };

  syncLegacyAsrSelectionFields = (
    config: ModelConfig,
    slot: AsrSelectionSlot,
    updates: Pick<AsrModelSelection, 'modelId' | 'modelPath'>
  ): Partial<AppConfig> => {
    const asr = this.normalizeAsrConfig(config);
    const mode = SLOT_MODE[slot];
    asr.selections[slot] = {
      engine: 'local',
      mode,
      modelId: updates.modelId ?? null,
      modelPath: updates.modelPath,
    };

    const patch: Partial<AppConfig> = { asr };
    if (mode === 'streaming') {
      patch.streamingModelPath = updates.modelPath;
    } else {
      patch.batchModelPath = updates.modelPath;
    }
    return patch;
  };

  syncStreamingAsrSelectionFields = (
    config: ModelConfig,
    updates: Pick<AsrModelSelection, 'modelId' | 'modelPath'>
  ): Partial<AppConfig> => {
    const livePatch = this.syncLegacyAsrSelectionFields(config, 'live', updates);
    const captionPatch = this.syncLegacyAsrSelectionFields(
      { ...config, ...livePatch },
      'caption',
      updates
    );
    const voiceTypingPatch = this.syncLegacyAsrSelectionFields(
      { ...config, ...livePatch, ...captionPatch },
      'voiceTyping',
      updates
    );

    return {
      ...livePatch,
      asr: voiceTypingPatch.asr,
    };
  };
  syncLiveAsrSelectionFields = this.syncStreamingAsrSelectionFields;
  syncLiveOnlineAsrSelectionFields = this.syncStreamingOnlineAsrSelectionFields;

  // --- Private helpers ---

  private createLocalSelection = (mode: AsrMode, modelPath: string): AsrModelSelection => {
    return {
      engine: 'local',
      mode,
      modelId: null,
      modelPath,
    };
  };

  private createDefaultAsrProviders = (): AsrProviderConfig => {
    return {
      online: {
        [VOLCENGINE_DOUBAO_PROVIDER_ID]: { ...DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG },
        [GROQ_WHISPER_PROVIDER_ID]: { ...DEFAULT_GROQ_WHISPER_ASR_CONFIG },
      },
    };
  };

  private normalizeAsrProviders = (
    providers: Partial<AsrProviderConfig> | undefined
  ): AsrProviderConfig => {
    return {
      online: {
        [VOLCENGINE_DOUBAO_PROVIDER_ID]: getOnlineProviderConfig(
          providers,
          VOLCENGINE_DOUBAO_PROVIDER_ID
        ),
        [GROQ_WHISPER_PROVIDER_ID]: getOnlineProviderConfig(providers, GROQ_WHISPER_PROVIDER_ID),
      },
    };
  };

  private getLegacyModelPath = (config: AppConfig, mode: AsrMode): string => {
    return mode === 'streaming' ? config.streamingModelPath || '' : config.batchModelPath || '';
  };

  private normalizeAsrConfig = (config: ModelConfig): AsrConfig => {
    const currentSelections = config.asr?.selections;
    return {
      selections: {
        live: this.normalizeSelection(
          currentSelections?.live,
          'streaming',
          config.streamingModelPath || ''
        ),
        caption: this.normalizeSelection(
          currentSelections?.caption,
          'streaming',
          config.streamingModelPath || ''
        ),
        voiceTyping: this.normalizeSelection(
          currentSelections?.voiceTyping,
          'streaming',
          config.streamingModelPath || ''
        ),
        batch: this.normalizeSelection(
          currentSelections?.batch,
          'batch',
          config.batchModelPath || ''
        ),
      },
      providers: this.normalizeAsrProviders(config.asr?.providers),
    };
  };

  private normalizeSelection = (
    selection: AsrModelSelection | undefined,
    mode: AsrMode,
    fallbackPath: string
  ): AsrModelSelection => {
    const rawSelection = selection as
      | ({ engine?: string } & Partial<AsrModelSelection>)
      | undefined;
    if (
      rawSelection &&
      (rawSelection.engine === 'online' || isLegacyOnlineEngine(rawSelection.engine))
    ) {
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
      engine: 'local',
      mode,
      modelId: selection?.modelId ?? null,
      modelPath: selection?.modelPath?.trim() ? selection.modelPath : fallbackPath,
    };
  };

  private getSelection = (config: AppConfig, slot: AsrSelectionSlot): AsrModelSelection => {
    const mode = SLOT_MODE[slot];
    const selection = this.normalizeAsrConfig(config).selections[slot];
    if (selection.engine === 'online') {
      return selection;
    }
    if (selection.modelPath.trim()) {
      return selection;
    }
    return this.createLocalSelection(mode, this.getLegacyModelPath(config, mode));
  };

  private resolveModelInfo = (selection: AsrModelSelection): ModelInfo | null => {
    if (selection.engine !== 'local') {
      return null;
    }
    if (selection.modelId) {
      return this.ports.PRESET_MODELS_MAP.get(selection.modelId) ?? null;
    }
    return findSelectedModelByMode(selection.modelPath, selection.mode);
  };

  private buildHotwords = (config: AppConfig, slot?: AsrSelectionSlot): string | null => {
    if (config.dictionaryContent?.trim()) {
      const parsed = parseYamlDictionary(config.dictionaryContent);
      const projectId = slot === 'voiceTyping' ? VOICE_TYPING_SCOPE_ID : undefined;
      const terms = getTermsForProject(parsed, projectId);
      return terms.hotwords.length > 0 ? terms.hotwords.join(',') : null;
    }

    const words =
      config.hotwordSets
        ?.filter((set) => set.enabled)
        .flatMap((set) => set.rules.map((rule) => rule.text.trim()))
        .filter(Boolean) ?? [];
    return words.length > 0 ? words.join(',') : null;
  };

  buildHotwordsWithPipeline = (
    config: AppConfig,
    pipeline?: ProjectPipelineConfig | EffectivePipelineSnapshot | null,
    projectId?: string | null,
    projectName?: string
  ): string | null => {
    if (config.dictionaryContent?.trim()) {
      const parsed = parseYamlDictionary(config.dictionaryContent);
      const terms = getTermsForProject(parsed, projectId, projectName);
      return terms.hotwords.length > 0 ? terms.hotwords.join(',') : null;
    }

    const baseWords =
      config.hotwordSets
        ?.filter((set) => set.enabled)
        .flatMap((set) => set.rules.map((rule) => rule.text.trim()))
        .filter(Boolean) ?? [];

    const customTerms = pipeline?.customTerms ?? [];
    const pipelineWords: string[] = [];
    for (const term of customTerms) {
      const trimmed = term.trim();
      if (!trimmed) continue;
      if (trimmed.includes('=>')) {
        const parts = trimmed.split('=>');
        const target = parts[1]?.trim();
        if (target) pipelineWords.push(target);
      } else if (trimmed.includes('->')) {
        const parts = trimmed.split('->');
        const target = parts[1]?.trim();
        if (target) pipelineWords.push(target);
      } else {
        pipelineWords.push(trimmed);
      }
    }

    const allWords = Array.from(new Set([...baseWords, ...pipelineWords]));
    return allWords.length > 0 ? allWords.join(',') : null;
  };

  private buildOnlineProviderRequest = (
    providers: AsrProviderConfig,
    selection: AsrModelSelection
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
  };
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
  syncLiveAsrSelectionFields,
  syncLiveOnlineAsrSelectionFields,
} = asrConfigService;
