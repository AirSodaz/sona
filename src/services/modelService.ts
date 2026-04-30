import i18n from '../i18n';
import { logger } from "../utils/logger";
import { join, appLocalDataDir } from '@tauri-apps/api/path';
import { mkdir, exists, remove } from '@tauri-apps/plugin-fs';
import { listen } from '@tauri-apps/api/event';
import presetModelsData from '../shared/preset-models.json';
import { extractErrorMessage } from '../utils/errorUtils';
import {
    cancelDownload,
    downloadFile,
    extractTarBz2,
} from './tauri/app';
import { TauriEvent } from './tauri/events';
import type { ModelFileConfig } from '../types/model';

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

interface DownloadProgressPayloadObject {
    0?: number;
    1?: number;
    2?: string;
    downloaded?: number;
    total?: number;
    id?: string;
}

function parseDownloadProgressPayload(payload: unknown): { downloaded: number; total: number; id: string } {
    if (Array.isArray(payload)) {
        const [downloaded, total, id] = payload;
        return {
            downloaded: typeof downloaded === 'number' ? downloaded : 0,
            total: typeof total === 'number' ? total : 0,
            id: typeof id === 'string' ? id : '',
        };
    }

    if (typeof payload === 'object' && payload !== null) {
        const value = payload as DownloadProgressPayloadObject;
        const downloaded = typeof value[0] === 'number'
            ? value[0]
            : typeof value.downloaded === 'number'
                ? value.downloaded
                : 0;
        const total = typeof value[1] === 'number'
            ? value[1]
            : typeof value.total === 'number'
                ? value.total
                : 0;
        const id = typeof value[2] === 'string'
            ? value[2]
            : typeof value.id === 'string'
                ? value.id
                : '';

        return { downloaded, total, id };
    }

    return { downloaded: 0, total: 0, id: '' };
}

/**
 * Callback function for reporting download or extraction progress.
 *
 * @param percentage The progress percentage (0-100).
 * @param status A short description of the current status.
 * @param isFinished Whether the entire process is complete.
 */
export type ProgressCallback = (percentage: number, status: string, isFinished?: boolean) => void;

/**
 * Service for managing AI models (downloading, verifying, path resolution).
 */
