import { useEffect } from 'react';
import {
  flushConfigPersistence,
  persistConfigImmediately,
  scheduleConfigPersistence,
} from '../services/configPersistenceService';
import { useConfigStore } from '../stores/configStore';

export { flushConfigPersistence, persistConfigImmediately, scheduleConfigPersistence };

export function useConfigPersistence(isLoaded: boolean) {
  const config = useConfigStore((state) => state.config);

  useEffect(() => {
    if (!isLoaded) return;
    scheduleConfigPersistence(config);
  }, [config, isLoaded]);
}
