import { join, appLocalDataDir } from '@tauri-apps/api/path';
import { mkdir, exists, remove } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Command } from '@tauri-apps/plugin-shell';
import { resolveResource } from '@tauri-apps/api/path';


export interface ModelInfo {
    id: string;
    name: string;
    description: string;
    url: string;
    type: 'streaming' | 'offline';
    language: string;
    size: string; // Display size
    isArchive?: boolean;
    filename?: string;
    engine: 'onnx' | 'ncnn';
}

export const PRESET_MODELS: ModelInfo[] = [
    {
        id: 'sherpa-ncnn-streaming-zipformer-bilingual-zh-en-2023-02-13',
        name: 'Chinese/English - Zipformer (GPU)',
        description: 'Bilingual streaming model for GPU (NCNN)',
        url: 'https://github.com/k2-fsa/sherpa-ncnn/releases/download/models/sherpa-ncnn-streaming-zipformer-bilingual-zh-en-2023-02-13.tar.bz2',
        type: 'streaming',
        language: 'zh,en',
        size: '~150 MB',
        engine: 'ncnn'
    },
    {
        id: 'sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20',
        name: 'Chinese/English - Zipformer (CPU)',
        description: 'Bilingual streaming model for Chinese and English',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20.tar.bz2',
        type: 'streaming',
        language: 'zh,en',
        size: '~100 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-onnx-streaming-paraformer-bilingual-zh-en',
        name: 'Chinese/English - Paraformer (CPU)',
        description: 'Streaming Paraformer model for Chinese and English',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-paraformer-bilingual-zh-en.tar.bz2',
        type: 'streaming',
        language: 'zh,en',
        size: '~140 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17',
        name: 'Multilingual - SenseVoice (CPU)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2',
        type: 'offline',
        language: 'zh,en,ja,ko,yue',
        size: '~900 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
        name: 'Multilingual - SenseVoice (Int8)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (Int8 quantized)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2',
        type: 'offline',
        language: 'zh,en,ja,ko,yue',
        size: '~200 MB',
        engine: 'onnx'
    },
    {
        id: 'sherpa-ncnn-streaming-paraformer-bilingual-zh-en',
        name: 'Chinese/English - Paraformer (NCNN)',
        description: 'Streaming Paraformer model for GPU (NCNN)',
        url: 'https://github.com/k2-fsa/sherpa-ncnn/releases/download/models/sherpa-ncnn-streaming-paraformer-bilingual-zh-en.tar.bz2',
        type: 'streaming',
        language: 'zh,en',
        size: '~150 MB',
        engine: 'ncnn'
    },
    {
        id: 'sherpa-ncnn-sense-voice-zh-en-ja-ko-yue-2024-07-17',
        name: 'Multilingual - SenseVoice (NCNN)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (NCNN)',
        url: 'https://github.com/k2-fsa/sherpa-ncnn/releases/download/models/sherpa-ncnn-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2',
        type: 'offline',
        language: 'zh,en,ja,ko,yue',
        size: '~500 MB',
        engine: 'ncnn'
    }
];

export type ProgressCallback = (percentage: number, status: string) => void;

class ModelService {
    async getModelsDir(): Promise<string> {
        const appDataDir = await appLocalDataDir();
        const modelsDir = await join(appDataDir, 'models');
        if (!(await exists(modelsDir))) {
            await mkdir(modelsDir, { recursive: true });
        }
        console.log('[ModelService] Models directory:', modelsDir);
        return modelsDir;
    }

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

    async downloadModel(modelId: string, onProgress?: ProgressCallback): Promise<string> {
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

        if (onProgress) {
            unlisten = await listen<any>('download-progress', (event) => { // Changed type to any to inspect raw payload
                console.log('[ModelService] Download progress event:', event);
                const payload = event.payload;
                // Handle both [downloaded, total] tuple and potential object wrapper
                let downloaded = 0;
                let total = 0;

                if (Array.isArray(payload)) {
                    [downloaded, total] = payload;
                } else if (typeof payload === 'object' && payload !== null) {
                    // Check if it's an object with keys like { downloaded: number, total: number } or just handle it if structure is different
                    // For now assuming existing contract should be tuple, but logging will reveal truth.
                    // It is possible bindings return it differently.
                    // The original code expected [number, number]
                    // Let's stick to original logic but with logging first, but wait, 
                    // if I change type to any I can inspect safely.
                    downloaded = (payload as any)[0] || (payload as any).downloaded || 0;
                    total = (payload as any)[1] || (payload as any).total || 0;
                }

                console.log('[ModelService] Parsed progress:', { downloaded, total });

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
                try {
                    const url = mirror ? `${mirror}${model.url}` : model.url;

                    if (onProgress) {
                        onProgress(0, mirror ? `Downloading from mirror...` : 'Downloading...');
                    }

                    console.log(`Attempting download from: ${url}`);
                    await invoke('download_file', {
                        url: url,
                        outputPath: tempFilePath
                    });

                    downloadSuccess = true;
                    break; // Success!
                } catch (error) {
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
            try {
                // Try sidecar extraction first
                await this.extractWithSidecar(tempFilePath, modelsDir, onProgress);
            } catch (error) {
                console.warn('Sidecar extraction failed, falling back to built-in:', error);

                onProgress?.(60, '7zip failed, using fallback...');

                try {
                    // Fallback to built-in
                    await invoke('extract_tar_bz2', {
                        archivePath: tempFilePath,
                        targetDir: modelsDir
                    });
                } catch (fallbackError) {
                    throw new Error(`Extraction failed (both methods): ${fallbackError}`);
                }
            }
        } finally {
            if (extractUnlisten) extractUnlisten();
        }



        // Clean up archive
        await remove(tempFilePath);

        onProgress?.(100, 'Done');

        if (model.filename) {
            return await join(modelsDir, model.filename);
        }
        return await join(modelsDir, modelId); // Approximate path, real path depends on archive structure
    }

    async getModelPath(modelId: string): Promise<string> {
        const model = PRESET_MODELS.find(m => m.id === modelId);
        const modelsDir = await this.getModelsDir();
        if (model && model.filename) {
            return await join(modelsDir, model.filename);
        }
        return await join(modelsDir, modelId);
    }

    async isModelInstalled(modelId: string): Promise<boolean> {
        const modelPath = await this.getModelPath(modelId);
        return await exists(modelPath);
    }

    async deleteModel(modelId: string): Promise<void> {
        const modelPath = await this.getModelPath(modelId);
        if (await exists(modelPath)) {
            await remove(modelPath, { recursive: true });
        }
    }

    private async extractWithSidecar(archivePath: string, targetDir: string, onProgress?: ProgressCallback): Promise<void> {
        console.log('[ModelService] Attempting extraction via sidecar (7zip)...');

        const scriptPath = await resolveResource('sidecar/dist/index.mjs');
        const args = [
            scriptPath,
            '--mode', 'extract',
            '--file', archivePath,
            '--target-dir', targetDir
        ];

        const command = Command.sidecar('binaries/node', args);

        return new Promise(async (resolve, reject) => {
            let stderr = '';

            command.on('close', (data) => {
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

            await command.spawn();
        });
    }
}


export const modelService = new ModelService();
