import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Dropdown } from '../../Dropdown';
import { LlmFeature, LlmModelEntry, LlmProvider } from '../../../types/transcript';
import { LlmAssistantConfig } from '../../../types/config';
import {
  addLlmModel,
  enrichLlmModelMetadata,
  findLlmModelId,
  getProviderLlmModels,
  getFeatureModelEntry,
  isProviderModelDiscoveryExpired,
  modelSummaryToMetadata,
  setFeatureModelSelection,
  setFeatureTemperature,
  setFeatureReasoningEnabled,
  setFeatureReasoningLevel,
  syncProviderDiscoveredModels,
} from '../../../services/llm/state';
import {
  buildLlmConfig,
  DEFAULT_LLM_TEMPERATURE,
  getProviderDefinition,
  listProviderDefinitions,
} from '../../../services/llm/providers';
import { describeLlmModel, listLlmModels } from '../../../services/tauri/llm';
import { getCurrentLlmSettings, getModelPlaceholder, isProviderConfiguredForConfig } from './helpers';

interface FeatureCardProps {
  stepNumber: number;
  featureId: LlmFeature;
  title: string;
  icon: React.ReactNode;
  config: LlmAssistantConfig;
  applyLlmSettings: (s: LlmAssistantConfig['llmSettings']) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
  isActive?: boolean;
  showHeaderTitle?: boolean;
}

