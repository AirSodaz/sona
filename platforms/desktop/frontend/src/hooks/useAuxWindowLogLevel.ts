import { useEffect } from 'react';
import { STORE_KEY_CONFIG, settingsStore } from '../services/storageService';
import type { AppConfig } from '../types/config';
import { logger, setLoggerLevel } from '../utils/logger';
import { normalizeLogLevel } from '../utils/logLevel';

export function useAuxWindowLogLevel(): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    const applyConfig = (config: AppConfig | null | undefined) => {
      if (disposed) return;
      if (config?.logLevel) {
        const normalized = normalizeLogLevel(config.logLevel);
        setLoggerLevel(normalized);
      }
    };

    const setup = async () => {
      try {
        const storedConfig = await settingsStore.get<AppConfig>(STORE_KEY_CONFIG);
        if (disposed) return;
        applyConfig(storedConfig);
        const unlistenFn = await settingsStore.onKeyChange<AppConfig>(
          STORE_KEY_CONFIG,
          applyConfig
        );
        if (disposed) {
          unlistenFn();
          return;
        }
        unlisten = unlistenFn;
      } catch (error) {
        logger.warn('[useAuxWindowLogLevel] Failed to synchronize log level', { error });
      }
    };

    void setup();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
