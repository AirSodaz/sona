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
    if ('BackupSettingsSection' in module && module.BackupSettingsSection) {
        return { default: module.BackupSettingsSection };
    }
    if ('default' in module && module.default) {
        return { default: module.default };
    }
    return { default: module.BackupSettingsSection ?? module.default };
});

export function preloadSettingsGeneralDeferredSections(): Promise<void> {
    return loadBackupSettingsSection().then(() => undefined);
}
