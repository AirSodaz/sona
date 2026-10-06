import { useEffect } from 'react';
import {
  flushConfigPersistence,
  scheduleConfigPersistence,
} from '../services/configPersistenceService';
import { useConfigStore } from '../stores/configStore';

export { flushConfigPersistence, scheduleConfigPersistence };

export function useConfigPersistence(isLoaded: boolean) {
  const config = useConfigStore((state) => state.config);

  useEffect(() => {
    if (!isLoaded) return;
    scheduleConfigPersistence(config);
  }, [config, isLoaded]);
}
