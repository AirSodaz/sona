import { create } from 'zustand';
import type { AppConfig } from '../types/config';
import { DEFAULT_CONFIG, useConfigStore } from './configStore';

interface EffectiveConfigState {
  config: AppConfig;
  syncConfig: () => Promise<void>;
}

function getStoreConfig(): AppConfig {
  if (typeof useConfigStore?.getState === 'function') {
    return useConfigStore.getState().config ?? DEFAULT_CONFIG;
  }
  return DEFAULT_CONFIG;
}

export const useEffectiveConfigStore = create<EffectiveConfigState>((set) => ({
  config: getStoreConfig(),
  syncConfig: async () => {
    set({ config: getStoreConfig() });
  },
}));

if (typeof useConfigStore?.subscribe === 'function') {
  useConfigStore.subscribe((state) => {
    useEffectiveConfigStore.setState({ config: state?.config ?? DEFAULT_CONFIG });
  });
}

export function getEffectiveConfigSnapshot(): AppConfig {
  return getStoreConfig();
}
