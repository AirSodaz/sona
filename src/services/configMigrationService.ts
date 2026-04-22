import { AppConfig } from '../types/config';
import { ensureLlmState } from './llmConfig';
import { settingsStore, STORE_KEY_CONFIG } from './storageService';
import i18n from '../i18n';
import { DEFAULT_CONFIG } from '../stores/configStore';
import { logger } from '../utils/logger';

export interface MigrationResult {
  config: AppConfig;
  migrated: boolean;
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
  const needsUpgrade = isConfigMigrated || !configToLoad.configVersion || configToLoad.configVersion < 1;

  if (!needsUpgrade) {
    const normalizedLlmSettings = ensureLlmState(configToLoad).llmSettings;
    const normalizedConfig: AppConfig = {
      ...(configToLoad as AppConfig),
      llmSettings: normalizedLlmSettings,
      summaryEnabled: (configToLoad as AppConfig).summaryEnabled ?? true,
    };

    const llmChanged =
      JSON.stringify((configToLoad as AppConfig).llmSettings ?? null) !==
      JSON.stringify(normalizedLlmSettings);
    const summaryEnabledChanged = (configToLoad as AppConfig).summaryEnabled !== normalizedConfig.summaryEnabled;

    if (!llmChanged && !summaryEnabledChanged) {
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
  const parsed = configToLoad;
  const { llmSettings } = ensureLlmState(parsed);

  const upgradedConfig: AppConfig = {
    configVersion: 1, // Bump to latest version
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
    polishKeywords: parsed.polishKeywords || '',
    polishContext: parsed.polishContext || '',
    polishScenario: parsed.polishScenario || '',
    autoPolish: parsed.autoPolish ?? false,
    autoPolishFrequency: parsed.autoPolishFrequency || 5,
    autoCheckUpdates: parsed.autoCheckUpdates ?? true,
    textReplacementSets: parsed.textReplacementSets || [],
    hotwordSets: parsed.hotwordSets || [],
    hotwords: parsed.hotwords || [],
    liveRecordShortcut: parsed.liveRecordShortcut || 'Ctrl + Space',
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
