import type {
  AsrMode,
  AsrModelSelection,
  OnlineAsrProviderId,
  VolcengineDoubaoAsrProviderConfig,
} from '../types/config';

export type OnlineAsrProviderConfigMap = {
  'volcengine-doubao': VolcengineDoubaoAsrProviderConfig;
};

export type OnlineAsrProviderConfig = OnlineAsrProviderConfigMap[OnlineAsrProviderId];

export type OnlineAsrProviderRequest = {
  providerId: OnlineAsrProviderId;
  profileId: string;
  config: OnlineAsrProviderConfig;
};

export type OnlineAsrProviderDefinition<TConfig extends OnlineAsrProviderConfig = OnlineAsrProviderConfig> = {
  id: OnlineAsrProviderId;
  profileId: string;
  optionLabelKey: string;
  optionDefaultLabel: string;
  titleKey: string;
  titleDefault: string;
  cloudUploadHintKey: string;
  cloudUploadHintDefault: string;
  defaultConfig: TConfig;
  normalizeConfig: (config: Partial<TConfig> | undefined) => TConfig;
  isConfigured: (config: TConfig, mode: AsrMode) => boolean;
};

export const VOLCENGINE_DOUBAO_PROVIDER_ID: OnlineAsrProviderId = 'volcengine-doubao';
export const VOLCENGINE_DOUBAO_PROFILE_ID = 'volcengine-doubao-default';
export const VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT =
  'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash';
export const VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID = 'volc.bigasr.auc_turbo';

export const DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG: VolcengineDoubaoAsrProviderConfig = {
  apiKey: '',
  streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
  streamingResourceId: 'volc.seedasr.sauc.duration',
  batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
};

export function isVolcengineFlashBatchMode(
  provider: Pick<VolcengineDoubaoAsrProviderConfig, 'batchEndpoint' | 'batchResourceId'> | undefined,
): boolean {
  return provider?.batchEndpoint.trim().replace(/\/+$/, '') === VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT
    && provider.batchResourceId.trim() === VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID;
}

export function normalizeVolcengineDoubaoConfig(
  provider: Partial<VolcengineDoubaoAsrProviderConfig> | undefined,
): VolcengineDoubaoAsrProviderConfig {
  const batchEndpoint = provider?.batchEndpoint?.trim() || VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT;
  const batchResourceId = provider?.batchResourceId?.trim() || VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID;
  const flashBatch = isVolcengineFlashBatchMode({ batchEndpoint, batchResourceId })
    ? { batchEndpoint, batchResourceId }
    : {
        batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
        batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
      };

  return {
    apiKey: provider?.apiKey?.trim() ?? '',
    streamingEndpoint: provider?.streamingEndpoint?.trim()
      || DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.streamingEndpoint,
    streamingResourceId: provider?.streamingResourceId?.trim()
      || DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.streamingResourceId,
    ...flashBatch,
  };
}

function isVolcengineConfigured(
  config: VolcengineDoubaoAsrProviderConfig,
  mode: AsrMode,
): boolean {
  if (!config.apiKey.trim()) {
    return false;
  }
  if (mode === 'streaming') {
    return Boolean(config.streamingEndpoint.trim() && config.streamingResourceId.trim());
  }
  return isVolcengineFlashBatchMode(config);
}

export const ONLINE_ASR_PROVIDER_DEFINITIONS = [
  {
    id: VOLCENGINE_DOUBAO_PROVIDER_ID,
    profileId: VOLCENGINE_DOUBAO_PROFILE_ID,
    optionLabelKey: 'settings.asr.volcengine_doubao_option',
    optionDefaultLabel: '豆包语音 (云端)',
    titleKey: 'settings.asr.volcengine_title',
    titleDefault: '火山引擎语音服务 (火山 ASR)',
    cloudUploadHintKey: 'settings.asr.cloud_upload_hint',
    cloudUploadHintDefault: '音频会发送到火山引擎进行识别。',
    defaultConfig: DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
    normalizeConfig: normalizeVolcengineDoubaoConfig,
    isConfigured: isVolcengineConfigured,
  } satisfies OnlineAsrProviderDefinition<VolcengineDoubaoAsrProviderConfig>,
] as const;

export const ONLINE_ASR_PROVIDER_MAP = new Map<OnlineAsrProviderId, OnlineAsrProviderDefinition>(
  ONLINE_ASR_PROVIDER_DEFINITIONS.map((definition) => [definition.id, definition]),
);

export function isOnlineAsrProviderId(value: string | null | undefined): value is OnlineAsrProviderId {
  return value === VOLCENGINE_DOUBAO_PROVIDER_ID;
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

export function getOnlineProviderConfig<TProvider extends OnlineAsrProviderId>(
  providers: { online?: Record<string, unknown>; volcengineDoubao?: VolcengineDoubaoAsrProviderConfig } | undefined,
  providerId: TProvider,
): OnlineAsrProviderConfigMap[TProvider] {
  const definition = getOnlineAsrProviderDefinition(providerId);
  if (!definition) {
    throw new Error(`Unsupported online ASR provider: ${providerId}`);
  }
  const modernConfig = providers?.online?.[providerId];
  const legacyConfig = providerId === VOLCENGINE_DOUBAO_PROVIDER_ID
    ? providers?.volcengineDoubao
    : undefined;
  const normalizedModern = modernConfig
    ? definition.normalizeConfig(modernConfig as never)
    : undefined;
  const rawConfig = legacyConfig
    && (!normalizedModern || JSON.stringify(normalizedModern) === JSON.stringify(definition.defaultConfig))
    ? legacyConfig
    : modernConfig;
  return definition.normalizeConfig(rawConfig as never) as OnlineAsrProviderConfigMap[TProvider];
}
