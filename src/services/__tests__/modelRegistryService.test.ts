import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_MODEL_RULES,
  PRESET_MODELS_MAP,
  type ModelCatalogSnapshot,
} from '../modelService';
import { createModelRegistryService } from '../modelRegistryService';

const snapshotModel = {
  id: 'snapshot-model',
  name: 'Snapshot Model',
  description: '',
  url: 'https://example.com/snapshot.tar.bz2',
  type: 'sensevoice',
  modes: ['streaming'],
  language: 'en',
  size: '1 MB',
  isArchive: true,
  engine: 'sherpa-onnx',
  installPath: '/snapshot/models/snapshot-model',
  downloadPath: '/snapshot/downloads/snapshot.tar.bz2',
  isInstalled: true,
  rules: {
    requiresVad: false,
    requiresPunctuation: true,
  },
} as const;

function makeSnapshot(): ModelCatalogSnapshot {
  return {
    modelsDir: '/snapshot/models',
    models: [snapshotModel as any],
    sections: [],
    selectionOptions: {
      streaming: [],
      offline: [],
      speakerSegmentation: [],
      speakerEmbedding: [],
    },
    modelPathById: {
      [snapshotModel.id]: snapshotModel.installPath,
    },
    modelIdByNormalizedPath: {},
    pathMatchTokens: [],
    dependencyRequestsByModelId: {},
    restoreDefaults: {
      punctuationModelPath: '',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: '',
      enableITN: true,
      vadBufferSize: 5,
      maxConcurrent: 2,
    },
  };
}

describe('modelRegistryService', () => {
  const getModelCatalogSnapshot = vi.fn();
  const resolveModelCatalogSelectedIds = vi.fn();
  const getModelsDir = vi.fn();
  const join = vi.fn((...parts: string[]) => Promise.resolve(parts.join('/')));

  beforeEach(() => {
    vi.clearAllMocks();
    getModelCatalogSnapshot.mockResolvedValue(makeSnapshot());
    resolveModelCatalogSelectedIds.mockResolvedValue({
      streaming: 'streaming-id',
      offline: null,
      speakerSegmentation: null,
      speakerEmbedding: null,
    });
    getModelsDir.mockResolvedValue('/app/data/models');
  });

  it('loads and caches the latest catalog snapshot', async () => {
    const registry = createModelRegistryService({
      getModelCatalogSnapshot,
      resolveModelCatalogSelectedIds,
      getModelsDir,
      join,
      presetModelsMap: PRESET_MODELS_MAP,
      defaultModelRules: DEFAULT_MODEL_RULES,
    });

    const snapshot = await registry.getModelCatalogSnapshot();
    const model = await registry.resolveCatalogModel(snapshotModel.id);

    expect(snapshot.models[0].id).toBe(snapshotModel.id);
    expect(model?.installPath).toBe(snapshotModel.installPath);
    expect(getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
  });

  it('resolves paths from snapshot, catalog model, preset filename, and model id fallback', async () => {
    const registry = createModelRegistryService({
      getModelCatalogSnapshot,
      resolveModelCatalogSelectedIds,
      getModelsDir,
      join,
      presetModelsMap: PRESET_MODELS_MAP,
      defaultModelRules: DEFAULT_MODEL_RULES,
    });
    const filenameModel = [...PRESET_MODELS_MAP.values()].find((model) => model.filename);
    const directoryModel = [...PRESET_MODELS_MAP.values()].find((model) => !model.filename);

    await registry.getModelCatalogSnapshot();

    await expect(registry.getModelPath(snapshotModel.id)).resolves.toBe(snapshotModel.installPath);
    await expect(registry.getModelPath(filenameModel!.id)).resolves.toBe(`/app/data/models/${filenameModel!.filename}`);
    await expect(registry.getModelPath(directoryModel!.id)).resolves.toBe(`/app/data/models/${directoryModel!.id}`);
  });

  it('returns snapshot rules before preset rules and falls back to defaults', async () => {
    const registry = createModelRegistryService({
      getModelCatalogSnapshot,
      resolveModelCatalogSelectedIds,
      getModelsDir,
      join,
      presetModelsMap: PRESET_MODELS_MAP,
      defaultModelRules: DEFAULT_MODEL_RULES,
    });
    const presetWithRules = [...PRESET_MODELS_MAP.values()].find((model) => model.rules);

    await registry.getModelCatalogSnapshot();

    expect(registry.getModelRules(snapshotModel.id)).toEqual(snapshotModel.rules);
    expect(registry.getModelRules(presetWithRules!.id)).toEqual(presetWithRules!.rules);
    expect(registry.getModelRules('missing-model')).toEqual(DEFAULT_MODEL_RULES);
  });

  it('delegates selected id resolution to the registry port', async () => {
    const registry = createModelRegistryService({
      getModelCatalogSnapshot,
      resolveModelCatalogSelectedIds,
      getModelsDir,
      join,
      presetModelsMap: PRESET_MODELS_MAP,
      defaultModelRules: DEFAULT_MODEL_RULES,
    });

    await expect(registry.resolveModelCatalogSelectedIds({
      streamingModelPath: '/streaming',
      offlineModelPath: '',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: '',
    })).resolves.toEqual({
      streaming: 'streaming-id',
      offline: null,
      speakerSegmentation: null,
      speakerEmbedding: null,
    });

    expect(resolveModelCatalogSelectedIds).toHaveBeenCalledWith({
      streamingModelPath: '/streaming',
      offlineModelPath: '',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: '',
    });
  });

  it('resolves selected ids from a snapshot without calling the backend port', async () => {
    const snapshot = {
      ...makeSnapshot(),
      selectionOptions: {
        streaming: [
          {
            id: snapshotModel.id,
            label: snapshotModel.name,
            installPath: snapshotModel.installPath,
            isInstalled: true,
          },
        ],
        offline: [
          {
            id: snapshotModel.id,
            label: snapshotModel.name,
            installPath: snapshotModel.installPath,
            isInstalled: true,
          },
        ],
        speakerSegmentation: [],
        speakerEmbedding: [],
      },
      modelIdByNormalizedPath: {
        '/snapshot/models/snapshot-model': snapshotModel.id,
      },
      pathMatchTokens: [
        {
          id: snapshotModel.id,
          token: 'snapshot-model',
        },
      ],
    };
    const registry = createModelRegistryService({
      getModelCatalogSnapshot,
      resolveModelCatalogSelectedIds,
      getModelsDir,
      join,
      presetModelsMap: PRESET_MODELS_MAP,
      defaultModelRules: DEFAULT_MODEL_RULES,
    });

    const result = registry.resolveModelCatalogSelectedIdsFromSnapshot(snapshot, {
      streamingModelPath: 'C:\\snapshot\\models\\snapshot-model',
      offlineModelPath: 'D:\\portable\\snapshot-model',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: '',
    });

    expect(result).toEqual({
      streaming: snapshotModel.id,
      offline: snapshotModel.id,
      speakerSegmentation: null,
      speakerEmbedding: null,
    });
    expect(resolveModelCatalogSelectedIds).not.toHaveBeenCalled();
  });
});
