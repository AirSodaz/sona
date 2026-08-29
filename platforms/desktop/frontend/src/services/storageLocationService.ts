import type { StorageDirectoriesInfo } from '../types/storage';
import {
    storageGetDirectories,
    storageMigrateDataDirectory,
    storageResetDataDirectory,
    storageSetModelsDirectory,
    storageResetModelsDirectory,
    storageOpenPath,
} from './tauri/storage';
import { openDialog } from './tauri/platform/dialog';
import { relaunch } from './tauri/platform/process';
import { runGuardedQuit } from './quitGuard';

export interface StorageLocationServicePorts {
    storageGetDirectories: typeof storageGetDirectories;
    storageMigrateDataDirectory: typeof storageMigrateDataDirectory;
    storageResetDataDirectory: typeof storageResetDataDirectory;
    storageSetModelsDirectory: typeof storageSetModelsDirectory;
    storageResetModelsDirectory: typeof storageResetModelsDirectory;
    storageOpenPath: typeof storageOpenPath;
    openDialog: typeof openDialog;
    relaunch: typeof relaunch;
    runGuardedQuit: typeof runGuardedQuit;
}

export class StorageLocationService {
    constructor(private readonly ports: StorageLocationServicePorts) {}

    async getDirectories(): Promise<StorageDirectoriesInfo> {
        return this.ports.storageGetDirectories();
    }

    async selectDirectory(defaultPath?: string): Promise<string | null> {
        const selected = await this.ports.openDialog({
            directory: true,
            multiple: false,
            defaultPath: defaultPath || undefined,
        });
        if (!selected) {
            return null;
        }
        return Array.isArray(selected) ? selected[0] ?? null : selected;
    }

    async migrateDataDirectory(
        targetDir: string,
        copyExisting: boolean,
    ): Promise<StorageDirectoriesInfo> {
        return this.ports.storageMigrateDataDirectory(targetDir, copyExisting);
    }

    async resetDataDirectory(): Promise<StorageDirectoriesInfo> {
        return this.ports.storageResetDataDirectory();
    }

    async setModelsDirectory(
        targetDir: string,
        moveExisting: boolean,
    ): Promise<StorageDirectoriesInfo> {
        return this.ports.storageSetModelsDirectory(targetDir, moveExisting);
    }

    async resetModelsDirectory(): Promise<StorageDirectoriesInfo> {
        return this.ports.storageResetModelsDirectory();
    }

    async openPath(path: string): Promise<void> {
        await this.ports.storageOpenPath(path);
    }

    async relaunchApp(): Promise<void> {
        await this.ports.runGuardedQuit(async () => {
            await this.ports.relaunch();
        });
    }
}

export function createStorageLocationService(
    ports: StorageLocationServicePorts,
): StorageLocationService {
    return new StorageLocationService(ports);
}

export const storageLocationService = createStorageLocationService({
    storageGetDirectories,
    storageMigrateDataDirectory,
    storageResetDataDirectory,
    storageSetModelsDirectory,
    storageResetModelsDirectory,
    storageOpenPath,
    openDialog,
    relaunch,
    runGuardedQuit,
});
