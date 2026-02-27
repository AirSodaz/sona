import { describe, it, expect, vi, beforeEach } from 'vitest';
import { modelService, PRESET_MODELS } from '../modelService';
import { invoke } from '@tauri-apps/api/core';
import { exists, remove } from '@tauri-apps/plugin-fs';

// Mock mocks
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(),
    mkdir: vi.fn(),
    remove: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
    appLocalDataDir: vi.fn().mockResolvedValue('/app/data'),
    join: vi.fn((...args) => Promise.resolve(args.join('/')))
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

describe('ModelService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('checkHardware', () => {
        it('returns true for non-NCNN models', async () => {
            const onnxModelId = PRESET_MODELS.find(m => m.engine === 'onnx')?.id;
            if (!onnxModelId) throw new Error('No ONNX model found in presets');

            const result = await modelService.checkHardware(onnxModelId);
            expect(result.compatible).toBe(true);
            expect(invoke).not.toHaveBeenCalledWith('check_gpu_availability');
        });
    });

    describe('isModelInstalled', () => {
        it('returns true if file exists', async () => {
            // vi.mocked(join).mockResolvedValue('/app/data/models/test-model');
            (exists as any).mockResolvedValue(true);

            const result = await modelService.isModelInstalled('test-model');
            expect(result).toBe(true);
        });

        it('returns false if file does not exist', async () => {
            (exists as any).mockResolvedValue(false);
            const result = await modelService.isModelInstalled('test-model');
            expect(result).toBe(false);
        });
    });

    describe('downloadModel', () => {
        const modelId = PRESET_MODELS[0].id; // Use first available model

        it('downloads a model successfully', async () => {
            const onProgress = vi.fn();

            // Mock download_file invoke
            (invoke as any).mockImplementation((cmd: string) => {
                if (cmd === 'download_file') return Promise.resolve();
                return Promise.resolve();
            });


            await modelService.downloadModel(modelId, onProgress);

            expect(invoke).toHaveBeenCalledWith('download_file', expect.objectContaining({
                url: expect.stringContaining('http'),
                outputPath: expect.stringContaining('/app/data/models/'),
            }));
            expect(onProgress).toHaveBeenCalledWith(100, 'Done');
        });
    });

    describe('deleteModel', () => {
        it('removes the model directory/file if it exists', async () => {
            (exists as any).mockResolvedValue(true);
            (remove as any).mockResolvedValue(undefined);

            await modelService.deleteModel('test-model');

            expect(remove).toHaveBeenCalled();
        });
    });

    describe('ITN Models', () => {
        const itnModel = PRESET_MODELS.find(m => m.type === 'itn');

        it('checks if ITN model is installed', async () => {
            if (!itnModel) return; // Skip if no ITN model
            (exists as any).mockResolvedValue(true);
            const result = await modelService.isModelInstalled(itnModel.id);
            expect(result).toBe(true);
        });

        it('downloads ITN model', async () => {
            if (!itnModel) return;
            (exists as any).mockResolvedValue(false);
            (invoke as any).mockResolvedValue(undefined);
            await modelService.downloadModel(itnModel.id);

            expect(invoke).toHaveBeenCalledWith('download_file', expect.objectContaining({
                url: expect.stringContaining(itnModel.url)
            }));
        });
    });
});