export const FeatureCard = React.memo(function FeatureCard({
  stepNumber,
  featureId,
  title,
  icon,
  config,
  applyLlmSettings,
  t,
  isActive = true,
  showHeaderTitle = true,
}: FeatureCardProps) {
  const currentLlmState = getCurrentLlmSettings(config);
  const latestLlmStateRef = useRef(currentLlmState);
  const isMountedRef = useRef(true);
  latestLlmStateRef.current = currentLlmState;
  const applyTrackedLlmSettings = useCallback((nextSettings: LlmAssistantConfig['llmSettings']) => {
    if (nextSettings) {
      latestLlmStateRef.current = nextSettings;
    }
    applyLlmSettings(nextSettings);
  }, [applyLlmSettings]);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);
  const modelEntry = getFeatureModelEntry(config, featureId);
  const configuredProvider = useMemo(() => {
    if (featureId === 'translation') {
      return 'google_translate_free' as LlmProvider;
    }

    const definitions = listProviderDefinitions(currentLlmState.customProviders);
    return definitions.find((definition) => (
      definition.id !== 'google_translate'
      && definition.id !== 'google_translate_free'
      && isProviderConfiguredForConfig(config, definition.id, currentLlmState.providers[definition.id])
    ))?.id ?? 'open_ai';
  }, [config, currentLlmState.customProviders, currentLlmState.providers, featureId]);
  const selectedProvider = modelEntry?.provider || configuredProvider;
  const selectedModel = modelEntry?.model || '';
  const temperature = featureId === 'polish'
    ? (currentLlmState.selections.polishTemperature ?? DEFAULT_LLM_TEMPERATURE)
    : featureId === 'translation'
      ? (currentLlmState.selections.translationTemperature ?? DEFAULT_LLM_TEMPERATURE)
      : (currentLlmState.selections.summaryTemperature ?? DEFAULT_LLM_TEMPERATURE);

  const reasoningEnabled = featureId === 'polish'
    ? !!currentLlmState.selections.polishReasoningEnabled
    : featureId === 'translation'
      ? !!currentLlmState.selections.translationReasoningEnabled
      : !!currentLlmState.selections.summaryReasoningEnabled;

  const reasoningLevel = featureId === 'polish'
    ? (currentLlmState.selections.polishReasoningLevel ?? 'medium')
    : featureId === 'translation'
      ? (currentLlmState.selections.translationReasoningLevel ?? 'medium')
      : (currentLlmState.selections.summaryReasoningLevel ?? 'medium');

  const supportsReasoning = useMemo(() => {
    return !!(
      modelEntry?.metadata?.supportsReasoning ||
      (modelEntry?.model && (
        modelEntry.model.toLowerCase().includes('o1-') ||
        modelEntry.model.toLowerCase() === 'o1' ||
        modelEntry.model.toLowerCase().includes('o3-') ||
        modelEntry.model.toLowerCase().includes('deepseek-reasoner') ||
        modelEntry.model.toLowerCase().includes('deepseek-r1') ||
        modelEntry.model.toLowerCase().includes('claude-3-7') ||
        modelEntry.model.toLowerCase().includes('gemini-2.5')
      ))
    );
  }, [modelEntry]);

  const supportsTemperatureForModel = (provider: LlmProvider, model: string, metadata?: LlmModelEntry['metadata']) => {
    const explicit = metadata?.supportsTemperature;
    if (typeof explicit === 'boolean') {
      return explicit;
    }
    if (provider === 'google_translate' || provider === 'google_translate_free') {
      return false;
    }
    const normalizedModel = model.toLowerCase();
    return !(
      /(^|[-:])o1(?:$|[-:])/.test(normalizedModel)
      || /(^|[-:])o3(?:$|[-:])/.test(normalizedModel)
      || normalizedModel.includes('deepseek-reasoner')
      || normalizedModel.includes('deepseek-r1')
    );
  };

  const handleReasoningEnabledChange = (enabled: boolean) => {
    applyTrackedLlmSettings(setFeatureReasoningEnabled(latestLlmStateRef.current, featureId, enabled));
  };

  const handleReasoningLevelChange = (level: string) => {
    applyTrackedLlmSettings(setFeatureReasoningLevel(
      latestLlmStateRef.current,
      featureId,
      level as 'low' | 'medium' | 'high',
    ));
  };

  const [localProvider, setLocalProvider] = useState<LlmProvider>(selectedProvider);
  const [localModelName, setLocalModelName] = useState<string>(selectedModel);
  const [modelCandidates, setModelCandidates] = useState<string[]>([]);
  const [isLoadingCandidates, setIsLoadingCandidates] = useState(false);
  const [isCandidateMenuOpen, setIsCandidateMenuOpen] = useState(false);
  const [highlightedCandidateIndex, setHighlightedCandidateIndex] = useState(-1);
  const candidateContainerRef = useRef<HTMLDivElement>(null);
  const localProviderDefinition = useMemo(
    () => getProviderDefinition(localProvider, currentLlmState.customProviders),
    [currentLlmState.customProviders, localProvider],
  );
  const supportsTemperature = supportsTemperatureForModel(
    localProvider,
    localProvider === selectedProvider ? localModelName || selectedModel : localModelName,
    localProvider === selectedProvider ? modelEntry?.metadata : undefined,
  );
  const providerApiHost = currentLlmState.providers[localProvider]?.apiHost;
  const providerApiKey = currentLlmState.providers[localProvider]?.apiKey;

  const providerOptions = useMemo(() => {
    const filtered = listProviderDefinitions(currentLlmState.customProviders).filter(p => {
      if (p.id === selectedProvider) return true;

      if (featureId !== 'translation' && (p.id === 'google_translate' || p.id === 'google_translate_free')) {
        return false;
      }

      const setting = currentLlmState.providers[p.id as LlmProvider];
      return isProviderConfiguredForConfig(config, p.id as LlmProvider, setting);
    });

    return filtered.map((p) => ({
      value: p.id,
      label: t(p.labelKey, { defaultValue: p.labelDefault }),
    }));
  }, [config, featureId, currentLlmState.customProviders, currentLlmState.providers, selectedProvider, t]);

  const persistedProviderModels = useMemo(
    () => getProviderLlmModels(currentLlmState, localProvider),
    [currentLlmState, localProvider],
  );

  const filteredCandidates = useMemo(() => {
    const query = localModelName.trim().toLowerCase();
    if (!query) return modelCandidates;
    return modelCandidates.filter((c) => c.toLowerCase().includes(query));
  }, [modelCandidates, localModelName]);

  const fetchModelCandidates = useCallback(async (provider: LlmProvider) => {
    const latestLlmState = latestLlmStateRef.current;
    const persistedModels = getProviderLlmModels(latestLlmState, provider);
    const isCacheExpired = isProviderModelDiscoveryExpired(latestLlmState, provider);
    if (persistedModels.length > 0 && !isCacheExpired) {
      setModelCandidates(persistedModels.map((entry) => entry.model));
      setIsLoadingCandidates(false);
      return;
    }

    const setting = latestLlmState.providers[provider];
    if (!getProviderDefinition(provider, latestLlmState.customProviders).supportsModelListing || !setting) {
      setModelCandidates([]);
      setIsLoadingCandidates(false);
      return;
    }
    setIsLoadingCandidates(true);
    try {
      const strategy = getProviderDefinition(provider, latestLlmState.customProviders).strategy;
      const fetchedAt = new Date().toISOString();
      const result = await listLlmModels({ provider, strategy, baseUrl: setting.apiHost, apiKey: setting.apiKey });
      const models = Array.isArray(result)
        ? result
          .map((entry) => (typeof entry === 'string' ? entry : entry.model))
          .filter((entry): entry is string => typeof entry === 'string' && entry.trim().length > 0)
        : [];
      setModelCandidates(models);
      applyTrackedLlmSettings(syncProviderDiscoveredModels(latestLlmStateRef.current, provider, result, fetchedAt));
    } catch {
      setModelCandidates(persistedModels.map((entry) => entry.model));
    } finally {
      setIsLoadingCandidates(false);
    }
  }, [applyTrackedLlmSettings]);

  useEffect(() => {
    if (!isActive) {
      return;
    }

    if (persistedProviderModels.length > 0 && !isProviderModelDiscoveryExpired(currentLlmState, localProvider)) {
      setModelCandidates(persistedProviderModels.map((entry) => entry.model));
      return;
    }

    queueMicrotask(() => {
      void fetchModelCandidates(localProvider);
    });
  }, [
    currentLlmState,
    fetchModelCandidates,
    isActive,
    localProvider,
    persistedProviderModels,
    providerApiHost,
    providerApiKey,
  ]);

  const commitModelChange = (providerToSave: LlmProvider, modelToSave: string) => {
    const trimmedModel = modelToSave.trim();
    if (!trimmedModel) {
      return;
    }

    const latestLlmState = latestLlmStateRef.current;
    const isManualAddition = !findLlmModelId(latestLlmState, providerToSave, trimmedModel);
    let nextState = addLlmModel(latestLlmState, { provider: providerToSave, model: trimmedModel });
    const entryId = nextState.modelOrder.find((id) => {
      const existing = nextState.models[id];
      return existing?.provider === providerToSave && existing.model === trimmedModel;
    });

    if (!entryId) {
      return;
    }
    nextState = setFeatureModelSelection(nextState, featureId, entryId);
    applyTrackedLlmSettings(nextState);

    const providerSetting = nextState.providers[providerToSave];
    if (
      !isManualAddition
      || !providerSetting
      || providerToSave === 'google_translate'
      || providerToSave === 'google_translate_free'
    ) {
      return;
    }

    void describeLlmModel({
      ...buildLlmConfig(providerToSave, providerSetting, nextState.customProviders),
      model: trimmedModel,
    }).then((summary) => {
      if (!isMountedRef.current || !summary || summary.model !== trimmedModel) {
        return;
      }
      const metadata = modelSummaryToMetadata(summary);
      if (Object.keys(metadata).length === 0) {
        return;
      }
      const latestState = latestLlmStateRef.current;
      const enrichedState = enrichLlmModelMetadata(latestState, entryId, metadata);
      if (enrichedState === latestState) {
        return;
      }
      applyTrackedLlmSettings(enrichedState);
    }).catch(() => {
      // Catalog enrichment is best-effort and must not block a manual model.
    });
  };

  const handleProviderChange = (newProvider: string) => {
    const p = newProvider as LlmProvider;
    setLocalProvider(p);
    if (featureId === 'translation' && (p === 'google_translate' || p === 'google_translate_free')) {
      setLocalModelName('default');
      commitModelChange(p, 'default');
    } else {
      setLocalModelName('');
    }
  };

  const handleModelSelect = (candidate: string) => {
    setLocalModelName(candidate);
    setIsCandidateMenuOpen(false);
    commitModelChange(localProvider, candidate);
  };

  const handleInputBlur = (e: React.FocusEvent) => {
    if (!candidateContainerRef.current?.contains(e.relatedTarget as Node)) {
      setIsCandidateMenuOpen(false);
      if (localModelName !== selectedModel) {
        commitModelChange(localProvider, localModelName);
      }
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      if (!isCandidateMenuOpen && filteredCandidates.length > 0) {
        event.preventDefault(); setIsCandidateMenuOpen(true); setHighlightedCandidateIndex(0); return;
      }
      if (filteredCandidates.length > 0) {
        event.preventDefault(); setHighlightedCandidateIndex((prev) => (prev + 1) % filteredCandidates.length);
      }
      return;
    }
    if (event.key === 'ArrowUp' && isCandidateMenuOpen && filteredCandidates.length > 0) {
      event.preventDefault(); setHighlightedCandidateIndex((prev) => (prev <= 0 ? filteredCandidates.length - 1 : prev - 1)); return;
    }
    if (event.key === 'Escape' || event.key === 'Tab') {
      setIsCandidateMenuOpen(false); setHighlightedCandidateIndex(-1); return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      if (isCandidateMenuOpen && highlightedCandidateIndex >= 0 && filteredCandidates[highlightedCandidateIndex]) {
        handleModelSelect(filteredCandidates[highlightedCandidateIndex]);
        return;
      }
      setIsCandidateMenuOpen(false);
      commitModelChange(localProvider, localModelName);
    }
  };

  const handleTempChange = (val: number) => {
    applyTrackedLlmSettings(setFeatureTemperature(latestLlmStateRef.current, featureId, val));
  };

  const temperatureLabelId = `feature-temperature-label-${featureId}`;

  return (
    <div
      className="feature-card"
      data-feature-id={featureId}
    >
      {showHeaderTitle && (
        <div className="feature-card-header">
          <div className="feature-card-title-group">
            <span className="feature-card-step">{String(stepNumber).padStart(2, '0')}</span>
            <span className="feature-card-icon">{icon}</span>
            <span className="feature-card-title-text">{title}</span>
          </div>
        </div>
      )}

      <div className="feature-card-content">
        <div className="feature-card-row feature-card-row-primary">
          <div className="feature-field">
            <label className="settings-label">{t('settings.llm.credential_provider')}</label>
            <Dropdown
              id={`provider-${featureId}`}
              value={localProvider}
              onChange={handleProviderChange}
              options={providerOptions}
              style={{ width: '100%' }}
            />
          </div>

          {localProviderDefinition.supportsModelListing ? (
            <div ref={candidateContainerRef} className="feature-field model-combobox-wrapper">
              <label className="settings-label" htmlFor={`feature-model-${featureId}`}>{t('settings.llm.model_library')}</label>
              <div className="dropdown-container" style={{ margin: 0 }}>
                <input
                  id={`feature-model-${featureId}`}
                  type="text"
                  className="settings-input"
                  value={localModelName}
                  onChange={(e) => setLocalModelName(e.target.value)}
                  onFocus={() => setIsCandidateMenuOpen(true)}
                  onBlur={handleInputBlur}
                  onKeyDown={handleKeyDown}
                  placeholder={getModelPlaceholder(localProvider)}
                />
                {isLoadingCandidates && (
                  <div className="settings-hint feature-card-loading-indicator">
                    <Loader2 size={16} className="animate-spin" style={{ color: 'var(--color-primary)' }} />
                  </div>
                )}
                {isCandidateMenuOpen && filteredCandidates.length > 0 && (
                  <div className="dropdown-menu" style={{ zIndex: 10, position: 'absolute', width: '100%' }}>
                    {filteredCandidates.slice(0, 8).map((candidate, index) => (
                      <button
                        key={candidate}
                        type="button"
                        className={`dropdown-item ${index === highlightedCandidateIndex ? 'selected' : ''}`}
                        onMouseDown={(e) => e.preventDefault()}
                        onMouseEnter={() => setHighlightedCandidateIndex(index)}
                        onClick={() => handleModelSelect(candidate)}
                      >
                        {candidate}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="feature-field model-combobox-wrapper">
              <label className="settings-label" htmlFor={`feature-model-${featureId}`}>{t('settings.llm.model_library')}</label>
              <input
                id={`feature-model-${featureId}`}
                type="text"
                className="settings-input feature-model-unsupported-input"
                value={t('settings.llm.model_selection_unsupported', { defaultValue: 'Model selection unsupported' })}
                disabled
                readOnly
              />
            </div>
          )}
        </div>

        {localProvider !== 'google_translate' && supportsReasoning && (
          <div className="feature-card-row feature-card-row-reasoning">
            <div className="feature-field-toggle">
              <label className="settings-toggle-label">
                <input
                  id={`feature-reasoning-toggle-${featureId}`}
                  type="checkbox"
                  checked={reasoningEnabled}
                  onChange={(e) => handleReasoningEnabledChange(e.target.checked)}
                />
                <span className="toggle-text">{t('settings.llm.reasoning_mode')}</span>
              </label>
            </div>

            {reasoningEnabled && (
              <div className="feature-field reasoning-level-wrapper">
                <label className="settings-label" htmlFor={`feature-reasoning-level-${featureId}`}>
                  {t('settings.llm.reasoning_level')}
                </label>
                <Dropdown
                  id={`feature-reasoning-level-${featureId}`}
                  value={reasoningLevel}
                  onChange={(val) => handleReasoningLevelChange(val)}
                  options={[
                    { value: 'low', label: t('settings.llm.reasoning_level_low') },
                    { value: 'medium', label: t('settings.llm.reasoning_level_medium') },
                    { value: 'high', label: t('settings.llm.reasoning_level_high') },
                  ]}
                  style={{ width: '100%' }}
                />
              </div>
            )}
          </div>
        )}

        {supportsTemperature && (
          <div className="feature-card-row feature-card-row-secondary">
            <div className="feature-field">
              <div className="feature-temperature-row">
                <span className="feature-temperature-label" id={temperatureLabelId}>
                  {t('settings.llm.temperature')}
                </span>
                <div className="feature-temperature-controls">
                  <input
                    id={`feature-temp-slider-${featureId}`}
                    type="range"
                    className="feature-temperature-slider"
                    min={0}
                    max={2}
                    step={0.05}
                    value={temperature}
                    onChange={(e) => handleTempChange(parseFloat(e.target.value))}
                    aria-label={`${title} ${t('settings.llm.temperature')}`}
                    style={{ '--temperature-progress': `${(temperature / 2) * 100}%` } as React.CSSProperties}
                  />
                  <input
                    id={`feature-temp-${featureId}`}
                    type="number"
                    className="settings-input feature-temperature-number"
                    min={0}
                    max={2}
                    step={0.05}
                    value={temperature}
                    onChange={(e) => {
                      const val = parseFloat(e.target.value);
                      if (!Number.isNaN(val) && val >= 0 && val <= 2) handleTempChange(val);
                    }}
                    aria-label={`${title} ${t('settings.llm.temperature')}`}
                  />
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});
