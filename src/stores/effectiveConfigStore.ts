import { create } from 'zustand';
import type { AppConfig } from '../types/config';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { useConfigStore, DEFAULT_CONFIG } from './configStore';
import { useProjectStore } from './projectStore';

interface EffectiveConfigState {
  config: AppConfig;
  syncConfig: () => void;
}

function computeEffectiveConfig(): AppConfig {
  const projectStore = useProjectStore.getState();
  const activeProject = typeof projectStore.getActiveProject === 'function'
    ? projectStore.getActiveProject()
    : null;

  return resolveEffectiveConfig(useConfigStore.getState().config, activeProject);
}

export const useEffectiveConfigStore = create<EffectiveConfigState>((set) => ({
  config: DEFAULT_CONFIG,
  syncConfig: () => set({ config: computeEffectiveConfig() }),
}));

export function getEffectiveConfigSnapshot(): AppConfig {
  return useEffectiveConfigStore.getState().config;
}
