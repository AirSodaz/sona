import type {
    StorageDirectoriesInfo,
    StorageUsageSnapshot,
    WebviewBrowsingDataClearResult,
} from '../../types/storage';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function storageGetUsageSnapshot(): Promise<StorageUsageSnapshot> {
    return invokeTauri(TauriCommand.storage.getUsageSnapshot);
}

export async function storageClearWebviewBrowsingData(): Promise<WebviewBrowsingDataClearResult> {
    return invokeTauri(TauriCommand.storage.clearWebviewBrowsingData);
}

export async function storageGetDirectories(): Promise<StorageDirectoriesInfo> {
    return invokeTauri(TauriCommand.storage.getDirectories);
}

export async function storageMigrateDataDirectory(
    targetDir: string,
    copyExisting: boolean,
): Promise<StorageDirectoriesInfo> {
    return invokeTauri(TauriCommand.storage.migrateDataDirectory, {
        targetDir,
        copyExisting,
    });
}

export async function storageResetDataDirectory(): Promise<StorageDirectoriesInfo> {
    return invokeTauri(TauriCommand.storage.resetDataDirectory);
}

export async function storageSetModelsDirectory(
    targetDir: string,
    moveExisting: boolean,
): Promise<StorageDirectoriesInfo> {
    return invokeTauri(TauriCommand.storage.setModelsDirectory, {
        targetDir,
        moveExisting,
    });
}

export async function storageResetModelsDirectory(): Promise<StorageDirectoriesInfo> {
    return invokeTauri(TauriCommand.storage.resetModelsDirectory);
}

export async function storageOpenPath(path: string): Promise<void> {
    return invokeTauri(TauriCommand.storage.openPath, { path });
}
