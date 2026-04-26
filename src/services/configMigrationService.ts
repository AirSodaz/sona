import { AppConfig } from '../types/config';
import { ensureLlmState } from './llmConfig';
import { settingsStore, STORE_KEY_CONFIG } from './storageService';
import i18n from '../i18n';
import { DEFAULT_CONFIG } from '../stores/configStore';
import { logger } from '../utils/logger';
import {
  coercePolishPresetId,
  migrateLegacyPolishSelection,
  normalizePolishCustomPresets,
} from '../utils/polishPresets';
import {
  migrateLegacyPolishKeywords,
  normalizePolishKeywordSets,
} from '../utils/polishKeywords';

export interface MigrationResult {
  config: AppConfig;
  migrated: boolean;
}

const CURRENT_CONFIG_VERSION = DEFAULT_CONFIG.configVersion ?? 4;

function shouldUpgradeConfig(config: any, isConfigMigrated: boolean): boolean {
  if (isConfigMigrated) {
    return true;
  }

  const version = typeof config?.configVersion === 'number' ? config.configVersion : 0;
  if (version < CURRENT_CONFIG_VERSION) {
    return true;
  }

  if (config?.summaryEnabled === undefined) {
    return true;
  }

  if (!Array.isArray(config?.polishCustomPresets)) {
    return true;
  }

  if (!Array.isArray(config?.polishKeywordSets)) {
    return true;
  }

  if (typeof config?.polishPresetId !== 'string' || !config.polishPresetId.trim()) {
    return true;
  }

  return false;
}

/**
 * Handles migrating legacy configuration from localStorage or upgrading 
 * older versions of the Tauri settings.json store to the latest format.
 */
