import { logger } from "../utils/logger";
import { join, appLocalDataDir } from '@tauri-apps/api/path';
import { mkdir, exists, remove } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';


/**
 * Interface defining the structure and metadata for an AI model.
 */
export interface ModelRules {
    /** Whether the model requires Voice Activity Detection (VAD). */
    requiresVad: boolean;
    /** Whether the model requires a Punctuation model. */
    requiresPunctuation: boolean;
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
    type: 'zipformer' | 'sensevoice' | 'paraformer' | 'punctuation' | 'vad' | 'itn';
    /** Modes supported by the model (e.g., streaming, offline). */
    modes?: ('streaming' | 'offline')[];
    /** Languages supported by the model (comma-separated). */
    language: string;
    /** Display size of the model (e.g., "~100 MB"). */
    size: string;
    /** Whether the download is an archive that needs extraction. Defaults to true. */
    isArchive?: boolean;
    /** Specific filename to look for or save as. */
    filename?: string;
    /** Inference engine used by the model. */
    engine: 'sherpa-onnx';
    /** Explicit model rules for VAD and Punctuation models. */
    rules?: ModelRules;
}

export const DEFAULT_MODEL_RULES: ModelRules = {
    requiresVad: true,
    requiresPunctuation: false
};

/** List of pre-defined models available for download. */
export const PRESET_MODELS: ModelInfo[] = [
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
        name: 'Multilingual - SenseVoice (Int8)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (with build in punctuation)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2',
        type: 'sensevoice',
        modes: ['streaming', 'offline'],
        language: 'zh,en,ja,ko,yue',
        size: '~155 MB',
        engine: 'sherpa-onnx',
        rules: {
            requiresVad: true,
            requiresPunctuation: false
        }
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17',
        name: 'Multilingual - SenseVoice',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (with build in punctuation)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2',
        type: 'sensevoice',
        modes: ['streaming', 'offline'],
        language: 'zh,en,ja,ko,yue',
        size: '~999 MB',
        engine: 'sherpa-onnx',
        rules: {
            requiresVad: true,
            requiresPunctuation: false
        }
    },
    {
        id: 'sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en',
        name: 'Trilingual - Paraformer',
        description: 'Streaming Paraformer model. Supports Chinese, Cantonese, English',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en.tar.bz2',
        type: 'paraformer',
        modes: ['streaming'],
        language: 'zh,yue,en',
        size: '~999 MB',
        engine: 'sherpa-onnx',
        rules: {
            requiresVad: false,
            requiresPunctuation: true
        }
    },
    {
        id: 'sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30',
        name: 'Streaming Zipformer - ZH XLarge (Int8)',
        description: 'Streaming Zipformer model for Chinese',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30.tar.bz2',
        type: 'zipformer',
        modes: ['streaming'],
        language: 'zh',
        size: '~570 MB',
        engine: 'sherpa-onnx',
        rules: {
            requiresVad: false,
            requiresPunctuation: true
        }
    },
    {
        id: 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8',
        name: 'Punctuation - CT Transformer (Int8)',
        description: 'Chinese/English Punctuation Model (Int8 quantized). Adds punctuation to raw text.',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/punctuation-models/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8.tar.bz2',
        type: 'punctuation',
        language: 'zh,en',
        size: '~62 MB',
        engine: 'sherpa-onnx'
    },
    {
        id: 'silero-vad',
        name: 'Silero - VAD',
        description: 'Silero Voice Activity Detection model.',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx',
        type: 'vad',
        language: 'zh,en',
        size: '629KB',
        isArchive: false,
        filename: 'silero_vad.onnx',
        engine: 'sherpa-onnx'
    },
    {
        id: 'itn-zh-number',
        name: 'Chinese Number ITN',
        description: 'Inverse Text Normalization for Chinese Numbers',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/itn_zh_number.fst',
        type: 'itn',
        language: 'zh',
        filename: 'itn_zh_number.fst',
        size: '< 1 MB',
        engine: 'sherpa-onnx',
        isArchive: false
    }
];

/**
 * Callback function for reporting download or extraction progress.
 *
 * @param percentage The progress percentage (0-100).
 * @param status A short description of the current status.
 */
