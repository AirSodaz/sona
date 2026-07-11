import onlineAsrProviderManifest from '../../../../../core/src/ports/online-asr-providers.json';
import type {
  AsrMode,
  AsrModelSelection,
  OnlineAsrProviderId,
  OnlineAsrProviderConfig,
  AsrProviderConfig,
} from '../types/config';

export type OnlineAsrProviderRequest = {
  providerId: OnlineAsrProviderId;
  profileId: string;
  config: OnlineAsrProviderConfig;
};

export type OnlineAsrProviderDefinition = {
  id: OnlineAsrProviderId;
  profileId: string;
  optionLabelKey: string;
  optionDefaultLabel: string;
  titleKey: string;
  titleDefault: string;
  onlineUploadHintKey: string;
  onlineUploadHintDefault: string;
  defaultConfig: OnlineAsrProviderConfig;
  manifestEntry: typeof onlineAsrProviderManifest.providers[number];
  normalizeConfig: (config: Partial<OnlineAsrProviderConfig> | undefined) => OnlineAsrProviderConfig;
  isConfigured: (config: OnlineAsrProviderConfig, mode: AsrMode) => boolean;
};

export const VOLCENGINE_DOUBAO_PROVIDER_ID = 'volcengine-doubao';
export const VOLCENGINE_DOUBAO_PROFILE_ID = 'volcengine-doubao';
export const VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT = 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash';
export const VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID = 'volc.bigasr.auc_turbo';

export const GROQ_WHISPER_PROVIDER_ID = 'groq-whisper';

export const DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG = onlineAsrProviderManifest.providers.find(p => p.id === VOLCENGINE_DOUBAO_PROVIDER_ID)?.defaults as Record<string, unknown> || {};
export const DEFAULT_GROQ_WHISPER_ASR_CONFIG = onlineAsrProviderManifest.providers.find(p => p.id === GROQ_WHISPER_PROVIDER_ID)?.defaults as Record<string, unknown> || {};

export function isVolcengineFlashBatchMode(
  provider: Partial<OnlineAsrProviderConfig> | undefined,
): boolean {
  const endpoint = (provider?.batchEndpoint as string)?.trim().replace(/\/+$/, '') ?? '';
  const expectedEndpoint = VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT.replace(/\/+$/, '');
  const resourceId = (provider?.batchResourceId as string)?.trim() ?? '';
  return endpoint === expectedEndpoint && resourceId === VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID;
}

export const ONLINE_ASR_PROVIDER_DEFINITIONS: OnlineAsrProviderDefinition[] = onlineAsrProviderManifest.providers.map(entry => {
  const normalizeConfig = (config: Partial<OnlineAsrProviderConfig> | undefined): OnlineAsrProviderConfig => {
    const defaults = entry.defaults as OnlineAsrProviderConfig;
    if (!config) return { ...defaults };
    
    const normalized: OnlineAsrProviderConfig = { ...defaults };
    for (const key of Object.keys(defaults)) {
      if (config[key] !== undefined) {
        if (typeof config[key] === 'string') {
          if ((config[key] as string).trim() !== '') {
            normalized[key] = (config[key] as string).trim();
          }
        } else {
          normalized[key] = config[key]!;
        }
      }
    }
    
    if (entry.id === VOLCENGINE_DOUBAO_PROVIDER_ID) {
      if (!isVolcengineFlashBatchMode(normalized)) {
        normalized.batchEndpoint = defaults.batchEndpoint;
        normalized.batchResourceId = defaults.batchResourceId;
      }
    }

    return normalized;
  };

  const isConfigured = (config: OnlineAsrProviderConfig, mode: AsrMode): boolean => {
    const capabilities = mode === 'streaming' ? entry.streaming : entry.batch;
    
    if ((capabilities as Record<string, unknown>).supported === false) {
      return false;
    }

    if (capabilities.requiresApiKey && !(config.apiKey as string)?.trim()) {
      return false;
    }
    
    if (capabilities.requiredConfigFields) {
      for (const field of capabilities.requiredConfigFields) {
        if (!(config[field] as string)?.trim()) {
          return false;
        }
      }
    }
    
    if (mode === 'batch' && entry.id === VOLCENGINE_DOUBAO_PROVIDER_ID) {
       if (!entry.batch.localFileMode.supported || !isVolcengineFlashBatchMode(config)) {
         return false;
       }
    }

    return true;
  };

  return {
    id: entry.id,
    profileId: entry.profileId,
    optionLabelKey: entry.ui.optionLabelKey,
    optionDefaultLabel: entry.ui.optionDefaultLabel,
    titleKey: entry.ui.titleKey,
    titleDefault: entry.ui.titleDefault,
    onlineUploadHintKey: entry.ui.onlineUploadHintKey,
    onlineUploadHintDefault: entry.ui.onlineUploadHintDefault,
    defaultConfig: entry.defaults as OnlineAsrProviderConfig,
    manifestEntry: entry,
    normalizeConfig,
    isConfigured,
  };
});

export const ONLINE_ASR_PROVIDER_MAP = new Map<OnlineAsrProviderId, OnlineAsrProviderDefinition>(
  ONLINE_ASR_PROVIDER_DEFINITIONS.map((definition) => [definition.id, definition]),
);

export function isOnlineAsrProviderId(value: string | null | undefined): value is OnlineAsrProviderId {
  return value != null && ONLINE_ASR_PROVIDER_MAP.has(value);
}

export function getOnlineAsrProviderDefinition(
  providerId: string | null | undefined,
): OnlineAsrProviderDefinition | null {
  return isOnlineAsrProviderId(providerId)
    ? ONLINE_ASR_PROVIDER_MAP.get(providerId) ?? null
    : null;
}

export function createOnlineAsrSelection(
  providerId: OnlineAsrProviderId,
  mode: AsrMode,
): AsrModelSelection {
  const definition = getOnlineAsrProviderDefinition(providerId);
  return {
    engine: 'online',
    mode,
    modelId: null,
    modelPath: '',
    providerId,
    profileId: definition?.profileId ?? providerId,
  };
}

export function getOnlineProviderConfig(
  providers: AsrProviderConfig | undefined,
  providerId: OnlineAsrProviderId,
): OnlineAsrProviderConfig {
  const definition = getOnlineAsrProviderDefinition(providerId);
  if (!definition) {
    throw new Error(`Unsupported online ASR provider: ${providerId}`);
  }
  const modernConfig = providers?.online?.[providerId];
  const legacyConfig = providerId === VOLCENGINE_DOUBAO_PROVIDER_ID
    ? providers?.volcengineDoubao
    : providerId === 'groq-whisper'
      ? providers?.groqWhisper
      : undefined;
      
  const normalizedModern = modernConfig
    ? definition.normalizeConfig(modernConfig)
    : undefined;
  const rawConfig = legacyConfig
    && (!normalizedModern || JSON.stringify(normalizedModern) === JSON.stringify(definition.defaultConfig))
    ? legacyConfig
    : modernConfig;
  return definition.normalizeConfig(rawConfig);
}
