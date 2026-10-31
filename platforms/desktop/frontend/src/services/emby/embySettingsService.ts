import { useConfigStore } from '../../stores/configStore';
import type { AppConfig } from '../../types/config';
import { DEFAULT_EMBY_TOOL_OVERRIDES, type EmbyToolOverrides } from '../../types/emby';
import { logger } from '../../utils/logger';
import { getEmbyProcessingSettings, saveEmbyProcessingSettings } from './embyService';

export { DEFAULT_EMBY_TOOL_OVERRIDES };

export const EMBY_TOOL_OVERRIDES_STORAGE_KEY = 'sona:emby:tool_overrides';

export function normalizeEmbyToolOverrides(input: unknown): EmbyToolOverrides {
  if (!input || typeof input !== 'object' || Array.isArray(input)) {
    return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
  }
  const parsed = input as Record<string, unknown>;
  return {
    asrSelection:
      (parsed.asrSelection as EmbyToolOverrides['asrSelection']) ??
      DEFAULT_EMBY_TOOL_OVERRIDES.asrSelection,
    asrLanguage:
      typeof parsed.asrLanguage === 'string'
        ? parsed.asrLanguage
        : DEFAULT_EMBY_TOOL_OVERRIDES.asrLanguage,
    translationModelId:
      typeof parsed.translationModelId === 'string' || parsed.translationModelId === null
        ? parsed.translationModelId
        : DEFAULT_EMBY_TOOL_OVERRIDES.translationModelId,
    defaultSubtitleMode:
      parsed.defaultSubtitleMode === 'pure_translation' ||
      parsed.defaultSubtitleMode === 'bilingual' ||
      parsed.defaultSubtitleMode === 'separate_tracks'
        ? parsed.defaultSubtitleMode
        : DEFAULT_EMBY_TOOL_OVERRIDES.defaultSubtitleMode,
    overwriteExistingExternal:
      typeof parsed.overwriteExistingExternal === 'boolean'
        ? parsed.overwriteExistingExternal
        : DEFAULT_EMBY_TOOL_OVERRIDES.overwriteExistingExternal,
  };
}

export function loadEmbyToolOverrides(): EmbyToolOverrides {
  if (typeof localStorage !== 'undefined') {
    const raw = localStorage.getItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY);
    if (raw) {
      try {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          return normalizeEmbyToolOverrides(parsed);
        }
      } catch (error) {
        logger.warn('[EmbySettingsService] Failed to load overrides from localStorage:', error);
      }
      return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
    }
  }

  try {
    const configSettings = useConfigStore.getState().config?.embyProcessingSettings;
    if (configSettings) {
      return normalizeEmbyToolOverrides(configSettings);
    }
  } catch {
    // Config store not initialized or in isolated environment
  }

  return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
}

function readOverridesFromLocalStorage(): EmbyToolOverrides | null {
  if (typeof localStorage === 'undefined') {
    return null;
  }
  try {
    const raw = localStorage.getItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return null;
    }
    return normalizeEmbyToolOverrides(parsed);
  } catch (error) {
    logger.warn('[EmbySettingsService] Failed to load overrides from localStorage:', error);
    return null;
  }
}

export async function loadEmbyToolOverridesAsync(): Promise<EmbyToolOverrides> {
  try {
    const backendSettings = await getEmbyProcessingSettings();
    if (backendSettings) {
      const normalized = normalizeEmbyToolOverrides(backendSettings);
      try {
        useConfigStore.getState().setConfig({ embyProcessingSettings: normalized });
      } catch {}
      if (typeof localStorage !== 'undefined') {
        try {
          localStorage.setItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY, JSON.stringify(normalized));
        } catch {}
      }
      return normalized;
    }
  } catch (error) {
    logger.warn('[EmbySettingsService] Failed to load overrides from backend:', error);
  }

  const local = readOverridesFromLocalStorage();
  if (local) {
    try {
      useConfigStore.getState().setConfig({ embyProcessingSettings: local });
      void saveEmbyProcessingSettings(local).catch(() => {});
    } catch {}
    return local;
  }

  return loadEmbyToolOverrides();
}

export function saveEmbyToolOverrides(overrides: EmbyToolOverrides): void {
  const normalized = normalizeEmbyToolOverrides(overrides);

  try {
    useConfigStore.getState().setConfig({ embyProcessingSettings: normalized });
  } catch (error) {
    logger.warn('[EmbySettingsService] Failed to update configStore with overrides:', error);
  }

  void saveEmbyProcessingSettings(normalized).catch((error) => {
    logger.warn('[EmbySettingsService] Failed to save overrides via Tauri:', error);
  });

  if (typeof localStorage !== 'undefined') {
    try {
      localStorage.setItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY, JSON.stringify(normalized));
    } catch (error) {
      logger.error('[EmbySettingsService] Failed to save overrides to localStorage:', error);
    }
  }
}

export function buildEmbyEffectiveConfig(
  globalConfig: AppConfig,
  overrides: EmbyToolOverrides
): AppConfig {
  const snapshot: AppConfig = JSON.parse(JSON.stringify(globalConfig));

  if (overrides.asrSelection) {
    if (!snapshot.asr) {
      snapshot.asr = {
        selections: {
          live: overrides.asrSelection,
          caption: overrides.asrSelection,
          voiceTyping: overrides.asrSelection,
          batch: overrides.asrSelection,
        },
      };
    } else if (!snapshot.asr.selections) {
      snapshot.asr.selections = {
        live: overrides.asrSelection,
        caption: overrides.asrSelection,
        voiceTyping: overrides.asrSelection,
        batch: overrides.asrSelection,
      };
    } else {
      snapshot.asr.selections.batch = overrides.asrSelection;
    }
  }

  if (overrides.asrLanguage !== undefined && overrides.asrLanguage !== null) {
    snapshot.language = overrides.asrLanguage;
  }

  if (overrides.translationModelId !== undefined && overrides.translationModelId !== null) {
    if (!snapshot.llmSettings) {
      snapshot.llmSettings = {
        activeProvider: 'open_ai',
        providers: {},
        models: {},
        modelOrder: [],
        selections: {
          translationModelId: overrides.translationModelId,
        },
      };
    } else if (!snapshot.llmSettings.selections) {
      snapshot.llmSettings.selections = {
        translationModelId: overrides.translationModelId,
      };
    } else {
      snapshot.llmSettings.selections.translationModelId = overrides.translationModelId;
    }
  }

  return snapshot;
}
