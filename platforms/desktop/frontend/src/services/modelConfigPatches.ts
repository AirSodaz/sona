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
import type { ModelCatalogRestoreDefaults, ModelInfo } from '../types/modelCatalog';

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
    if (model.modes.includes('batch')) {
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
      return { liveVadModelPath: path, batchVadModelPath: path };
    case 'punctuation':
      return { livePunctuationModelPath: path, batchPunctuationModelPath: path };
    case 'speaker-segmentation':
      return {
        liveSpeakerSegmentationModelPath: path,
        batchSpeakerSegmentationModelPath: path,
      };
    case 'speaker-embedding':
      return {
        liveSpeakerEmbeddingModelPath: path,
        batchSpeakerEmbeddingModelPath: path,
      };
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
  const asr = createDefaultAsrConfig(config.streamingModelPath, config.batchModelPath);

  if (config.asr?.selections) {
    asr.selections = { ...config.asr.selections };
  }

  if (config.streamingModelPath === deletedPath) {
    updates.streamingModelPath = '';
    asr.selections.live = { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' };
    asr.selections.caption = { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' };
    asr.selections.voiceTyping = { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' };
  }

  if (config.batchModelPath === deletedPath) {
    updates.batchModelPath = '';
    asr.selections.batch = { engine: 'local', mode: 'batch', modelId: null, modelPath: '' };
  }

  if (config.livePunctuationModelPath === deletedPath) {
    updates.livePunctuationModelPath = '';
  }

  if (config.batchPunctuationModelPath === deletedPath) {
    updates.batchPunctuationModelPath = '';
  }

  if (config.liveVadModelPath === deletedPath) {
    updates.liveVadModelPath = '';
  }

  if (config.batchVadModelPath === deletedPath) {
    updates.batchVadModelPath = '';
  }

  if (config.liveSpeakerSegmentationModelPath === deletedPath) {
    updates.liveSpeakerSegmentationModelPath = '';
  }

  if (config.batchSpeakerSegmentationModelPath === deletedPath) {
    updates.batchSpeakerSegmentationModelPath = '';
  }

  if (config.liveSpeakerEmbeddingModelPath === deletedPath) {
    updates.liveSpeakerEmbeddingModelPath = '';
  }

  if (config.batchSpeakerEmbeddingModelPath === deletedPath) {
    updates.batchSpeakerEmbeddingModelPath = '';
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
    livePunctuationModelPath: defaults.punctuationModelPath ?? '',
    batchPunctuationModelPath: defaults.punctuationModelPath ?? '',
    liveVadModelPath: defaults.vadModelPath ?? '',
    batchVadModelPath: defaults.vadModelPath ?? '',
    liveSpeakerSegmentationModelPath: defaults.speakerSegmentationModelPath ?? '',
    batchSpeakerSegmentationModelPath: defaults.speakerSegmentationModelPath ?? '',
    liveSpeakerEmbeddingModelPath: defaults.speakerEmbeddingModelPath ?? '',
    batchSpeakerEmbeddingModelPath: defaults.speakerEmbeddingModelPath ?? '',
    batchVadEnabled: defaults.batchVadEnabled ?? true,
    liveVadBufferSize: Number.isFinite(defaults.vadBufferSize) ? defaults.vadBufferSize : 5,
    batchVadBufferSize: Number.isFinite(defaults.vadBufferSize) ? defaults.vadBufferSize : 5,
    maxConcurrent: Number.isFinite(defaults.maxConcurrent) ? defaults.maxConcurrent : 2,
    enableITN: defaults.enableITN,
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

  if (defaults.batchModelPath !== undefined) {
    Object.assign(updates, syncLegacyAsrSelectionFields(
      { ...config, ...updates },
      'batch',
      {
        modelId: null,
        modelPath: defaults.batchModelPath,
      },
    ));
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
