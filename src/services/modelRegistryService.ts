import { logger } from '../utils/logger';
import type {
  ModelCatalogModel,
  ModelCatalogSelectedIds,
  ModelCatalogSnapshot,
  ModelInfo,
  ModelRules,
  ModelSelectionPaths,
} from './modelService';

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

export function createModelRegistryService(ports: ModelRegistryServicePorts): ModelRegistryService {
  return new ModelRegistryService(ports);
}