class ModelService {
    /**
     * Gets the local directory where models are stored.
     *
     * Creates the directory if it does not exist.
     *
     * @return A promise that resolves to the absolute path of the models directory.
     */
    async getModelsDir(): Promise<string> {
        const appDataDir = await appLocalDataDir();
        const modelsDir = await join(appDataDir, 'models');
        if (!(await exists(modelsDir))) {
            await mkdir(modelsDir, { recursive: true });
        }
        logger.info('[ModelService] Models directory:', modelsDir);
        return modelsDir;
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
     * Generic file downloader with mirror support, progress reporting, and cancellation.
     *
     * @param url The primary URL to download from.
     * @param outputPath The local path to save the file to.
     * @param onProgress Optional callback for progress updates.
     * @param signal Optional AbortSignal for cancellation.
     * @param label Optional label for the download (used in progress messages).
     */
    private async downloadFile(
        url: string,
        outputPath: string,
        onProgress?: ProgressCallback,
        signal?: AbortSignal,
        label: string = i18n.t('settings.model_download_status.download_label')
    ): Promise<void> {
        // Mirrors to try in order
        const mirrors = [
            '', // Direct
            'https://mirror.ghproxy.com/',
            'https://ghproxy.net/'
        ];

        let downloadSuccess = false;
        let lastError: unknown = null;

        // wrapper to manage listener
        let unlisten: (() => void) | undefined;
        let lastDownloaded = 0;
        let lastTime = Date.now();

        // Generate a unique ID for this download request
        const downloadId = Math.random().toString(36).substring(7);

        if (signal) {
            signal.addEventListener('abort', async () => {
                try {
                    await cancelDownload(downloadId);
                } catch (e) {
                    logger.error('Failed to cancel download:', e);
                }
            });
        }

        if (onProgress) {
            unlisten = await listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
                const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);

                // Filter by ID
                if (id && id !== downloadId) return;

                // Calculate speed
                const now = Date.now();
                const timeDiff = now - lastTime;

                if (timeDiff > 500 || total === downloaded) { // Update every 500ms or on completion
                    const bytesDiff = downloaded - lastDownloaded;
                    const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
                    const speedStr = speedBytesPerSec > 1024 * 1024
                        ? `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
                        : `${Math.round(speedBytesPerSec / 1024)} KB/s`;

                    lastDownloaded = downloaded;
                    lastTime = now;

                    if (total > 0) {
                        const percentage = Math.round((downloaded / total) * 100);
                        const downloadedMB = Math.round(downloaded / 1024 / 1024);
                        const totalMB = Math.round(total / 1024 / 1024);
                        onProgress(percentage, i18n.t('settings.model_download_status.downloading', {
                            label,
                            downloadedMB,
                            totalMB,
                            speed: speedStr,
                        }));
                    }
                }
            });
        }

        try {
            for (const mirror of mirrors) {
                if (signal?.aborted) throw new Error('Download cancelled');

                try {
                    const downloadUrl = mirror ? `${mirror}${url}` : url;

                    if (onProgress) {
                        onProgress(0, i18n.t(
                            mirror
                                ? 'settings.model_download_status.downloading_from_mirror'
                                : 'settings.model_download_status.downloading_only',
                            { label }
                        ));
                    }

                    logger.info(`Attempting download from: ${downloadUrl} with ID: ${downloadId}`);
                    await downloadFile({
                        url: downloadUrl,
                        outputPath: outputPath,
                        id: downloadId,
                    });

                    downloadSuccess = true;
                    break; // Success!
                } catch (error) {
                    if (signal?.aborted || extractErrorMessage(error).includes('cancelled')) {
                        throw Object.assign(new Error('Download cancelled'), { cause: error });
                    }
                    logger.warn(`Download failed via ${mirror || 'direct'}:`, error);
                    lastError = error;
                    // Continue to next mirror
                }
            }
        } finally {
            if (unlisten) unlisten();
        }

        if (!downloadSuccess) {
            const lastErrorMessage = lastError ? extractErrorMessage(lastError) : 'Unknown error';
            throw new Error(`Download failed after all attempts. Last error: ${lastErrorMessage}`);
        }
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
        const model = PRESET_MODELS_MAP.get(modelId);
        if (!model) throw new Error('Model not found');

        const modelsDir = await this.getModelsDir();
        const targetFilename = model.filename || `${modelId}.tar.bz2`;
        const tempFilePath = await join(modelsDir, targetFilename);

        await this.downloadFile(model.url, tempFilePath, onProgress, signal, 'Downloading');

        if (model.isArchive === false) {
            onProgress?.(100, i18n.t('settings.model_download_status.done'), true);
            return tempFilePath;
        }

        if (signal?.aborted) throw new Error('Download cancelled');

        // No manual saving needed, Rust did it directly

        onProgress?.(100, i18n.t('settings.model_download_status.extracting'), false);

        let extractUnlisten: (() => void) | undefined;
        if (onProgress) {
            extractUnlisten = await listen<string>(TauriEvent.app.extractProgress, (event) => {
                const filename = event.payload;
                // Truncate filename if too long
                const displayFilename = filename.length > 30 ? '...' + filename.slice(-27) : filename;
                onProgress(100, i18n.t('settings.model_download_status.extracting_file', {
                    filename: displayFilename,
                }), false);
            });
        }

        try {
            logger.info('Starting extraction...');
            // Try backend extraction
            await this.extractArchive(tempFilePath, modelsDir, onProgress, signal);
        } catch (error) {
            throw Object.assign(new Error(`Extraction failed: ${extractErrorMessage(error)}`), { cause: error });
        } finally {
            if (extractUnlisten) extractUnlisten();
        }

        // Clean up archive
        await remove(tempFilePath);

        onProgress?.(100, i18n.t('settings.model_download_status.done'), true);

        if (model.filename) {
            return await join(modelsDir, model.filename);
        }
        if (model.type === 'punctuation') {
            return await join(modelsDir, modelId);
        }
        if (model.type === 'vad') {
            return tempFilePath;
        }
        return await join(modelsDir, modelId);
    }

    /**
     * Resolves the local file system path for a given model ID.
     *
     * @param modelId The ID of the model.
     * @return A promise resolving to the model's path.
     */
    async getModelPath(modelId: string): Promise<string> {
        const model = PRESET_MODELS_MAP.get(modelId);
        const modelsDir = await this.getModelsDir();
        if (model && model.filename) {
            return await join(modelsDir, model.filename);
        }
        return await join(modelsDir, modelId);
    }

    /**
     * Checks if a model is currently installed.
     *
     * @param modelId The ID of the model.
     * @return A promise resolving to true if installed, false otherwise.
     */
    async isModelInstalled(modelId: string): Promise<boolean> {
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
        if (await exists(modelPath)) {
            await remove(modelPath, { recursive: true });
        }
    }

    /**
     * Extracts an archive using the Rust backend.
     *
     * @param archivePath The path to the archive file.
     * @param targetDir The directory to extract into.
     * @param onProgress Optional callback for extraction progress.
     * @param signal Optional AbortSignal.
     * @return A promise that resolves when extraction is complete.
     */
    private async extractArchive(archivePath: string, targetDir: string, _onProgress?: ProgressCallback, signal?: AbortSignal): Promise<void> {
        logger.info('[ModelService] Attempting extraction via Rust backend (extract_tar_bz2)...');

        if (signal) {
            signal.addEventListener('abort', () => {
                logger.warn('Extraction cancellation requested, but not supported via Rust backend yet.');
            });
        }

        try {
            await extractTarBz2({
                archivePath: archivePath,
                targetDir: targetDir,
            });
        } catch (error) {
            throw Object.assign(new Error(`Extraction failed: ${extractErrorMessage(error)}`), { cause: error });
        }
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
        const model = PRESET_MODELS_MAP.get(modelId);
        if (model && model.rules) {
            return model.rules;
        }
        return DEFAULT_MODEL_RULES;
    }
}


export const modelService = new ModelService();
