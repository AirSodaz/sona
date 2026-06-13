import presetModelsData from '../shared/preset-models.json';
import {
    downloadFile,
    extractTarBz2,
    getModelCatalogSnapshot as getModelCatalogSnapshotFromRust,
    resolveModelCatalogSelectedIds as resolveModelCatalogSelectedIdsFromRust,
} from './tauri/app';
import type { ModelFileConfig } from '../types/model';
import { createModelDownloadService } from './modelDownloadService';
import { createModelFileService } from './modelFileService';
import { createModelRegistryService } from './modelRegistryService';
import { listen } from './tauri/platform/events';
import { exists, mkdir, remove } from './tauri/platform/fs';
import { appLocalDataDir, join } from './tauri/platform/path';

export type { ModelFileConfig } from '../types/model';


/**
 * Interface defining the structure and metadata for an AI model.
 */
export type TimestampSupportHint = 'token' | 'segment' | 'unknown';

export interface ModelRules {
    /** Whether the model requires Voice Activity Detection (VAD). */
    requiresVad: boolean;
    /** Whether the model requires a Punctuation model. */
    requiresPunctuation: boolean;
    /** Optional hint for UI/diagnostics about the model's timestamp granularity. */
    timestampSupportHint?: TimestampSupportHint;
}

export interface ModelInfo {
    /** Unique identifier for the model. */
    id: string;
    /** Human-readable name of the model. */
    name: string;
    /** Brief description of the model's capabilities. */
    description: string;
    /** URL to download the model archive or file. */
    url: string;
    /** Type of the model (e.g., zipformer, sensevoice). */
    type:
      | 'zipformer'
      | 'sensevoice'
      | 'paraformer'
      | 'punctuation'
      | 'vad'
      | 'itn'
      | 'whisper'
      | 'funasr-nano'
      | 'fire-red-asr'
      | 'dolphin'
      | 'qwen3-asr'
      | 'speaker-segmentation'
      | 'speaker-embedding';
    /** Modes supported by the model (e.g., streaming, offline). */
    modes?: ('streaming' | 'offline')[];
    /** Languages supported by the model (comma-separated). */
    language: string;
    /** Display size of the model (e.g., "~100 MB"). */
    size: string;
    /** Expected SHA-256 for non-archive single-file downloads. */
    sha256?: string;
    /** Whether the model should be highlighted as recommended. */
    isRecommended?: boolean;
    /** Whether the download is an archive that needs extraction. Defaults to true. */
    isArchive?: boolean;
    /** Specific filename to look for or save as. */
    filename?: string;
    /** Inference engine used by the model. */
    engine: 'sherpa-onnx';
    /** Explicit model rules for VAD and Punctuation models. */
    rules?: ModelRules;
    /** Explicit file names within the model folder. */
    fileConfig?: ModelFileConfig;
    /** Group ID to group different versions of the same model. */
    groupId?: string;
    /** Version label to display in the grouped card. */
    versionLabel?: string;
}

export type ModelCatalogSectionType =
    | 'asr'
    | 'punctuation'
    | 'vad'
    | 'speaker-segmentation'
    | 'speaker-embedding';

export interface ModelCatalogModel extends ModelInfo {
    /** Resolved app-local install path for this preset. */
    installPath: string;
    /** Resolved app-local download target path for this preset. */
    downloadPath: string;
    /** Whether the install path currently exists. */
    isInstalled: boolean;
    /** Resolved model rules, including defaults for presets that omit rules. */
    rules: ModelRules;
}

export interface ModelCatalogGroup {
    key: string;
    models: ModelCatalogModel[];
}

export interface ModelCatalogSection {
    type: ModelCatalogSectionType;
    groups: ModelCatalogGroup[];
}

export interface ModelSelectionOption {
    id: string;
    label: string;
    installPath: string;
    isInstalled: boolean;
}

export interface ModelCatalogSelectionOptions {
    streaming: ModelSelectionOption[];
    offline: ModelSelectionOption[];
    speakerSegmentation: ModelSelectionOption[];
    speakerEmbedding: ModelSelectionOption[];
}

export type ModelDependencyConfigKey = 'vadModelPath' | 'punctuationModelPath';

export interface ModelDependencyRequest {
    modelId: string;
    configKey: ModelDependencyConfigKey;
    installPath: string;
    isInstalled: boolean;
}

export interface ModelCatalogPathMatchToken {
    id: string;
    token: string;
}

export interface ModelCatalogRestoreDefaults {
    streamingModelPath?: string;
    offlineModelPath?: string;
    vadModelPath?: string;
    punctuationModelPath?: string;
    speakerSegmentationModelPath?: string;
    speakerEmbeddingModelPath?: string;
    enableITN: boolean;
    batchVadEnabled?: boolean;
    vadBufferSize: number;
    maxConcurrent: number;
}

export interface ModelCatalogSnapshot {
    modelsDir: string;
    models: ModelCatalogModel[];
    sections: ModelCatalogSection[];
    selectionOptions: ModelCatalogSelectionOptions;
    modelPathById: Record<string, string>;
    modelIdByNormalizedPath: Record<string, string>;
    pathMatchTokens: ModelCatalogPathMatchToken[];
    dependencyRequestsByModelId: Record<string, ModelDependencyRequest[]>;
    restoreDefaults: ModelCatalogRestoreDefaults;
}

export interface ModelSelectionPaths {
    streamingModelPath: string;
    offlineModelPath: string;
    speakerSegmentationModelPath: string;
    speakerEmbeddingModelPath: string;
}

export interface ModelCatalogSelectedIds {
    streaming: string | null;
    offline: string | null;
    speakerSegmentation: string | null;
    speakerEmbedding: string | null;
}

export const DEFAULT_MODEL_RULES: ModelRules = {
    requiresVad: true,
    requiresPunctuation: false
};

/** List of pre-defined models available for download. */
export const PRESET_MODELS: ModelInfo[] = presetModelsData as ModelInfo[];

/** Map of pre-defined models keyed by their ID for O(1) lookups. */
export const PRESET_MODELS_MAP: Map<string, ModelInfo> = new Map(
    PRESET_MODELS.map(model => [model.id, model])
);

/**
 * Callback function for reporting download or extraction progress.
 *
 * @param percentage The progress percentage (0-100).
 * @param status A short description of the current status.
 * @param isFinished Whether the entire process is complete.
 */
export type ProgressCallback = (percentage: number, status: string, isFinished?: boolean) => void;

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
     * @return A promise resolving to the local path of the downloaded model.
     * @throws {Error} If the model is not found or download fails.
     */
    async downloadModel(modelId: string, onProgress?: ProgressCallback, signal?: AbortSignal): Promise<string> {
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
        const { cancelDownload } = await import('./tauri/app');
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
