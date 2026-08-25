import { logger } from '../utils/logger';
import type { ScenarioModelPathConfig, ScenarioModelKind } from '../utils/scenarioModels';
import { scenarioModelFieldKey } from '../utils/scenarioModels';
import type {
  ModelCatalogModel,
  ModelCatalogSelectedIds,
  ModelCatalogSnapshot,
  ModelInfo,
  ModelRules,
  ModelSelectionPaths,
  ScenarioSelectedModelIds,
} from '../types/modelCatalog';

interface ModelRegistryServicePorts {
  getModelCatalogSnapshot: () => Promise<ModelCatalogSnapshot>;
  resolveModelCatalogSelectedIds: (paths: ModelSelectionPaths) => Promise<ModelCatalogSelectedIds>;
  getModelsDir: () => Promise<string>;
  join: (...paths: string[]) => Promise<string>;
  presetModelsMap: Map<string, ModelInfo>;
  defaultModelRules: ModelRules;
}

class ModelRegistryService {
  private latestCatalogSnapshot: ModelCatalogSnapshot | null = null;

  constructor(private readonly ports: ModelRegistryServicePorts) {}

  get latestSnapshot(): ModelCatalogSnapshot | null {
    return this.latestCatalogSnapshot;
  }

  async getModelCatalogSnapshot(): Promise<ModelCatalogSnapshot> {
    const snapshot = await this.ports.getModelCatalogSnapshot();
    this.latestCatalogSnapshot = snapshot;
    return snapshot;
  }

  async resolveModelCatalogSelectedIds(paths: ModelSelectionPaths): Promise<ModelCatalogSelectedIds> {
    return await this.ports.resolveModelCatalogSelectedIds(paths);
  }

  resolveModelCatalogSelectedIdsFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    paths: ModelSelectionPaths,
  ): ModelCatalogSelectedIds {
    return {
      streaming: resolveSelectedModelId(
        snapshot,
        paths.streamingModelPath,
        snapshot.selectionOptions.streaming,
      ),
      batch: resolveSelectedModelId(
        snapshot,
        paths.batchModelPath,
        snapshot.selectionOptions.batch,
      ),
      speakerSegmentation: resolveSelectedModelId(
        snapshot,
        paths.speakerSegmentationModelPath,
        snapshot.selectionOptions.speakerSegmentation,
      ),
      speakerEmbedding: resolveSelectedModelId(
        snapshot,
        paths.speakerEmbeddingModelPath,
        snapshot.selectionOptions.speakerEmbedding,
      ),
    };
  }

  resolveAsrSelectedModelIdsFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    paths: Pick<ModelSelectionPaths, 'streamingModelPath' | 'batchModelPath'>,
  ): Pick<ModelCatalogSelectedIds, 'streaming' | 'batch'> {
    return {
      streaming: resolveSelectedModelId(
        snapshot,
        paths.streamingModelPath,
        snapshot.selectionOptions.streaming,
      ),
      batch: resolveSelectedModelId(
        snapshot,
        paths.batchModelPath,
        snapshot.selectionOptions.batch,
      ),
    };
  }

  resolveScenarioSelectedModelIdsFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    config: ScenarioModelPathConfig,
  ): ScenarioSelectedModelIds {
    const sectionModelsByType = new Map<string, Array<{ id: string }>>();
    for (const section of snapshot.sections) {
      sectionModelsByType.set(
        section.type,
        section.groups.flatMap((group) => group.models),
      );
    }

    const kindToSectionType: Record<ScenarioModelKind, string> = {
      punctuationModelPath: 'punctuation',
      vadModelPath: 'vad',
      speakerSegmentationModelPath: 'speaker-segmentation',
      speakerEmbeddingModelPath: 'speaker-embedding',
    };

    const ids = {} as ScenarioSelectedModelIds;
    for (const scenario of ['live', 'batch'] as const) {
      for (const kind of Object.keys(kindToSectionType) as ScenarioModelKind[]) {
        const key = `${scenario}${kind
          .replace('ModelPath', '')
          .replace(/^./, (ch) => ch.toUpperCase())}` as keyof ScenarioSelectedModelIds;
        const options = sectionModelsByType.get(kindToSectionType[kind]) ?? [];
        ids[key] = resolveSelectedModelId(
          snapshot,
          config[scenarioModelFieldKey(kind, scenario)] ?? '',
          options,
        );
      }
    }
    return ids;
  }

  async resolveCatalogModel(modelId: string): Promise<ModelCatalogModel | undefined> {
    const cachedModel = this.latestCatalogSnapshot?.models.find(model => model.id === modelId);
    if (cachedModel) {
      return cachedModel;
    }

    try {
      const snapshot = await this.getModelCatalogSnapshot();
      return snapshot.models.find(model => model.id === modelId);
    } catch (error) {
      logger.warn('[ModelService] Failed to resolve model metadata from Rust catalog snapshot:', error);
      return undefined;
    }
  }

  resolvePresetModel(modelId: string): ModelInfo | undefined {
    return this.ports.presetModelsMap.get(modelId);
  }

  async getModelPath(modelId: string): Promise<string> {
    const cachedPath = this.latestCatalogSnapshot?.modelPathById[modelId];
    if (cachedPath) {
      return cachedPath;
    }

    const catalogModel = await this.resolveCatalogModel(modelId);
    if (catalogModel?.installPath) {
      return catalogModel.installPath;
    }

    const model = this.resolvePresetModel(modelId);
    if (!model) throw new Error('Model not found');

    const modelsDir = await this.ports.getModelsDir();
    if (model.filename) {
      return await this.ports.join(modelsDir, model.filename);
    }
    return await this.ports.join(modelsDir, modelId);
  }

  getModelRules(modelId: string): ModelRules {
    const snapshotModel = this.latestCatalogSnapshot?.models.find(model => model.id === modelId);
    if (snapshotModel?.rules) {
      return snapshotModel.rules;
    }

    const model = this.resolvePresetModel(modelId);
    if (model?.rules) {
      return model.rules;
    }
    return this.ports.defaultModelRules;
  }
}

function normalizeCatalogPath(path: string): string {
  return path.replace(/\\/g, '/').toLowerCase();
}

function resolveSelectedModelId(
  snapshot: ModelCatalogSnapshot,
  modelPath: string,
  options: Array<{ id: string }>,
): string | null {
  if (!modelPath.trim()) {
    return null;
  }

  const normalizedPath = normalizeCatalogPath(modelPath);
  const exactModelId = snapshot.modelIdByNormalizedPath[normalizedPath];
  if (exactModelId && options.some((option) => option.id === exactModelId)) {
    return exactModelId;
  }

  for (const option of options) {
    const token = snapshot.pathMatchTokens.find((item) => item.id === option.id);
    if (token && token.token && normalizedPath.includes(token.token)) {
      return option.id;
    }
  }

  return null;
}

export function createModelRegistryService(ports: ModelRegistryServicePorts): ModelRegistryService {
  return new ModelRegistryService(ports);
}
