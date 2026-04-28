import type {
  AppConfig,
  LegacyTextReplacementRule,
  TextReplacementRule,
} from '../types/config';
import { ensureLlmState, type LlmMigrationSource } from './llm/migration';
import i18n from '../i18n';
import { DEFAULT_CONFIG } from '../stores/configStore';
import {
  coercePolishPresetId,
  migrateLegacyPolishSelection,
  normalizePolishCustomPresets,
} from '../utils/polishPresets';
import {
  migrateLegacyPolishKeywords,
  normalizePolishKeywordSets,
} from '../utils/polishKeywords';
import {
  coerceSummaryTemplateId,
  normalizeSummaryCustomTemplates,
} from '../utils/summaryTemplates';
import { normalizeSpeakerProfiles } from '../types/speaker';

export interface MigrationResult {
  config: AppConfig;
  migrated: boolean;
}

const CURRENT_CONFIG_VERSION = DEFAULT_CONFIG.configVersion ?? 6;

type LegacyConfigFields = {
  modelPath?: string;
  recognitionModelPath?: string;
  summaryTemplate?: string;
};

type ConfigMigrationInput =
  LlmMigrationSource
  & LegacyConfigFields
  & {
    textReplacements?: LegacyTextReplacementRule[];
  };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function mapLegacyTextReplacementRules(
  rules: LegacyTextReplacementRule[],
): TextReplacementRule[] {
  return rules.map((rule) => ({
    id: rule.id,
    from: rule.from,
    to: rule.to,
  }));
}

function shouldUpgradeConfig(config: ConfigMigrationInput, isConfigMigrated: boolean): boolean {
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

  if (!Array.isArray(config?.summaryCustomTemplates)) {
    return true;
  }

  if (!Array.isArray(config?.speakerProfiles)) {
    return true;
  }

  if (typeof config?.speakerSegmentationModelPath !== 'string') {
    return true;
  }

  if (typeof config?.speakerEmbeddingModelPath !== 'string') {
    return true;
  }

  if (typeof config?.polishPresetId !== 'string' || !config.polishPresetId.trim()) {
    return true;
  }

  if (typeof config?.summaryTemplateId !== 'string' || !config.summaryTemplateId.trim()) {
    return true;
  }

  return false;
}

/**
 * Handles migrating legacy configuration inputs or upgrading older settings
 * payloads to the latest config shape without performing any persistence.
 */
