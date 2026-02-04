import { join, appLocalDataDir } from '@tauri-apps/api/path';
import { mkdir, exists, remove } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Command } from '@tauri-apps/plugin-shell';
import { resolveResource } from '@tauri-apps/api/path';


/**
 * Interface defining the structure and metadata for an AI model.
 */
export interface ModelInfo {
    /** Unique identifier for the model. */
    id: string;
    /** Human-readable name of the model. */
    name: string;
    /** Brief description of the model's capabilities. */
    description: string;
    /** URL to download the model archive or file. */
    url: string;
    /** Type of the model (e.g., streaming, offline). */
    type: 'streaming' | 'offline' | 'punctuation' | 'vad';
    /** Languages supported by the model (comma-separated). */
    language: string;
    /** Display size of the model (e.g., "~100 MB"). */
    size: string;
    /** Whether the download is an archive that needs extraction. Defaults to true. */
    isArchive?: boolean;
    /** Specific filename to look for or save as. */
    filename?: string;
    /** Inference engine used by the model. */
    engine: 'onnx' | 'ncnn';
}

/** List of pre-defined models available for download. */
export const PRESET_MODELS: ModelInfo[] = [
    {
        id: 'sherpa-onnx-streaming-paraformer-bilingual-zh-en',
        name: 'Chinese/English - Paraformer',
        description: 'Streaming Paraformer model for Chinese and English',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-paraformer-bilingual-zh-en.tar.bz2',
        type: 'streaming',
        language: 'zh,en',
        size: '~999 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09',
        name: 'Multilingual - SenseVoice - 2025-09-09 (Int8)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (optimized for Cantonese)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2',
        type: 'offline',
        language: 'zh,en,ja,ko,yue',
        size: '~158 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
        name: 'Multilingual - SenseVoice - 2024-07-17 (Int8)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (with build in punctuation)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2',
        type: 'offline',
        language: 'zh,en,ja,ko,yue',
        size: '~155 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8',
        name: 'Punctuation - CT Transformer (Int8)',
        description: 'Chinese/English Punctuation Model (Int8 quantized). Adds punctuation to raw text.',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/punctuation-models/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8.tar.bz2',
        type: 'punctuation',
        language: 'zh,en',
        size: '~62 MB',
        engine: 'onnx'
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
        engine: 'onnx'
    }
];

