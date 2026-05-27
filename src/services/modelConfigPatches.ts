import type { AppConfig } from '../types/config';
import {
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
  createDefaultAsrConfig,
  syncLegacyAsrSelectionFields,
  syncOnlineAsrProviderConfig,
  syncStreamingAsrSelectionFields,
} from './asrConfigService';
import type { ModelCatalogRestoreDefaults, ModelInfo } from './modelService';

export function buildModelPathConfigPatch(
  config: AppConfig,
  model: ModelInfo,
  path: string,
): Partial<AppConfig> {
  const updates: Partial<AppConfig> = {};

  if (model.modes && model.modes.length > 0) {
    if (model.modes.includes('streaming')) {
      Object.assign(updates, syncStreamingAsrSelectionFields(config, {
        modelId: model.id,
        modelPath: path,
      }));
    }
    if (model.modes.includes('offline')) {
      Object.assign(updates, syncLegacyAsrSelectionFields(
        { ...config, ...updates },
        'batch',
        {
          modelId: model.id,
          modelPath: path,
        },
      ));
    }
    return updates;
  }

  switch (model.type) {
    case 'vad':
      return { vadModelPath: path };
    case 'punctuation':
      return { punctuationModelPath: path };
    case 'speaker-segmentation':
      return { speakerSegmentationModelPath: path };
    case 'speaker-embedding':
      return { speakerEmbeddingModelPath: path };
    case 'itn':
      return {};
    default:
      return updates;
  }
}

export function buildModelRemovalConfigPatch(
  config: AppConfig,
  deletedPath: string,
): Partial<AppConfig> {
  const updates: Partial<AppConfig> = {};
  const asr = createDefaultAsrConfig(config.streamingModelPath, config.offlineModelPath);

  if (config.asr?.selections) {
    asr.selections = { ...config.asr.selections };
  }

  if (config.streamingModelPath === deletedPath) {
    updates.streamingModelPath = '';
    asr.selections.live = { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' };
    asr.selections.caption = { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' };
    asr.selections.voiceTyping = { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' };
  }

  if (config.offlineModelPath === deletedPath) {
    updates.offlineModelPath = '';
    asr.selections.batch = { engine: 'local-sherpa', mode: 'offline', modelId: null, modelPath: '' };
  }

  if (config.punctuationModelPath === deletedPath) {
    updates.punctuationModelPath = '';
  }

  if (config.vadModelPath === deletedPath) {
    updates.vadModelPath = '';
  }

  if (config.speakerSegmentationModelPath === deletedPath) {
    updates.speakerSegmentationModelPath = '';
  }

  if (config.speakerEmbeddingModelPath === deletedPath) {
    updates.speakerEmbeddingModelPath = '';
  }

  return {
    ...updates,
    asr,
  };
}

export function buildRestoreDefaultModelConfigPatch(
  config: AppConfig,
  defaults: ModelCatalogRestoreDefaults,
): Partial<AppConfig> {
  const updates: Partial<AppConfig> = {
    punctuationModelPath: defaults.punctuationModelPath ?? '',
    vadBufferSize: Number.isFinite(defaults.vadBufferSize) ? defaults.vadBufferSize : 5,
    maxConcurrent: Number.isFinite(defaults.maxConcurrent) ? defaults.maxConcurrent : 2,
    enableITN: defaults.enableITN,
    speakerSegmentationModelPath: defaults.speakerSegmentationModelPath ?? '',
    speakerEmbeddingModelPath: defaults.speakerEmbeddingModelPath ?? '',
  };

  if (defaults.streamingModelPath !== undefined) {
    Object.assign(updates, syncStreamingAsrSelectionFields(
      { ...config, ...updates },
      {
        modelId: null,
        modelPath: defaults.streamingModelPath,
      },
    ));
  }

  if (defaults.offlineModelPath !== undefined) {
    Object.assign(updates, syncLegacyAsrSelectionFields(
      { ...config, ...updates },
      'batch',
      {
        modelId: null,
        modelPath: defaults.offlineModelPath,
      },
    ));
  }

  if (defaults.vadModelPath !== undefined) {
    updates.vadModelPath = defaults.vadModelPath;
  }

  Object.assign(updates, syncOnlineAsrProviderConfig(
    { ...config, ...updates },
    VOLCENGINE_DOUBAO_PROVIDER_ID,
    {
      batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
      batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
    },
  ));

  return updates;
}
