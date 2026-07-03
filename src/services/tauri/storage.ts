import type {
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
