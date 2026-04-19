import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useUIConfig } from '../stores/configStore';
import { logger } from '../utils/logger';

export function useTraySyncEffect(isLoaded: boolean) {
    const { minimizeToTrayOnExit } = useUIConfig();

    useEffect(() => {
        if (!isLoaded) return;
        invoke('set_minimize_to_tray', { enabled: minimizeToTrayOnExit ?? true })
            .catch(e => logger.error('Failed to set minimize to tray:', e));
    }, [minimizeToTrayOnExit, isLoaded]);
}
