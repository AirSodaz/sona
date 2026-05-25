import { create } from 'zustand';
import type { AppConfig } from '../types/config';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { useConfigStore, DEFAULT_CONFIG } from './configStore';
import { useProjectStore } from './projectStore';

interface EffectiveConfigState {
  config: AppConfig;
  syncConfig: () => Promise<void>;
}

let syncRequestId = 0;

function isAppConfigLike(value: unknown): value is AppConfig {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Partial<AppConfig>;
  return typeof candidate.streamingModelPath === 'string'
    && typeof candidate.offlineModelPath === 'string';
}

function shouldUseGlobalSnapshot(snapshot: AppConfig | undefined, globalConfig: AppConfig): boolean {
  return !snapshot
    || (snapshot === DEFAULT_CONFIG && globalConfig !== DEFAULT_CONFIG)
    || snapshot.streamingModelPath !== globalConfig.streamingModelPath
    || snapshot.offlineModelPath !== globalConfig.offlineModelPath
    || snapshot.asr !== globalConfig.asr;
}

async function computeEffectiveConfig(): Promise<AppConfig> {
  const projectStore = useProjectStore.getState();
  const activeProject = typeof projectStore.getActiveProject === 'function'
    ? projectStore.getActiveProject()
    : null;

  return resolveEffectiveConfig(useConfigStore.getState().config, activeProject);
}

export const useEffectiveConfigStore = create<EffectiveConfigState>((set, get) => ({
  config: DEFAULT_CONFIG,
  syncConfig: async () => {
    const requestId = ++syncRequestId;
    const globalConfig = useConfigStore.getState().config ?? DEFAULT_CONFIG;
    if (shouldUseGlobalSnapshot(get().config, globalConfig)) {
      set({ config: globalConfig });
    }
    try {
      const resolved = await computeEffectiveConfig();
      if (requestId === syncRequestId) {
        set({ config: isAppConfigLike(resolved) ? resolved : globalConfig });
      }
    } catch {
      if (requestId === syncRequestId) {
        set({ config: globalConfig });
      }
    }
  },
}));

export function getEffectiveConfigSnapshot(): AppConfig {
  const globalConfig = useConfigStore.getState().config ?? DEFAULT_CONFIG;
  const snapshot = useEffectiveConfigStore.getState().config;
  if (shouldUseGlobalSnapshot(snapshot, globalConfig)) {
    return globalConfig;
  }
  return snapshot;
}
