import onlineAsrProviderManifest from '../shared/online-asr-providers.json';
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

type OnlineAsrProviderManifest = typeof onlineAsrProviderManifest;
type OnlineAsrProviderManifestEntry = OnlineAsrProviderManifest['providers'][number];

type VolcengineManifestEntry = OnlineAsrProviderManifestEntry & {
  id: 'volcengine-doubao';
  defaults: VolcengineDoubaoAsrProviderConfig;
};

function providerManifestEntry(providerId: OnlineAsrProviderId): OnlineAsrProviderManifestEntry {
  const entry = onlineAsrProviderManifest.providers.find((provider) => provider.id === providerId);
  if (!entry) {
    throw new Error(`Missing online ASR provider manifest entry: ${providerId}`);
  }
  return entry;
}

const VOLCENGINE_DOUBAO_MANIFEST = providerManifestEntry('volcengine-doubao') as VolcengineManifestEntry;
const VOLCENGINE_DOUBAO_LOCAL_FILE_BATCH = VOLCENGINE_DOUBAO_MANIFEST.batch.localFileMode;

export const VOLCENGINE_DOUBAO_PROVIDER_ID: OnlineAsrProviderId = VOLCENGINE_DOUBAO_MANIFEST.id;
export const VOLCENGINE_DOUBAO_PROFILE_ID = VOLCENGINE_DOUBAO_MANIFEST.profileId;
export const VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT = VOLCENGINE_DOUBAO_LOCAL_FILE_BATCH.endpoint;
export const VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID = VOLCENGINE_DOUBAO_LOCAL_FILE_BATCH.resourceId;

export const DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG: VolcengineDoubaoAsrProviderConfig = {
  ...VOLCENGINE_DOUBAO_MANIFEST.defaults,
};

function normalizedEndpoint(endpoint: string | undefined): string {
  return endpoint?.trim().replace(/\/+$/, '') ?? '';
}

function normalizedResourceId(resourceId: string | undefined): string {
  return resourceId?.trim() ?? '';
}

export function isVolcengineFlashBatchMode(
  provider: Pick<VolcengineDoubaoAsrProviderConfig, 'batchEndpoint' | 'batchResourceId'> | undefined,
): boolean {
  return normalizedEndpoint(provider?.batchEndpoint) === normalizedEndpoint(VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT)
    && normalizedResourceId(provider?.batchResourceId) === VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID;
}

export function normalizeVolcengineDoubaoConfig(
  provider: Partial<VolcengineDoubaoAsrProviderConfig> | undefined,
): VolcengineDoubaoAsrProviderConfig {
  const defaults = DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG;
  const batchEndpoint = provider?.batchEndpoint?.trim() || defaults.batchEndpoint;
  const batchResourceId = provider?.batchResourceId?.trim() || defaults.batchResourceId;
  const flashBatch = isVolcengineFlashBatchMode({ batchEndpoint, batchResourceId })
    ? { batchEndpoint, batchResourceId }
    : {
        batchEndpoint: defaults.batchEndpoint,
        batchResourceId: defaults.batchResourceId,
      };

  return {
    apiKey: provider?.apiKey?.trim() ?? defaults.apiKey,
    streamingEndpoint: provider?.streamingEndpoint?.trim() || defaults.streamingEndpoint,
    streamingResourceId: provider?.streamingResourceId?.trim() || defaults.streamingResourceId,
    ...flashBatch,
  };
}

function hasRequiredConfigFields(
  config: VolcengineDoubaoAsrProviderConfig,
  requiredFields: readonly string[],
): boolean {
  return requiredFields.every((field) => {
    switch (field) {
      case 'apiKey':
        return Boolean(config.apiKey.trim());
      case 'streamingEndpoint':
        return Boolean(config.streamingEndpoint.trim());
      case 'streamingResourceId':
        return Boolean(config.streamingResourceId.trim());
      case 'batchEndpoint':
        return Boolean(config.batchEndpoint.trim());
      case 'batchResourceId':
        return Boolean(config.batchResourceId.trim());
      default:
        return false;
    }
  });
}

function isVolcengineConfigured(
  config: VolcengineDoubaoAsrProviderConfig,
  mode: AsrMode,
): boolean {
  if (!config.apiKey.trim()) {
    return false;
  }
  if (mode === 'streaming') {
    return hasRequiredConfigFields(config, VOLCENGINE_DOUBAO_MANIFEST.streaming.requiredConfigFields);
  }
  return hasRequiredConfigFields(config, VOLCENGINE_DOUBAO_MANIFEST.batch.requiredConfigFields)
    && (!VOLCENGINE_DOUBAO_LOCAL_FILE_BATCH.supported || isVolcengineFlashBatchMode(config));
}

function definitionFromManifest(
  entry: VolcengineManifestEntry,
): OnlineAsrProviderDefinition<VolcengineDoubaoAsrProviderConfig> {
  return {
    id: entry.id,
    profileId: entry.profileId,
    optionLabelKey: entry.ui.optionLabelKey,
    optionDefaultLabel: entry.ui.optionDefaultLabel,
    titleKey: entry.ui.titleKey,
    titleDefault: entry.ui.titleDefault,
    cloudUploadHintKey: entry.ui.cloudUploadHintKey,
    cloudUploadHintDefault: entry.ui.cloudUploadHintDefault,
    defaultConfig: DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
    normalizeConfig: normalizeVolcengineDoubaoConfig,
    isConfigured: isVolcengineConfigured,
  };
}

export const ONLINE_ASR_PROVIDER_DEFINITIONS = [
  definitionFromManifest(VOLCENGINE_DOUBAO_MANIFEST),
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
