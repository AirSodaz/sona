import { useEffect } from 'react';
import { setAutoStart } from '../services/tauri/app';
import { useUIConfig } from '../stores/configStore';
import { logger } from '../utils/logger';

export function useAutoStartSyncEffect(isLoaded: boolean) {
  const { autoStart } = useUIConfig();

  useEffect(() => {
    if (!isLoaded) return;
    setAutoStart(autoStart ?? false).catch((e) => logger.error('Failed to set auto start:', e));
  }, [autoStart, isLoaded]);
}
