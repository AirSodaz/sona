function memoizeLoader<T>(loader: () => Promise<T>): () => Promise<T> {
    let promise: Promise<T> | null = null;

    return () => {
        if (!promise) {
            promise = loader();
        }

        return promise;
    };
}

export const loadBackupSettingsSection = memoizeLoader(async () => {
    const module = await import('./backup/BackupSettingsSection');
    return { default: module.BackupSettingsSection };
});

export function preloadSettingsGeneralDeferredSections(): Promise<void> {
    return loadBackupSettingsSection().then(() => undefined);
}
