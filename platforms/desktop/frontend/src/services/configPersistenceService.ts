import type { AppConfig } from '../types/config';
import { logger } from '../utils/logger';
import { STORE_KEY_CONFIG, settingsStore } from './storageService';
import { emit } from './tauri/platform/events';

let pendingTimer: number | NodeJS.Timeout | undefined;
let pendingConfig: AppConfig | null = null;
let pendingGeneration = 0;
let committedGeneration = 0;
let activeFlushPromise: Promise<void> | null = null;

export async function flushConfigPersistence(): Promise<void> {
  if (activeFlushPromise) {
    await activeFlushPromise;
  }
  if (pendingConfig === null || committedGeneration >= pendingGeneration) {
    return;
  }
  clearTimeout(pendingTimer);
  pendingTimer = undefined;

  const configToSave = pendingConfig;
  const targetGeneration = pendingGeneration;

  activeFlushPromise = (async () => {
    try {
      await settingsStore.set(STORE_KEY_CONFIG, configToSave);
      await emit('asr-config-updated');
      if (committedGeneration < targetGeneration) {
        committedGeneration = targetGeneration;
      }
      if (pendingGeneration === targetGeneration) {
        pendingConfig = null;
      }
    } catch (e) {
      logger.error('Failed to save config to store during flush:', e);
      throw e;
    } finally {
      activeFlushPromise = null;
    }
  })();

  await activeFlushPromise;
}

export function scheduleConfigPersistence(config: AppConfig): void {
  pendingConfig = config;
  pendingGeneration += 1;

  clearTimeout(pendingTimer);
  pendingTimer = setTimeout(() => {
    pendingTimer = undefined;
    void flushConfigPersistence().catch((e) => {
      logger.error('Failed to save config to store:', e);
    });
  }, 500);
}
