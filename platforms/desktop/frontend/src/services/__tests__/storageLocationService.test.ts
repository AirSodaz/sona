import { describe, expect, it, vi, beforeEach } from 'vitest';
import {
    createStorageLocationService,
    type StorageLocationServicePorts,
} from '../storageLocationService';
import type { StorageDirectoriesInfo } from '../../types/storage';

const mockDirectoriesInfo: StorageDirectoriesInfo = {
    dataDir: '/data',
    defaultDataDir: '/data',
    isCustomDataDir: false,
    modelsDir: '/data/models',
    defaultModelsDir: '/data/models',
    isCustomModelsDir: false,
};

describe('StorageLocationService', () => {
    let ports: StorageLocationServicePorts;

    beforeEach(() => {
        ports = {
            storageGetDirectories: vi.fn().mockResolvedValue(mockDirectoriesInfo),
            storageMigrateDataDirectory: vi.fn().mockResolvedValue({
                ...mockDirectoriesInfo,
                dataDir: '/new-data',
                isCustomDataDir: true,
            }),
            storageResetDataDirectory: vi.fn().mockResolvedValue(mockDirectoriesInfo),
            storageSetModelsDirectory: vi.fn().mockResolvedValue({
                ...mockDirectoriesInfo,
                modelsDir: '/new-models',
                isCustomModelsDir: true,
            }),
            storageResetModelsDirectory: vi.fn().mockResolvedValue(mockDirectoriesInfo),
            storageOpenPath: vi.fn().mockResolvedValue(undefined),
            openDialog: vi.fn().mockResolvedValue('/selected/path'),
            relaunch: vi.fn().mockResolvedValue(undefined),
            runGuardedQuit: vi.fn().mockImplementation(async (cb) => cb()),
        };
    });

    it('fetches directories info from tauri', async () => {
        const service = createStorageLocationService(ports);
        const info = await service.getDirectories();
        expect(info).toEqual(mockDirectoriesInfo);
        expect(ports.storageGetDirectories).toHaveBeenCalledTimes(1);
    });

    it('selects a directory via openDialog', async () => {
        const service = createStorageLocationService(ports);
        const selected = await service.selectDirectory('/initial');
        expect(selected).toBe('/selected/path');
        expect(ports.openDialog).toHaveBeenCalledWith({
            directory: true,
            multiple: false,
            defaultPath: '/initial',
        });
    });

    it('returns null if openDialog cancelled', async () => {
        ports.openDialog = vi.fn().mockResolvedValue(null);
        const service = createStorageLocationService(ports);
        const selected = await service.selectDirectory();
        expect(selected).toBeNull();
    });

    it('migrates data directory', async () => {
        const service = createStorageLocationService(ports);
        const info = await service.migrateDataDirectory('/new-data', true);
        expect(info.dataDir).toBe('/new-data');
        expect(ports.storageMigrateDataDirectory).toHaveBeenCalledWith('/new-data', true);
    });

    it('resets data directory', async () => {
        const service = createStorageLocationService(ports);
        const info = await service.resetDataDirectory();
        expect(info.isCustomDataDir).toBe(false);
        expect(ports.storageResetDataDirectory).toHaveBeenCalledTimes(1);
    });

    it('sets models directory', async () => {
        const service = createStorageLocationService(ports);
        const info = await service.setModelsDirectory('/new-models', true);
        expect(info.modelsDir).toBe('/new-models');
        expect(ports.storageSetModelsDirectory).toHaveBeenCalledWith('/new-models', true);
    });

    it('resets models directory', async () => {
        const service = createStorageLocationService(ports);
        const info = await service.resetModelsDirectory();
        expect(info.isCustomModelsDir).toBe(false);
        expect(ports.storageResetModelsDirectory).toHaveBeenCalledTimes(1);
    });

    it('opens path via tauri opener', async () => {
        const service = createStorageLocationService(ports);
        await service.openPath('/some/path');
        expect(ports.storageOpenPath).toHaveBeenCalledWith('/some/path');
    });

    it('relaunches app using runGuardedQuit', async () => {
        const service = createStorageLocationService(ports);
        await service.relaunchApp();
        expect(ports.runGuardedQuit).toHaveBeenCalledTimes(1);
        expect(ports.relaunch).toHaveBeenCalledTimes(1);
    });
});
