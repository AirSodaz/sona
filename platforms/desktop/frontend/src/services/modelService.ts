import type {
  ModelCatalogSelectedIds as CoreModelCatalogSelectedIds,
  ModelCatalogSnapshot as CoreModelCatalogSnapshot,
} from '../bindings';
import {
  getPlatform,
  type IPlatformPorts,
  type ITransport,
  type PlatformContext,
  TauriCommand,
} from '../platform';
import {
  DEFAULT_MODEL_RULES,
  type ModelCatalogSelectedIds,
  type ModelCatalogSnapshot,
  type ModelRules,
  type ModelSelectionPaths,
  PRESET_MODELS_MAP,
  type ProgressCallback,
} from '../types/modelCatalog';
import type { ScenarioModelPathConfig } from '../utils/scenarioModels';
import {
  normalizeModelCatalogSelectedIds,
  normalizeModelCatalogSnapshot,
} from './modelCatalogNormalizers';
import { createModelDownloadService, type ModelDownloadService } from './modelDownloadService';
import { createModelFileService, type ModelFileService } from './modelFileService';
import { createModelRegistryService, type ModelRegistryService } from './modelRegistryService';

export type { ModelFileConfig } from '../types/model';
export type {
  ModelCatalogModel,
  ModelCatalogRestoreDefaults,
  ModelCatalogSelectedIds,
  ModelCatalogSnapshot,
  ModelInfo,
  ModelRules,
  ModelSelectionPaths,
  ProgressCallback,
  TimestampSupportHint,
} from '../types/modelCatalog';
export {
  DEFAULT_MODEL_RULES,
  PRESET_MODELS,
  PRESET_MODELS_MAP,
} from '../types/modelCatalog';

export interface ModelServicePorts {
  fileService: ModelFileService;
  registryService: ModelRegistryService;
  downloadService: ModelDownloadService;
  deletePresetModel?: (modelId: string) => Promise<void>;
  fs?: IPlatformPorts['fs'];
}

/**
 * Service for managing AI models (downloading, verifying, path resolution).
 */
export class ModelService {
  constructor(private readonly ports: ModelServicePorts) {}

  /**
   * Gets the local directory where models are stored.
   *
   * Creates the directory if it does not exist.
   *
   * @return A promise that resolves to the absolute path of the models directory.
   */
  async getModelsDir(): Promise<string> {
    return this.ports.fileService.getModelsDir();
  }

  /**
   * Gets a settings-ready model catalog snapshot with app-local install status.
   *
   * @return A promise resolving to grouped model metadata and install paths.
   */
  async getModelCatalogSnapshot(): Promise<ModelCatalogSnapshot> {
    return this.ports.registryService.getModelCatalogSnapshot();
  }

  async resolveModelCatalogSelectedIds(
    paths: ModelSelectionPaths
  ): Promise<ModelCatalogSelectedIds> {
    return await this.ports.registryService.resolveModelCatalogSelectedIds(paths);
  }

  resolveModelCatalogSelectedIdsFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    paths: ModelSelectionPaths
  ): ModelCatalogSelectedIds {
    return this.ports.registryService.resolveModelCatalogSelectedIdsFromSnapshot(snapshot, paths);
  }

  resolveAsrSelectedModelIdsFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    paths: Pick<ModelSelectionPaths, 'streamingModelPath' | 'batchModelPath'>
  ): Pick<ModelCatalogSelectedIds, 'streaming' | 'batch'> {
    return this.ports.registryService.resolveAsrSelectedModelIdsFromSnapshot(snapshot, paths);
  }

  resolveScenarioSelectedModelIdsFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    config: ScenarioModelPathConfig
  ) {
    return this.ports.registryService.resolveScenarioSelectedModelIdsFromSnapshot(snapshot, config);
  }

  /**
   * Checks if the user's hardware is compatible with a specific model.
   *
   * @param modelId The ID of the model to check.
   * @return A promise resolving to an object with compatibility status and optional reason.
   */
  async checkHardware(modelId: string): Promise<{ compatible: boolean; reason?: string }> {
    const model = PRESET_MODELS_MAP.get(modelId);
    if (!model) return { compatible: false, reason: 'Model not found' };

    return { compatible: true };
  }

  /**
   * Downloads a model by its ID.
   *
   * Handles mirrors, progress reporting, and cancellation.
   *
   * @param modelId The ID of the model to download.
   * @param onProgress Optional callback for progress updates.
   * @param signal Optional AbortSignal to cancel the download.
   * @param mirror Optional mirror key to use for the download.
   * @return A promise resolving to the local path of the downloaded model.
   * @throws {Error} If the model is not found or download fails.
   */
  async downloadModel(
    modelId: string,
    onProgress?: ProgressCallback,
    signal?: AbortSignal,
    mirror?: string
  ): Promise<string> {
    const catalogModel = await this.ports.registryService.resolveCatalogModel(modelId);
    const model = catalogModel ?? PRESET_MODELS_MAP.get(modelId);
    if (!model) throw new Error('Model not found');

    const modelsDir =
      this.ports.registryService.latestSnapshot?.modelsDir ?? (await this.getModelsDir());
    return await this.ports.downloadService.downloadModel({
      modelId,
      model,
      modelsDir,
      onProgress,
      signal,
      mirror,
    });
  }

  /**
   * Resolves the local file system path for a given model ID.
   *
   * @param modelId The ID of the model.
   * @return A promise resolving to the model's path.
   */
  async getModelPath(modelId: string): Promise<string> {
    return await this.ports.registryService.getModelPath(modelId);
  }

  /**
   * Checks if a model is currently installed.
   *
   * @param modelId The ID of the model.
   * @return A promise resolving to true if installed, false otherwise.
   */
  async isModelInstalled(modelId: string): Promise<boolean> {
    const catalogModel = await this.ports.registryService.resolveCatalogModel(modelId);
    if (catalogModel) {
      return catalogModel.isInstalled;
    }

    const modelPath = await this.getModelPath(modelId);
    try {
      if (this.ports.fs) {
        return await this.ports.fs.exists(modelPath);
      }
      return await this.ports.fileService.exists(modelPath);
    } catch {
      return false;
    }
  }

  /**
   * Deletes an installed model.
   *
   * @param modelId The ID of the model to delete.
   * @return A promise resolving when deletion is complete.
   */
  async deleteModel(modelId: string): Promise<void> {
    if (this.ports.deletePresetModel) {
      await this.ports.deletePresetModel(modelId);
      return;
    }
    const modelPath = await this.getModelPath(modelId);
    await this.ports.fileService.removeIfExists(modelPath);
  }

  /**
   * Gets the model rules for a specific model ID.
   * If the model defines custom rules, those are used.
   * Otherwise, defaults to DEFAULT_MODEL_RULES.
   *
   * @param modelId The ID of the model.
   * @returns The ModelRules for the model.
   */
  getModelRules(modelId: string): ModelRules {
    return this.ports.registryService.getModelRules(modelId);
  }
}

