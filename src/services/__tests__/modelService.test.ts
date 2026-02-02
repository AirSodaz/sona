import { describe, it, expect, vi, beforeEach } from 'vitest';
import { modelService, PRESET_MODELS, ITN_MODELS } from '../modelService';
import { invoke } from '@tauri-apps/api/core';
import { exists, mkdir, remove } from '@tauri-apps/plugin-fs';
import { join, appLocalDataDir } from '@tauri-apps/api/path';
import { listen } from '@tauri-apps/api/event';
import { Command } from '@tauri-apps/plugin-shell';

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
    join: vi.fn((...args) => Promise.resolve(args.join('/'))),
    resolveResource: vi.fn().mockResolvedValue('/mock/resource/path/sidecar.mjs'),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-shell', () => {
    const EventEmitter = require('events');
    class MockCommand extends EventEmitter {
        stdout = new EventEmitter();
        stderr = new EventEmitter();
        spawn = vi.fn().mockResolvedValue({
            pid: 123,
            kill: vi.fn()
        });
        static sidecar = vi.fn(() => new MockCommand());
    }
    return { Command: MockCommand };
});

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

        // Add a fake NCNN model to PRESET_MODELS for testing if none exist
        const ncnnModel = {
            id: 'test-ncnn',
            name: 'Test NCNN',
            description: 'Test',
            url: '',
            type: 'offline' as const,
            language: 'en',
            size: '10MB',
            engine: 'ncnn' as const
        };
        // We can't easily push to the exported constant array directly if it's read-only,
        // but modelService imports it.
        // Strategy: We can mock the array imports in the test if needed, but for now let's see if we have NCNN models.
        // The current presets are all ONNX. So checkHardware calls invoke only if engine is NCNN.
        // We might need to rely on what's there.

        // Since there are no NCNN models in the provided `modelService.ts` content,
        // `checkHardware` will always return true unless we mock `PRESET_MODELS`.
        // Let's try to mock the module's PRESET_MODELS using vi.mock if we really want to test that branch.
    });

    describe('isModelInstalled', () => {
        it('returns true if file exists', async () => {
            vi.mocked(join).mockResolvedValue('/app/data/models/test-model');
            vi.mocked(exists).mockResolvedValue(true);

            const result = await modelService.isModelInstalled('test-model');
            expect(result).toBe(true);
            expect(exists).toHaveBeenCalledWith('/app/data/models/test-model');
        });

        it('returns false if file does not exist', async () => {
            vi.mocked(exists).mockResolvedValue(false);
            const result = await modelService.isModelInstalled('test-model');
            expect(result).toBe(false);
        });
    });

    describe('downloadModel', () => {
        const modelId = PRESET_MODELS[0].id; // Use first available model

        it('downloads a model successfully', async () => {
            const onProgress = vi.fn();

            // Mock download_file invoke
            vi.mocked(invoke).mockImplementation((cmd) => {
                if (cmd === 'download_file') return Promise.resolve();
                return Promise.resolve();
            });

            // Mock extraction
             vi.mocked(Command.sidecar).mockImplementation(() => {
                 const cmd = new (vi.mocked(Command).prototype.constructor as any)();
                 setTimeout(() => {
                     cmd.emit('close', { code: 0 });
                 }, 10);
                 return cmd;
             });

            await modelService.downloadModel(modelId, onProgress);

            expect(invoke).toHaveBeenCalledWith('download_file', expect.objectContaining({
                url: expect.stringContaining('http'),
                outputPath: expect.stringContaining('/app/data/models/'),
            }));
            expect(onProgress).toHaveBeenCalledWith(100, 'Done');
        });

        it('retries with mirrors on failure', async () => {
             vi.mocked(invoke).mockImplementation((cmd, args: any) => {
                if (cmd === 'download_file') {
                    // Fail first attempt (direct)
                    if (args.url === PRESET_MODELS[0].url) {
                         return Promise.reject('Network Error');
                    }
                    // Succeed on second attempt (mirror)
                    return Promise.resolve();
                }
                return Promise.resolve();
            });

             // Mock extraction
             vi.mocked(Command.sidecar).mockImplementation(() => {
                const cmd = new (vi.mocked(Command).prototype.constructor as any)();
                setTimeout(() => {
                    cmd.emit('close', { code: 0 });
                }, 10);
                return cmd;
            });

            await modelService.downloadModel(modelId);

            expect(invoke).toHaveBeenCalledTimes(2); // Direct + 1 mirror
        });

        it('handles cancellation', async () => {
            const controller = new AbortController();

            // Mock download to hang or check signal
            vi.mocked(invoke).mockImplementation(async (cmd, args: any) => {
                 if (cmd === 'download_file') {
                     // Check if aborted before starting (simulated)
                     if (controller.signal.aborted) throw new Error('cancelled');

                     // Simulate wait then check
                     await new Promise(r => setTimeout(r, 20));
                     if (controller.signal.aborted) throw new Error('cancelled');
                 }
            });

            const promise = modelService.downloadModel(modelId, undefined, controller.signal);

            setTimeout(() => {
                controller.abort();
            }, 5);

            await expect(promise).rejects.toThrow('Download cancelled');
            expect(invoke).toHaveBeenCalledWith('cancel_download', expect.any(Object));
        });
    });

    describe('deleteModel', () => {
        it('removes the model directory/file if it exists', async () => {
            vi.mocked(exists).mockResolvedValue(true);
            vi.mocked(remove).mockResolvedValue();

            await modelService.deleteModel('test-model');

            expect(remove).toHaveBeenCalled();
        });

        it('does nothing if model does not exist', async () => {
            vi.mocked(exists).mockResolvedValue(false);

            await modelService.deleteModel('test-model');

            expect(remove).not.toHaveBeenCalled();
        });
    });

    describe('ITN Models', () => {
        const itnModelId = ITN_MODELS[0].id;

        it('checks if ITN model is installed', async () => {
             vi.mocked(exists).mockResolvedValue(true);
             const result = await modelService.isITNModelInstalled(itnModelId);
             expect(result).toBe(true);
        });

        it('downloads ITN model', async () => {
             vi.mocked(exists).mockResolvedValue(false);
             vi.mocked(invoke).mockResolvedValue(undefined);
             await modelService.downloadITNModel(itnModelId);

             expect(invoke).toHaveBeenCalledWith('download_file', expect.objectContaining({
                 url: expect.stringContaining(ITN_MODELS[0].url)
             }));
        });
    });
});
