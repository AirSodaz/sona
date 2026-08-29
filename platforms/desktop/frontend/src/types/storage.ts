export type {
    SQLiteUsageSummary,
    StorageUsageSnapshot_Serialize as StorageUsageSnapshot,
    WebviewBrowsingDataClearResult,
} from '../bindings';

export interface StorageDirectoriesInfo {
    dataDir: string;
    defaultDataDir: string;
    isCustomDataDir: boolean;
    modelsDir: string;
    defaultModelsDir: string;
    isCustomModelsDir: boolean;
}
