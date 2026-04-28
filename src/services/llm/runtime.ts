import type { AppConfig } from '../../types/config';
import type {
  LlmConfig,
  LlmFeature,
  LlmProvider,
  LlmProviderSetting,
} from '../../types/transcript';
import {
  buildLlmConfig,
  DEFAULT_LLM_TEMPERATURE,
  getProviderDefinition,
  normalizeProvider,
} from './providers';
import {
  ensureProviderSetting,
  getFeatureModelEntry,
} from './state';

export function getActiveProvider(config: Pick<AppConfig, 'llmSettings'>): LlmProvider {
  return normalizeProvider(config.llmSettings?.activeProvider);
}

export function getActiveProviderSetting(config: Pick<AppConfig, 'llmSettings'>): LlmProviderSetting {
  const provider = getActiveProvider(config);
  return ensureProviderSetting(config.llmSettings, provider);
}

export function getActiveLlmConfig(config: Pick<AppConfig, 'llmSettings'>): LlmConfig {
  const provider = getActiveProvider(config);
  return buildLlmConfig(provider, getActiveProviderSetting(config));
}

function getFeatureTemperature(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): number | undefined {
  const selectionTemperature =
    feature === 'polish'
      ? config.llmSettings?.selections.polishTemperature
      : feature === 'translation'
        ? config.llmSettings?.selections.translationTemperature
        : config.llmSettings?.selections.summaryTemperature;

  return selectionTemperature ?? DEFAULT_LLM_TEMPERATURE;
}

export function getFeatureLlmConfig(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): LlmConfig | null {
  const modelEntry = getFeatureModelEntry(config, feature);
  if (!modelEntry) {
    return null;
  }

  const setting = ensureProviderSetting(config.llmSettings, modelEntry.provider);
  // Runtime calls always derive from the selected feature model instead of the active
  // provider UI. That keeps one persisted provider registry while allowing each feature
  // to talk to a different backend with its own model and temperature.
  return {
    ...buildLlmConfig(modelEntry.provider, setting),
    model: modelEntry.model,
    temperature: getFeatureTemperature(config, feature),
  };
}

export function isLlmConfigComplete(llmConfig: LlmConfig | null): boolean {
  if (!llmConfig) {
    return false;
  }

  const definition = getProviderDefinition(llmConfig.provider);
  const hasApiHost = Boolean(llmConfig.baseUrl?.trim() || definition.defaultApiHost);
  const hasApiKey = !definition.requiresApiKey || Boolean(llmConfig.apiKey?.trim());
  const hasModel = Boolean(llmConfig.model?.trim());

  return hasApiHost && hasApiKey && hasModel;
}

export function isFeatureLlmConfigComplete(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): boolean {
  return isLlmConfigComplete(getFeatureLlmConfig(config, feature));
}

export function isSummaryLlmConfigComplete(
  config: Pick<AppConfig, 'llmSettings'>,
): boolean {
  return isFeatureLlmConfigComplete(config, 'summary');
}
