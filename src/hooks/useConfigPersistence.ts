import { useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';
import { settingsStore, STORE_KEY_CONFIG } from '../services/storageService';
import { logger } from '../utils/logger';

export function useConfigPersistence(isLoaded: boolean) {
    const config = useConfigStore((state) => state.config);

    useEffect(() => {
        if (!isLoaded) return;

        const timeoutId = setTimeout(async () => {
            try {
                await settingsStore.set(STORE_KEY_CONFIG, config);
                await settingsStore.save();
            } catch (e) {
                logger.error('Failed to save config to store:', e);
            }
        }, 500);

        return () => clearTimeout(timeoutId);
    }, [config, isLoaded]);
}
