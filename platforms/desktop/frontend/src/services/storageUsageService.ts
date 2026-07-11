import type {
    StorageUsageSnapshot,
    WebviewBrowsingDataClearResult,
} from '../types/storage';
import {
    storageClearWebviewBrowsingData,
    storageGetUsageSnapshot,
} from './tauri/storage';

export interface StorageUsageServicePorts {
    storageGetUsageSnapshot: typeof storageGetUsageSnapshot;
    storageClearWebviewBrowsingData: typeof storageClearWebviewBrowsingData;
}

export class StorageUsageService {
    constructor(private readonly ports: StorageUsageServicePorts) {}

    async getUsageSnapshot(): Promise<StorageUsageSnapshot> {
        return this.ports.storageGetUsageSnapshot();
    }

    async clearWebviewBrowsingData(): Promise<WebviewBrowsingDataClearResult> {
        return this.ports.storageClearWebviewBrowsingData();
    }
}

export function createStorageUsageService(ports: StorageUsageServicePorts): StorageUsageService {
    return new StorageUsageService(ports);
}

export const storageUsageService = createStorageUsageService({
    storageGetUsageSnapshot,
    storageClearWebviewBrowsingData,
});
