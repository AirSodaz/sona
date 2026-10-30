import type { AppConfig } from '../../types/config';
import type { EmbyToolOverrides } from '../../types/emby';
import { logger } from '../../utils/logger';

export const EMBY_TOOL_OVERRIDES_STORAGE_KEY = 'sona:emby:tool_overrides';

export const DEFAULT_EMBY_TOOL_OVERRIDES: EmbyToolOverrides = {
  asrLanguage: 'auto',
  defaultSubtitleMode: 'bilingual',
  overwriteExistingExternal: true,
};

export function loadEmbyToolOverrides(): EmbyToolOverrides {
  if (typeof localStorage === 'undefined') {
    return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
  }
  try {
    const raw = localStorage.getItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY);
    if (!raw) {
      return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
    }
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
    }
    return {
      asrSelection: parsed.asrSelection ?? DEFAULT_EMBY_TOOL_OVERRIDES.asrSelection,
      asrLanguage: parsed.asrLanguage ?? DEFAULT_EMBY_TOOL_OVERRIDES.asrLanguage,
      translationModelId:
        parsed.translationModelId ?? DEFAULT_EMBY_TOOL_OVERRIDES.translationModelId,
      defaultSubtitleMode:
        parsed.defaultSubtitleMode ?? DEFAULT_EMBY_TOOL_OVERRIDES.defaultSubtitleMode,
      overwriteExistingExternal:
        parsed.overwriteExistingExternal ?? DEFAULT_EMBY_TOOL_OVERRIDES.overwriteExistingExternal,
    };
  } catch (error) {
    logger.warn('[EmbySettingsService] Failed to load overrides from localStorage:', error);
    return { ...DEFAULT_EMBY_TOOL_OVERRIDES };
  }
}

export function saveEmbyToolOverrides(overrides: EmbyToolOverrides): void {
  if (typeof localStorage === 'undefined') {
    return;
  }
  try {
    localStorage.setItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY, JSON.stringify(overrides));
  } catch (error) {
    logger.error('[EmbySettingsService] Failed to save overrides to localStorage:', error);
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