/** List of pre-defined Inverse Text Normalization (ITN) models. */
export const ITN_MODELS = [
    {
        id: 'itn-zh-number',
        name: 'Chinese Number ITN',
        description: 'Inverse Text Normalization for Chinese Numbers',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/itn_zh_number.fst',
        filename: 'itn_zh_number.fst',
        size: '< 1 MB'
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
        console.log('[ModelService] Models directory:', modelsDir);
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

        if (model.engine === 'ncnn') {
            try {
                const hasGpu = await invoke<boolean>('check_gpu_availability');
                if (!hasGpu) {
                    return {
                        compatible: false,
                        reason: 'No compatible GPU detected (Apple Silicon or NVIDIA). This model requires a GPU.'
                    };
                }
            } catch (e) {
                console.error('Hardware check failed:', e);
                // Safe default: assume incompatible if check fails
                return { compatible: false, reason: 'Hardware check failed.' };
            }
        }
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
        const model = PRESET_MODELS.find(m => m.id === modelId);
        if (!model) throw new Error('Model not found');

        const modelsDir = await this.getModelsDir();
        const targetFilename = model.filename || `${modelId}.tar.bz2`;
        const tempFilePath = await join(modelsDir, targetFilename);

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
                    console.error('Failed to cancel download:', e);
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
                        const percentage = Math.round((downloaded / total) * 50); // First 50% is download
                        const downloadedMB = Math.round(downloaded / 1024 / 1024);
                        const totalMB = Math.round(total / 1024 / 1024);
                        onProgress(percentage, `Downloading... ${downloadedMB}MB / ${totalMB}MB (${speedStr})`);
                    }
                }
            });
        }

        try {
            for (const mirror of mirrors) {
                if (signal?.aborted) throw new Error('Download cancelled');

                try {
                    const url = mirror ? `${mirror}${model.url}` : model.url;

                    if (onProgress) {
                        onProgress(0, mirror ? `Downloading from mirror...` : 'Downloading...');
                    }

                    console.log(`Attempting download from: ${url} with ID: ${downloadId}`);
                    await invoke('download_file', {
                        url: url,
                        outputPath: tempFilePath,
                        id: downloadId
                    });

                    downloadSuccess = true;
                    break; // Success!
                } catch (error: any) {
                    if (signal?.aborted || error.toString().includes('cancelled')) {
                        throw new Error('Download cancelled');
                    }
                    console.warn(`Download failed via ${mirror || 'direct'}:`, error);
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

        if (model.isArchive === false) {
            onProgress?.(100, 'Done');
            return tempFilePath;
        }

        if (signal?.aborted) throw new Error('Download cancelled');

        onProgress?.(50, 'Saving to disk (instant)...');
        // No manual saving needed, Rust did it directly

        onProgress?.(60, 'Extracting (this may take a while)...');

        let extractUnlisten: (() => void) | undefined;
        if (onProgress) {
            extractUnlisten = await listen<string>('extract-progress', (event) => {
                const filename = event.payload;
                // Truncate filename if too long
                const displayFilename = filename.length > 30 ? '...' + filename.slice(-27) : filename;
                onProgress(60, `Extracting: ${displayFilename}`);
            });
        }

        try {
            console.log('Starting extraction...');
            // Try sidecar extraction
            await this.extractWithSidecar(tempFilePath, modelsDir, onProgress, signal);
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
            // Punctuation models extract to a folder, usually named after the archive
            // We need to point to the directory itself or a specific file?
            // sherpa-onnx expects the directory containing model.onnx (or similar) or the model file itself depending on usage.
            // For OfflinePunctuation, it looks for model.onnx within the passed config path typically, or we construct config object.
            // Let's just return the directory path for now, consistent with others.
            return await join(modelsDir, modelId);
        }
        if (model.type === 'vad') {
            // VAD models are single files
            return tempFilePath; // already joined with modelsDir and filename
        }
        return await join(modelsDir, modelId); // Approximate path, real path depends on archive structure
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
     * Gets the path for an Inverse Text Normalization (ITN) model.
     *
     * @param modelId The ID of the ITN model.
     * @return A promise resolving to the path, or empty string if not found.
     */
    async getITNModelPath(modelId: string): Promise<string> {
        const model = ITN_MODELS.find(m => m.id === modelId);
        if (!model) return '';
        const modelsDir = await this.getModelsDir();
        return await join(modelsDir, model.filename);
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
            const model = ITN_MODELS.find(m => m.id === id);
            if (!model) return null;

            // Construct path manually to avoid re-calling getModelsDir
            const path = await join(modelsDir, model.filename);
            if (await exists(path)) {
                return path;
            }
            return null;
        }));

        return results.filter((p): p is string => p !== null);
    }

    /**
     * Checks if an ITN model is installed.
     *
     * @param modelId The ID of the ITN model.
     * @return A promise resolving to true if installed.
     */
    async isITNModelInstalled(modelId: string): Promise<boolean> {
        const path = await this.getITNModelPath(modelId);
        return await exists(path);
    }

    /**
     * Downloads an ITN model.
     *
     * @param modelId The ID of the ITN model.
     * @param onProgress Optional callback for progress updates.
     * @param signal Optional AbortSignal for cancellation.
     * @return A promise resolving to the path of the downloaded model.
     * @throws {Error} If download fails.
     */
    async downloadITNModel(modelId: string, onProgress?: ProgressCallback, signal?: AbortSignal): Promise<string> {
        const model = ITN_MODELS.find(m => m.id === modelId);
        if (!model) throw new Error('ITN Model not found');

        const modelsDir = await this.getModelsDir();
        const targetPath = await join(modelsDir, model.filename);

        if (await exists(targetPath)) return targetPath;

        // Mirrors to try in order
        const mirrors = [
            '', // Direct
            'https://mirror.ghproxy.com/',
            'https://ghproxy.net/'
        ];

        const downloadId = Math.random().toString(36).substring(7);

        if (signal) {
            signal.addEventListener('abort', async () => {
                try {
                    await invoke('cancel_download', { id: downloadId });
                } catch (e) {
                    console.error('Failed to cancel download:', e);
                }
            });
        }

        let unlisten: (() => void) | undefined;
        let lastDownloaded = 0;
        let lastTime = Date.now();

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

                if (id && id !== downloadId) return;

                const now = Date.now();
                const timeDiff = now - lastTime;

                if (timeDiff > 500 || total === downloaded) {
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
                        onProgress(percentage, `Downloading ITN Model... (${speedStr})`);
                    }
                }
            });
        }

        try {
            let downloadSuccess = false;
            let lastError: any = null;

            for (const mirror of mirrors) {
                if (signal?.aborted) throw new Error('Download cancelled');

                try {
                    const url = mirror ? `${mirror}${model.url}` : model.url;

                    if (onProgress) {
                        onProgress(0, mirror ? `Downloading from mirror...` : 'Downloading...');
                    }

                    console.log(`Attempting download ITN from: ${url} with ID: ${downloadId}`);
                    await invoke('download_file', {
                        url: url,
                        outputPath: targetPath,
                        id: downloadId
                    });

                    downloadSuccess = true;
                    break;
                } catch (error: any) {
                    if (signal?.aborted || error.toString().includes('cancelled')) {
                        throw new Error('Download cancelled');
                    }
                    console.warn(`Download ITN failed via ${mirror || 'direct'}:`, error);
                    lastError = error;
                }
            }

            if (!downloadSuccess) {
                throw new Error(`Download ITN failed: ${lastError}`);
            }

            return targetPath;
        } finally {
            if (unlisten) unlisten();
        }
    }

    /**
     * Extracts an archive using a sidecar process.
     *
     * Uses a node script utilizing system tools or libraries to handle the extraction.
     *
     * @param archivePath The path to the archive file.
     * @param targetDir The directory to extract into.
     * @param onProgress Optional callback for extraction progress.
     * @param signal Optional AbortSignal.
     * @return A promise that resolves when extraction is complete.
     */
    private async extractWithSidecar(archivePath: string, targetDir: string, onProgress?: ProgressCallback, signal?: AbortSignal): Promise<void> {
        console.log('[ModelService] Attempting extraction via sidecar (7zip)...');

        const scriptPath = await resolveResource('sidecar/dist/index.mjs');
        const args = [
            scriptPath,
            '--mode', 'extract',
            '--file', archivePath,
            '--target-dir', targetDir
        ];

        const command = Command.sidecar('binaries/node', args);
        let child: any = null;

        return new Promise(async (resolve, reject) => {
            let stderr = '';

            command.on('close', (data) => {
                if (signal?.aborted) {
                    reject(new Error('Extraction cancelled'));
                    return;
                }
                if (data.code === 0) {
                    resolve();
                } else {
                    reject(new Error(`Sidecar exited with code ${data.code}: ${stderr}`));
                }
            });

            command.on('error', (err) => reject(err));

            command.stdout.on('data', (line) => {
                try {
                    const data = JSON.parse(line);
                    if (data.type === 'progress') {
                        // Map 0-100 of extraction to 60-95% of total progress
                        // percentage is 0-100
                        const overall = 60 + Math.round((data.percentage / 100) * 35);
                        onProgress?.(overall, `Extracting: ${data.status}`);
                    } else if (data.error) {
                        // console.error('Sidecar error:', data.error);
                    }
                } catch (e) {
                    // Ignore
                }
            });

            command.stderr.on('data', (line) => {
                stderr += line + '\n';
                console.log('[Extract Sidecar stderr]', line);
            });

            child = await command.spawn();

            if (signal) {
                signal.addEventListener('abort', async () => {
                    if (child) {
                        try {
                            // Use kill if available on Child, or invoke kill command?
                            // Tauri v2 Command.spawn() returns a Child object which has kill().
                            await child.kill();
                        } catch (e) {
                            console.error('Failed to kill extraction process:', e);
                        }
                    }
                    reject(new Error('Extraction cancelled'));
                });
            }
        });
    }
}


export const modelService = new ModelService();