export async function migrateConfig(
  savedConfig: AppConfig | null | undefined,
  legacyConfig?: unknown,
): Promise<MigrationResult> {
  let configToLoad: AppConfig | ConfigMigrationInput | null | undefined = savedConfig;
  let isConfigMigrated = false;

  if (!savedConfig && isRecord(legacyConfig)) {
    configToLoad = legacyConfig as ConfigMigrationInput;
    isConfigMigrated = true;
  }

  // If we still have no config, use defaults and return.
  if (!configToLoad) {
    return { config: { ...DEFAULT_CONFIG }, migrated: false };
  }

  // Determine if an upgrade is needed based on version or missing keys.
  const needsUpgrade = shouldUpgradeConfig(configToLoad, isConfigMigrated);

  if (!needsUpgrade) {
    const existingConfig = configToLoad as AppConfig;
    const normalizedLlmSettings = ensureLlmState(existingConfig).llmSettings;
    const normalizedPolishCustomPresets = normalizePolishCustomPresets(
      existingConfig.polishCustomPresets,
    );
    const normalizedPolishPresetId = coercePolishPresetId(
      existingConfig.polishPresetId,
      normalizedPolishCustomPresets,
    );
    const normalizedSummaryCustomTemplates = normalizeSummaryCustomTemplates(
      existingConfig.summaryCustomTemplates,
    );
    const normalizedSummaryTemplateId = coerceSummaryTemplateId(
      existingConfig.summaryTemplateId,
      normalizedSummaryCustomTemplates,
    );
    const normalizedPolishKeywordSets = migrateLegacyPolishKeywords(
      existingConfig.polishKeywords,
      existingConfig.polishKeywordSets,
    );
    const normalizedConfig: AppConfig = {
      ...DEFAULT_CONFIG,
      ...existingConfig,
      configVersion: CURRENT_CONFIG_VERSION,
      llmSettings: normalizedLlmSettings,
      summaryEnabled: existingConfig.summaryEnabled ?? true,
      summaryTemplateId: normalizedSummaryTemplateId,
      summaryCustomTemplates: normalizedSummaryCustomTemplates,
      polishKeywords: '',
      polishPresetId: normalizedPolishPresetId,
      polishCustomPresets: normalizedPolishCustomPresets,
      polishKeywordSets: normalizedPolishKeywordSets,
      speakerSegmentationModelPath: existingConfig.speakerSegmentationModelPath || '',
      speakerEmbeddingModelPath: existingConfig.speakerEmbeddingModelPath || '',
      speakerProfiles: normalizeSpeakerProfiles(existingConfig.speakerProfiles),
    };

    const llmChanged =
      JSON.stringify(existingConfig.llmSettings ?? null) !==
      JSON.stringify(normalizedLlmSettings);
    const summaryEnabledChanged = existingConfig.summaryEnabled !== normalizedConfig.summaryEnabled;
    const polishPresetsChanged =
      JSON.stringify(existingConfig.polishCustomPresets ?? []) !==
        JSON.stringify(normalizedPolishCustomPresets)
      || existingConfig.polishPresetId !== normalizedPolishPresetId
      || existingConfig.configVersion !== CURRENT_CONFIG_VERSION;
    const summaryTemplatesChanged =
      JSON.stringify(existingConfig.summaryCustomTemplates ?? []) !==
        JSON.stringify(normalizedSummaryCustomTemplates)
      || existingConfig.summaryTemplateId !== normalizedSummaryTemplateId;
    const polishKeywordSetsChanged =
      JSON.stringify(existingConfig.polishKeywordSets ?? []) !==
        JSON.stringify(normalizedPolishKeywordSets)
      || (existingConfig.polishKeywords || '') !== normalizedConfig.polishKeywords;
    const speakerProfilesChanged =
      JSON.stringify(existingConfig.speakerProfiles ?? []) !==
        JSON.stringify(normalizedConfig.speakerProfiles)
      || typeof existingConfig.speakerSegmentationModelPath !== 'string'
      || typeof existingConfig.speakerEmbeddingModelPath !== 'string';

    if (
      !llmChanged
      && !summaryEnabledChanged
      && !polishPresetsChanged
      && !summaryTemplatesChanged
      && !polishKeywordSetsChanged
      && !speakerProfilesChanged
    ) {
      return { config: normalizedConfig, migrated: false };
    }

    return { config: normalizedConfig, migrated: true };
  }

  // Perform data normalization and structural upgrades.
  const parsed = configToLoad as ConfigMigrationInput;
  const { llmSettings } = ensureLlmState(parsed);
  const normalizedPolishCustomPresets = normalizePolishCustomPresets(parsed.polishCustomPresets);
  const normalizedSummaryCustomTemplates = normalizeSummaryCustomTemplates(parsed.summaryCustomTemplates);
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
    summaryTemplateId: coerceSummaryTemplateId(
      parsed.summaryTemplateId ?? parsed.summaryTemplate,
      normalizedSummaryCustomTemplates,
    ),
    summaryCustomTemplates: normalizedSummaryCustomTemplates,
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
    speakerProfiles: normalizeSpeakerProfiles(parsed.speakerProfiles),
    hotwords: parsed.hotwords || [],
    liveRecordShortcut: parsed.liveRecordShortcut || 'Ctrl + Space',
    voiceTypingEnabled: parsed.voiceTypingEnabled ?? false,
    voiceTypingShortcut: parsed.voiceTypingShortcut || 'Alt+V',
    voiceTypingMode: parsed.voiceTypingMode || 'hold',
    speakerSegmentationModelPath: parsed.speakerSegmentationModelPath || '',
    speakerEmbeddingModelPath: parsed.speakerEmbeddingModelPath || '',
  };

  // Migration: textReplacements -> textReplacementSets
  if (parsed.textReplacements && parsed.textReplacements.length > 0 && upgradedConfig.textReplacementSets!.length === 0) {
    const defaultSet = {
      id: 'default-set',
      name: i18n.t('settings.default_rule_set_name', { defaultValue: 'Default Rules' }),
      enabled: true,
      ignoreCase: false,
      rules: mapLegacyTextReplacementRules(parsed.textReplacements),
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

  return { config: upgradedConfig, migrated: true };
}