export async function migrateConfig(savedConfig: AppConfig | null | undefined): Promise<MigrationResult> {
  let configToLoad: any = savedConfig;
  let isConfigMigrated = false;

  // 1. Fallback to localStorage if no saved config in Tauri store
  if (!savedConfig) {
    const legacyConfig = localStorage.getItem('sona-config');
    if (legacyConfig) {
      try {
        configToLoad = JSON.parse(legacyConfig);
        isConfigMigrated = true;
      } catch (e) {
        logger.error('Failed to parse legacy config:', e);
      }
    }
  }

  // 2. If we still have no config, use defaults and return
  if (!configToLoad) {
    return { config: { ...DEFAULT_CONFIG }, migrated: false };
  }

  // 3. Determine if an upgrade is needed based on version or missing keys
  const needsUpgrade = shouldUpgradeConfig(configToLoad, isConfigMigrated);

  if (!needsUpgrade) {
    const normalizedLlmSettings = ensureLlmState(configToLoad).llmSettings;
    const normalizedPolishCustomPresets = normalizePolishCustomPresets(
      (configToLoad as AppConfig).polishCustomPresets,
    );
    const normalizedPolishPresetId = coercePolishPresetId(
      (configToLoad as AppConfig).polishPresetId,
      normalizedPolishCustomPresets,
    );
    const normalizedPolishKeywordSets = migrateLegacyPolishKeywords(
      (configToLoad as AppConfig).polishKeywords,
      (configToLoad as AppConfig).polishKeywordSets,
    );
    const normalizedConfig: AppConfig = {
      ...DEFAULT_CONFIG,
      ...(configToLoad as AppConfig),
      configVersion: CURRENT_CONFIG_VERSION,
      llmSettings: normalizedLlmSettings,
      summaryEnabled: (configToLoad as AppConfig).summaryEnabled ?? true,
      polishKeywords: '',
      polishPresetId: normalizedPolishPresetId,
      polishCustomPresets: normalizedPolishCustomPresets,
      polishKeywordSets: normalizedPolishKeywordSets,
    };

    const llmChanged =
      JSON.stringify((configToLoad as AppConfig).llmSettings ?? null) !==
      JSON.stringify(normalizedLlmSettings);
    const summaryEnabledChanged = (configToLoad as AppConfig).summaryEnabled !== normalizedConfig.summaryEnabled;
    const polishPresetsChanged =
      JSON.stringify((configToLoad as AppConfig).polishCustomPresets ?? []) !==
        JSON.stringify(normalizedPolishCustomPresets)
      || (configToLoad as AppConfig).polishPresetId !== normalizedPolishPresetId
      || (configToLoad as AppConfig).configVersion !== CURRENT_CONFIG_VERSION;
    const polishKeywordSetsChanged =
      JSON.stringify((configToLoad as AppConfig).polishKeywordSets ?? []) !==
        JSON.stringify(normalizedPolishKeywordSets)
      || ((configToLoad as AppConfig).polishKeywords || '') !== normalizedConfig.polishKeywords;

    if (!llmChanged && !summaryEnabledChanged && !polishPresetsChanged && !polishKeywordSetsChanged) {
      return { config: normalizedConfig, migrated: false };
    }

    try {
      await settingsStore.set(STORE_KEY_CONFIG, normalizedConfig);
      await settingsStore.save();
    } catch (e) {
      logger.error('Failed to save normalized config to Tauri store:', e);
    }

    return { config: normalizedConfig, migrated: true };
  }

  // 4. Perform Data Normalization & Structural Upgrades
  const parsed = configToLoad as any;
  const { llmSettings } = ensureLlmState(parsed);
  const normalizedPolishCustomPresets = normalizePolishCustomPresets(parsed.polishCustomPresets);
  const normalizedPolishKeywordSets = normalizePolishKeywordSets(parsed.polishKeywordSets);
  const migratedPolishSelection = migrateLegacyPolishSelection(
    {
      presetId: parsed.polishPresetId,
      scenario: parsed.polishScenario,
      context: parsed.polishContext,
    },
    normalizedPolishCustomPresets,
    'Imported Preset',
  );

  const upgradedConfig: AppConfig = {
    ...DEFAULT_CONFIG,
    ...(parsed as AppConfig),
    configVersion: CURRENT_CONFIG_VERSION,
    streamingModelPath: parsed.streamingModelPath || parsed.recognitionModelPath || parsed.offlineModelPath || parsed.modelPath || '',
    offlineModelPath: parsed.offlineModelPath || parsed.recognitionModelPath || parsed.modelPath || '',
    punctuationModelPath: parsed.punctuationModelPath || '',
    vadModelPath: parsed.vadModelPath || '',
    enableITN: parsed.enableITN ?? true,
    vadBufferSize: parsed.vadBufferSize || 5,
    maxConcurrent: parsed.maxConcurrent || 2,
    appLanguage: parsed.appLanguage || 'auto',
    theme: parsed.theme || 'auto',
    font: parsed.font || 'system',
    language: parsed.language || 'auto',
    enableTimeline: parsed.enableTimeline ?? false,
    minimizeToTrayOnExit: parsed.minimizeToTrayOnExit ?? true,
    lockWindow: parsed.lockWindow ?? false,
    alwaysOnTop: parsed.alwaysOnTop ?? true,
    microphoneId: parsed.microphoneId || 'default',
    microphoneBoost: parsed.microphoneBoost ?? 1.0,
    systemAudioDeviceId: parsed.systemAudioDeviceId || 'default',
    muteDuringRecording: parsed.muteDuringRecording ?? false,
    startOnLaunch: parsed.startOnLaunch ?? false,
    captionWindowWidth: parsed.captionWindowWidth || 800,
    captionFontSize: parsed.captionFontSize || 24,
    captionFontColor: parsed.captionFontColor || '#ffffff',
    captionBackgroundOpacity: parsed.captionBackgroundOpacity ?? 0.6,
    llmSettings,
    summaryEnabled: parsed.summaryEnabled ?? true,
    translationLanguage: parsed.translationLanguage || 'zh',
    polishKeywords: '',
    polishPresetId: migratedPolishSelection.presetId,
    polishCustomPresets: migratedPolishSelection.customPresets,
    polishKeywordSets: migrateLegacyPolishKeywords(parsed.polishKeywords, normalizedPolishKeywordSets),
    autoPolish: parsed.autoPolish ?? false,
    autoPolishFrequency: parsed.autoPolishFrequency || 5,
    autoCheckUpdates: parsed.autoCheckUpdates ?? true,
    textReplacementSets: parsed.textReplacementSets || [],
    hotwordSets: parsed.hotwordSets || [],
    hotwords: parsed.hotwords || [],
    liveRecordShortcut: parsed.liveRecordShortcut || 'Ctrl + Space',
    voiceTypingEnabled: parsed.voiceTypingEnabled ?? false,
    voiceTypingShortcut: parsed.voiceTypingShortcut || 'Alt+V',
    voiceTypingMode: parsed.voiceTypingMode || 'hold',
  };

  // Migration: textReplacements -> textReplacementSets
  if (parsed.textReplacements && parsed.textReplacements.length > 0 && upgradedConfig.textReplacementSets!.length === 0) {
    const defaultSet = {
      id: 'default-set',
      name: i18n.t('settings.default_rule_set_name', { defaultValue: 'Default Rules' }),
      enabled: true,
      ignoreCase: false,
      rules: parsed.textReplacements.map((r: any) => ({
        id: r.id,
        from: r.from,
        to: r.to
      }))
    };
    upgradedConfig.textReplacementSets = [defaultSet];
  }

  // Migration: hotwords -> hotwordSets
  if (parsed.hotwords && parsed.hotwords.length > 0 && upgradedConfig.hotwordSets!.length === 0) {
    const defaultHotwordSet = {
      id: 'default-hotword-set',
      name: i18n.t('settings.default_rule_set_name', { defaultValue: 'Default Rules' }),
      enabled: true,
      rules: parsed.hotwords.map((word: string, index: number) => ({
        id: `hw-${index}`,
        text: word
      }))
    };
    upgradedConfig.hotwordSets = [defaultHotwordSet];
    // Keep hotwords as is for backwards compatibility, but use hotwordSets going forward.
  }

  // 5. Persist the upgraded config to Tauri store
  try {
    await settingsStore.set(STORE_KEY_CONFIG, upgradedConfig);
    await settingsStore.save();
    
    // Clean up localStorage if we migrated from it
    if (isConfigMigrated) {
      localStorage.removeItem('sona-config');
    }
  } catch (e) {
    logger.error('Failed to save upgraded config to Tauri store:', e);
  }

  return { config: upgradedConfig, migrated: true };
}
