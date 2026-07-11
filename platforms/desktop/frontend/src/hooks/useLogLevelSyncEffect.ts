import { useEffect } from 'react';
import { useUIConfig } from '../stores/configStore';
import { setLogLevel } from '../services/tauri/app';
import { logger, setLoggerLevel } from '../utils/logger';
import { normalizeLogLevel } from '../utils/logLevel';

export function useLogLevelSyncEffect(isLoaded: boolean) {
  const { logLevel } = useUIConfig();

  useEffect(() => {
    const normalizedLevel = normalizeLogLevel(logLevel);
    setLoggerLevel(normalizedLevel);

    if (!isLoaded) return;

    setLogLevel(normalizedLevel)
      .catch((error) => logger.error('Failed to set log level:', error));
  }, [isLoaded, logLevel]);
}
