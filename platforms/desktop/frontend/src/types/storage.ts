export interface StorageUsageSnapshot {
    generatedAt: string;
    totalBytes: number;
    categories: {
        audio: {
            bytes: number;
            historyAudioBytes: number;
            speakerSampleBytes: number;
            fileCount: number;
        };
        database: {
            bytes: number;
            sqlite: SQLiteUsageSummary;
        };
        models: {
            bytes: number;
            fileCount: number;
        };
        temporary: {
            bytes: number;
            fileCount: number;
        };
        webviewCache: {
            bytes: number | null;
            clearSupported: boolean;
            path?: string;
        };
        other: {
            bytes: number;
            fileCount: number;
        };
    };
}

export interface SQLiteUsageSummary {
    mainDbBytes: number;
    mainWalBytes: number;
    mainShmBytes: number;
    analyticsDbBytes: number;
    analyticsWalBytes: number;
    analyticsShmBytes: number;
    dataBytes: number;
    indexBytes: number;
    freePageBytes: number;
    indexEntries: Array<{
        schema: 'main' | 'analytics' | string;
        name: string;
        bytes: number;
    }>;
    dbstatAvailable: true;
}

export interface WebviewBrowsingDataClearResult {
    beforeBytes: number | null;
    afterBytes: number | null;
    clearRequested: true;
}
