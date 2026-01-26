import { join, appLocalDataDir } from '@tauri-apps/api/path';
import { mkdir, exists, remove } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface ModelInfo {
    id: string;
    name: string;
    description: string;
    url: string;
    type: 'streaming' | 'non-streaming';
    language: string;
    size: string; // Display size
    isArchive?: boolean;
    filename?: string;
}

export const PRESET_MODELS: ModelInfo[] = [
    {
        id: 'sherpa-onnx-streaming-zipformer-en-2023-02-21',
        name: 'English - Zipformer (Streaming)',
        description: 'Fast and accurate English streaming model',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2',
        type: 'streaming',
        language: 'en',
        size: '~90 MB'
    },
    {
        id: 'sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20',
        name: 'Chinese/English - Zipformer (Streaming)',
        description: 'Bilingual streaming model for Chinese and English',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20.tar.bz2',
        type: 'streaming',
        language: 'zh-en',
        size: '~100 MB'
    },
    {
        id: 'sherpa-onnx-paraformer-zh-2023-09-14',
        name: 'Chinese - Paraformer (Non-Streaming)',
        description: 'Accurate offline model from FunASR',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-paraformer-zh-2024-03-09.tar.bz2',
        type: 'non-streaming',
        language: 'zh',
        size: '~220 MB'
    },
    {
        id: 'sherpa-onnx-streaming-paraformer-bilingual-zh-en',
        name: 'Chinese/English - Paraformer (Streaming)',
        description: 'Streaming Paraformer model for Chinese and English',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-paraformer-bilingual-zh-en.tar.bz2',
        type: 'streaming',
        language: 'zh-en',
        size: '~140 MB'
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17',
        name: 'Multilingual - SenseVoice',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2',
        type: 'non-streaming',
        language: 'zh,en,ja,ko,yue',
        size: '~900 MB'
    },
    {
        id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
        name: 'Multilingual - SenseVoice (Int8)',
        description: 'Supports Chinese, English, Japanese, Korean, Cantonese (Int8 quantized)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2',
        type: 'non-streaming',
        language: 'zh,en,ja,ko,yue',
        size: '~200 MB'
    },
    {
        id: 'sherpa-onnx-whisper-tiny-int8',
        name: 'Multilingual - Whisper Tiny (Int8)',
        description: 'OpenAI Whisper Tiny model (Int8 quantized)',
        url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2',
        type: 'non-streaming',
        language: 'multilingual',
        size: '~150 MB',
        filename: 'sherpa-onnx-whisper-tiny' // Use existing folder name from tarball
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


    async downloadModel(modelId: string, onProgress?: ProgressCallback): Promise<string> {
        const model = PRESET_MODELS.find(m => m.id === modelId);
        if (!model) throw new Error('Model not found');

        const modelsDir = await this.getModelsDir();
        const targetFilename = model.filename || `${modelId}.tar.bz2`;
        const tempFilePath = await join(modelsDir, targetFilename);

        onProgress?.(0, 'Downloading...');

        // Listen for progress events
        let unlisten: (() => void) | undefined;
        if (onProgress) {
            unlisten = await listen<[number, number]>('download-progress', (event) => {
                const [downloaded, total] = event.payload;
                const percentage = Math.round((downloaded / total) * 50); // First 50% is download
                onProgress(percentage, `Downloading... ${Math.round(downloaded / 1024 / 1024)}MB / ${Math.round(total / 1024 / 1024)}MB`);
            });
        }

        try {
            await invoke('download_file', {
                url: model.url,
                outputPath: tempFilePath
            });
        } catch (error) {
            throw new Error(`Download failed: ${error}`);
        } finally {
            if (unlisten) unlisten();
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
            await invoke('extract_tar_bz2', {
                archivePath: tempFilePath,
                targetDir: modelsDir
            });
        } catch (error) {
            throw new Error(`Extraction failed: ${error}`);
        } finally {
            if (extractUnlisten) extractUnlisten();
        }

        // Clean up archive
        await remove(tempFilePath);

        onProgress?.(100, 'Done');
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
}

export const modelService = new ModelService();
