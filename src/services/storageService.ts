import { LazyStore } from '@tauri-apps/plugin-store';
import type { AppConfig } from '../types/config';
import {
  getAppSetting,
  loadAppConfig,
  saveAppConfig,
  setAppSetting,
} from './tauri/app';
import { emit, listen, type UnlistenFn } from './tauri/platform/events';

export const STORE_KEY_CONFIG = 'sona-config';
export const STORE_KEY_ONBOARDING = 'sona-onboarding';
export const STORE_KEY_ACTIVE_PROJECT = 'sona-active-project-id';
export const STORE_KEY_BACKUP_WEBDAV = 'sona-backup-webdav';

export const APP_SETTING_UPDATED_EVENT = 'app-setting-updated';

const legacySettingsStore = new LazyStore('settings.json');

interface SettingUpdatePayload<T = unknown> {
  key: string;
  value: T;
}

async function readLegacySetting<T>(key: string): Promise<T | null> {
  try {
    const value = await legacySettingsStore.get<T>(key);
    return value ?? null;
  } catch {
    return null;
  }
}

async function emitSettingUpdated(key: string, value: unknown): Promise<void> {
  await emit(APP_SETTING_UPDATED_EVENT, { key, value });
}

export const settingsStore = {
  async get<T>(key: string): Promise<T | null> {
    if (key === STORE_KEY_CONFIG) {
      const config = await loadAppConfig();
      if (config) {
        return config as T;
      }

      const legacyConfig = await readLegacySetting<T>(key);
      if (legacyConfig !== null) {
        await saveAppConfig(legacyConfig as unknown as AppConfig);
      }
      return legacyConfig;
    }

    const value = await getAppSetting<T>(key);
    if (value !== null && value !== undefined) {
      return value;
    }

    const legacyValue = await readLegacySetting<T>(key);
    if (legacyValue !== null && legacyValue !== undefined) {
      await setAppSetting(key, legacyValue);
    }
    return legacyValue;
  },

  async set(key: string, value: unknown): Promise<void> {
    if (key === STORE_KEY_CONFIG) {
      await saveAppConfig(value as AppConfig);
    } else {
      await setAppSetting(key, value);
    }

    await emitSettingUpdated(key, value);
  },

  async save(): Promise<void> {
    // SQLite writes are committed by set(); kept for LazyStore API compatibility.
  },

  async onKeyChange<T>(
    key: string,
    callback: (value: T | null | undefined) => void,
  ): Promise<UnlistenFn> {
    return listen(APP_SETTING_UPDATED_EVENT, (event) => {
      const payload = event.payload as Partial<SettingUpdatePayload<T>>;
      if (payload.key === key) {
        callback(payload.value);
      }
    });
  },
};