export function buildModelServicePortsFromPlatform(platform: {
  transport: ITransport;
  ports: IPlatformPorts;
}): ModelServicePorts {
  const fileService = createModelFileService({
    appLocalDataDir: () => platform.ports.path.appLocalDataDir(),
    join: (...paths) => Promise.resolve(platform.ports.path.join(...paths)),
    exists: (path) => platform.ports.fs.exists(path),
    mkdir: (path, options) => platform.ports.fs.mkdir(path, options),
    remove: (path, options) => platform.ports.fs.remove(path, options),
    getStorageDirectories: () => platform.transport.invoke(TauriCommand.storage.getDirectories),
  });

  const registryService = createModelRegistryService({
    getModelCatalogSnapshot: async () => {
      const raw = await platform.transport.invoke<CoreModelCatalogSnapshot>(
        TauriCommand.app.getModelCatalogSnapshot
      );
      return normalizeModelCatalogSnapshot(raw);
    },
    resolveModelCatalogSelectedIds: async (paths) => {
      const raw = await platform.transport.invoke<CoreModelCatalogSelectedIds>(
        TauriCommand.app.resolveModelCatalogSelectedIds,
        { paths }
      );
      return normalizeModelCatalogSelectedIds(raw);
    },
    getModelsDir: () => fileService.getModelsDir(),
    join: (...paths) => Promise.resolve(platform.ports.path.join(...paths)),
    presetModelsMap: PRESET_MODELS_MAP,
    defaultModelRules: DEFAULT_MODEL_RULES,
  });

  const downloadService = createModelDownloadService({
    downloadFile: (req) => platform.transport.invoke(TauriCommand.app.downloadFile, req),
    downloadPresetModel: (req) =>
      platform.transport.invoke(TauriCommand.app.downloadPresetModel, req),
    extractTarBz2: (req) => platform.transport.invoke(TauriCommand.app.extractTarBz2, req),
    cancelDownload: async (id) => {
      await platform.transport.invoke(TauriCommand.app.cancelDownload, { id });
    },
    remove: async (path) => {
      await platform.ports.fs.remove(path);
    },
    listen: <T>(event: string, handler: (event: { payload: T }) => void) => {
      const unlisten = platform.transport.listen<T>(event, (payload) => handler({ payload }));
      return Promise.resolve(unlisten);
    },
    join: (...paths) => Promise.resolve(platform.ports.path.join(...paths)),
    getModelsDir: () => fileService.getModelsDir(),
  });

  return {
    fileService,
    registryService,
    downloadService,
    deletePresetModel: (modelId) =>
      platform.transport.invoke(TauriCommand.app.deletePresetModel, { modelId }),
    fs: platform.ports.fs,
  };
}

export type ModelServiceInput =
  | ModelServicePorts
  | PlatformContext
  | {
      transport: ITransport;
      ports: IPlatformPorts;
    };

export function createModelService(input?: ModelServiceInput): ModelService {
  if (!input) {
    return new ModelService(buildModelServicePortsFromPlatform(getPlatform()));
  }
  if ('transport' in input) {
    return new ModelService(buildModelServicePortsFromPlatform(input));
  }
  return new ModelService(input);
}

export const modelService = createModelService();
