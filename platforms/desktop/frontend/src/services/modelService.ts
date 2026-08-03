import {
    cancelDownload,
    downloadFile,
    extractTarBz2,
    getModelCatalogSnapshot as getModelCatalogSnapshotFromRust,
    resolveModelCatalogSelectedIds as resolveModelCatalogSelectedIdsFromRust,
} from './tauri/app';
import {
    DEFAULT_MODEL_RULES,
    PRESET_MODELS_MAP,
    type ModelCatalogSelectedIds,
    type ModelCatalogSnapshot,
    type ModelRules,
    type ModelSelectionPaths,
    type ProgressCallback,
} from '../types/modelCatalog';
import { createModelDownloadService } from './modelDownloadService';
import { createModelFileService } from './modelFileService';
import { createModelRegistryService } from './modelRegistryService';
import { listen } from './tauri/platform/events';
import { exists, mkdir, remove } from './tauri/platform/fs';
import { appLocalDataDir, join } from './tauri/platform/path';

export type { ModelFileConfig } from '../types/model';
export {
    DEFAULT_MODEL_RULES,
    PRESET_MODELS,
    PRESET_MODELS_MAP,
} from '../types/modelCatalog';
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

export interface ModelServicePorts {
    fileService: ReturnType<typeof createModelFileService>;
    registryService: ReturnType<typeof createModelRegistryService>;
    downloadService: ReturnType<typeof createModelDownloadService>;
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

    async resolveModelCatalogSelectedIds(paths: ModelSelectionPaths): Promise<ModelCatalogSelectedIds> {
        return await this.ports.registryService.resolveModelCatalogSelectedIds(paths);
    }

    resolveModelCatalogSelectedIdsFromSnapshot(
        snapshot: ModelCatalogSnapshot,
        paths: ModelSelectionPaths,
    ): ModelCatalogSelectedIds {
        return this.ports.registryService.resolveModelCatalogSelectedIdsFromSnapshot(snapshot, paths);
    }

    /**
     * Checks if the user's hardware is compatible with a specific model.
     *
     * @param modelId The ID of the model to check.
     * @return A promise resolving to an object with compatibility status and optional reason.
     */
    async checkHardware(modelId: string): Promise<{ compatible: boolean, reason?: string }> {
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
    async downloadModel(modelId: string, onProgress?: ProgressCallback, signal?: AbortSignal, mirror?: string): Promise<string> {
        const catalogModel = await this.ports.registryService.resolveCatalogModel(modelId);
        const model = catalogModel ?? PRESET_MODELS_MAP.get(modelId);
        if (!model) throw new Error('Model not found');

        const modelsDir = this.ports.registryService.latestSnapshot?.modelsDir ?? await this.getModelsDir();
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
        return await exists(modelPath);
    }

    /**
     * Deletes an installed model.
     *
     * @param modelId The ID of the model to delete.
     * @return A promise resolving when deletion is complete.
     */
    async deleteModel(modelId: string): Promise<void> {
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

export function createModelService(ports: ModelServicePorts): ModelService {
    return new ModelService(ports);
}

const fileService = createModelFileService({
    appLocalDataDir,
    join,
    exists,
    mkdir,
    remove,
});

const registryService = createModelRegistryService({
    getModelCatalogSnapshot: getModelCatalogSnapshotFromRust,
    resolveModelCatalogSelectedIds: resolveModelCatalogSelectedIdsFromRust,
    getModelsDir: () => fileService.getModelsDir(),
    join,
    presetModelsMap: PRESET_MODELS_MAP,
    defaultModelRules: DEFAULT_MODEL_RULES,
});

const downloadService = createModelDownloadService({
    downloadFile,
    extractTarBz2,
    cancelDownload: async (id: string) => {
        await cancelDownload(id);
    },
    remove: async (path: string) => {
        await remove(path);
    },
    listen,
    join,
    getModelsDir: () => fileService.getModelsDir(),
});

export const modelService = createModelService({
    fileService,
    registryService,
    downloadService,
});