export type ProgressCallback = (percentage: number, status: string) => void;

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
        const model = PRESET_MODELS.find(m => m.id === modelId);
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
        label: string = 'Downloading'
    ): Promise<void> {
        // Mirrors to try in order
        const mirrors = [
            '', // Direct
            'https://mirror.ghproxy.com/',
            'https://ghproxy.net/'
        ];

        let downloadSuccess = false;
        let lastError: any = null;

        // wrapper to manage listener
        let unlisten: (() => void) | undefined;
        let lastDownloaded = 0;
        let lastTime = Date.now();

        // Generate a unique ID for this download request
        const downloadId = Math.random().toString(36).substring(7);

        if (signal) {
            signal.addEventListener('abort', async () => {
                try {
                    await invoke('cancel_download', { id: downloadId });
                } catch (e) {
                    logger.error('Failed to cancel download:', e);
                }
            });
        }

        if (onProgress) {
            unlisten = await listen<any>('download-progress', (event) => {
                const payload = event.payload;
                let downloaded = 0;
                let total = 0;
                let id = '';

                if (Array.isArray(payload)) {
                    [downloaded, total, id] = payload;
                } else if (typeof payload === 'object' && payload !== null) {
                    downloaded = (payload as any)[0] || (payload as any).downloaded || 0;
                    total = (payload as any)[1] || (payload as any).total || 0;
                    id = (payload as any)[2] || (payload as any).id || '';
                }

                // Filter by ID
                if (id && id !== downloadId) return;

                // Calculate speed
                const now = Date.now();
                const timeDiff = now - lastTime;

                if (timeDiff > 500 || total === downloaded) { // Update every 500ms or on completion
                    const bytesDiff = downloaded - lastDownloaded;
                    const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
                    let speedStr = '';

                    if (speedBytesPerSec > 1024 * 1024) {
                        speedStr = `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
                    } else {
                        speedStr = `${Math.round(speedBytesPerSec / 1024)} KB/s`;
                    }

                    lastDownloaded = downloaded;
                    lastTime = now;

                    if (total > 0) {
                        const percentage = Math.round((downloaded / total) * 100);
                        const downloadedMB = Math.round(downloaded / 1024 / 1024);
                        const totalMB = Math.round(total / 1024 / 1024);
                        onProgress(percentage, `${label}... ${downloadedMB}MB / ${totalMB}MB (${speedStr})`);
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
                        onProgress(0, mirror ? `${label} from mirror...` : `${label}...`);
                    }

                    logger.info(`Attempting download from: ${downloadUrl} with ID: ${downloadId}`);
                    await invoke('download_file', {
                        url: downloadUrl,
                        outputPath: outputPath,
                        id: downloadId
                    });

                    downloadSuccess = true;
                    break; // Success!
                } catch (error: any) {
                    if (signal?.aborted || error.toString().includes('cancelled')) {
                        throw new Error('Download cancelled');
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
            throw new Error(`Download failed after all attempts. Last error: ${lastError}`);
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
        const model = PRESET_MODELS.find(m => m.id === modelId);
        if (!model) throw new Error('Model not found');

        const modelsDir = await this.getModelsDir();
        const targetFilename = model.filename || `${modelId}.tar.bz2`;
        const tempFilePath = await join(modelsDir, targetFilename);

        await this.downloadFile(model.url, tempFilePath, onProgress, signal, 'Downloading');

        if (model.isArchive === false) {
            onProgress?.(100, 'Done');
            return tempFilePath;
        }

        if (signal?.aborted) throw new Error('Download cancelled');

        // No manual saving needed, Rust did it directly

        onProgress?.(100, 'Extracting (this may take a while)...');

        let extractUnlisten: (() => void) | undefined;
        if (onProgress) {
            extractUnlisten = await listen<string>('extract-progress', (event) => {
                const filename = event.payload;
                // Truncate filename if too long
                const displayFilename = filename.length > 30 ? '...' + filename.slice(-27) : filename;
                onProgress(100, `Extracting: ${displayFilename}`);
            });
        }

        try {
            logger.info('Starting extraction...');
            // Try backend extraction
            await this.extractArchive(tempFilePath, modelsDir, onProgress, signal);
        } catch (error) {
            throw new Error(`Extraction failed: ${error}`);
        } finally {
            if (extractUnlisten) extractUnlisten();
        }

        // Clean up archive
        await remove(tempFilePath);

        onProgress?.(100, 'Done');

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
        const model = PRESET_MODELS.find(m => m.id === modelId);
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
     * Efficiently resolves paths for all enabled ITN models in parallel, respecting preference order.
     *
     * @param enabledModels A set of enabled model IDs.
     * @param order The preferred order of model IDs.
     * @return A promise resolving to an array of valid file paths.
     */
    async getEnabledITNModelPaths(enabledModels: Set<string>, order: string[]): Promise<string[]> {
        const modelsDir = await this.getModelsDir();

        // 1. Models in the specified order
        const orderedModels = order.filter(id => enabledModels.has(id));

        // 2. Any other enabled models not in the order (fallback)
        const remainingModels = Array.from(enabledModels).filter(id => !order.includes(id));

        const allModelsToCheck = [...orderedModels, ...remainingModels];

        // Parallelize file system checks
        const results = await Promise.all(allModelsToCheck.map(async (id) => {
            const model = PRESET_MODELS.find(m => m.id === id);
            if (!model) return null;

            // Construct path manually to avoid re-calling getModelsDir
            const path = await join(modelsDir, model.filename || id);
            if (await exists(path)) {
                return path;
            }
            return null;
        }));

        return results.filter((p): p is string => p !== null);
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
            await invoke('extract_tar_bz2', {
                archivePath: archivePath,
                targetDir: targetDir
            });
        } catch (error) {
            throw new Error(`Extraction failed: ${error}`);
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
        const model = PRESET_MODELS.find(m => m.id === modelId);
        if (model && model.rules) {
            return model.rules;
        }
        return DEFAULT_MODEL_RULES;
    }
}


export const modelService = new ModelService();
