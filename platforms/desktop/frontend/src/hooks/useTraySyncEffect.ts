import { useEffect } from 'react';
import { useUIConfig } from '../stores/configStore';
import { logger } from '../utils/logger';
import { setMinimizeToTray } from '../services/tauri/app';

export function useTraySyncEffect(isLoaded: boolean) {
    const { minimizeToTrayOnExit } = useUIConfig();

    useEffect(() => {
        if (!isLoaded) return;
        setMinimizeToTray(minimizeToTrayOnExit ?? true)
            .catch(e => logger.error('Failed to set minimize to tray:', e));
    }, [minimizeToTrayOnExit, isLoaded]);
}
