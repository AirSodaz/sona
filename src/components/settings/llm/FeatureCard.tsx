import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Check, Loader2, X } from 'lucide-react';
import { Dropdown } from '../../Dropdown';
import { LlmFeature, LlmProvider } from '../../../types/transcript';
import { LlmAssistantConfig } from '../../../types/config';
import {
  addLlmModel,
  getFeatureModelEntry,
  setFeatureModelSelection,
  setFeatureTemperature,
} from '../../../services/llm/state';
import { isFeatureLlmConfigComplete } from '../../../services/llm/runtime';
import {
  DEFAULT_LLM_TEMPERATURE,
  getProviderDefinition,
  LLM_PROVIDER_DEFINITIONS,
} from '../../../services/llm/providers';
import { listLlmModels } from '../../../services/tauri/llm';
import { getCurrentLlmSettings, getModelPlaceholder, isProviderConfigured } from './helpers';

interface FeatureCardProps {
  stepNumber: number;
  featureId: LlmFeature;
  title: string;
  icon: React.ReactNode;
  config: LlmAssistantConfig;
  applyLlmSettings: (s: LlmAssistantConfig['llmSettings']) => void;
  t: (key: string) => string;
  featureEnabled?: boolean;
  headerAction?: React.ReactNode;
}

export function FeatureCard({
  stepNumber,
  featureId,
  title,
  icon,
  config,
  applyLlmSettings,
  t,
  featureEnabled = true,
  headerAction,
}: FeatureCardProps) {
  const currentLlmState = getCurrentLlmSettings(config);
  const modelEntry = getFeatureModelEntry(config, featureId);
  const selectedProvider = modelEntry?.provider || 'open_ai';
  const selectedModel = modelEntry?.model || '';
  const temperature = featureId === 'polish'
    ? (currentLlmState.selections.polishTemperature ?? DEFAULT_LLM_TEMPERATURE)
    : featureId === 'translation'
      ? (currentLlmState.selections.translationTemperature ?? DEFAULT_LLM_TEMPERATURE)
      : (currentLlmState.selections.summaryTemperature ?? DEFAULT_LLM_TEMPERATURE);

  const [localProvider, setLocalProvider] = useState<LlmProvider>(selectedProvider);
  const [localModelName, setLocalModelName] = useState<string>(selectedModel);
  const [modelCandidates, setModelCandidates] = useState<string[]>([]);
  const [isLoadingCandidates, setIsLoadingCandidates] = useState(false);
  const [isCandidateMenuOpen, setIsCandidateMenuOpen] = useState(false);
  const [highlightedCandidateIndex, setHighlightedCandidateIndex] = useState(-1);
  const candidateContainerRef = useRef<HTMLDivElement>(null);
  const providerApiHost = currentLlmState.providers[localProvider]?.apiHost;
  const providerApiKey = currentLlmState.providers[localProvider]?.apiKey;

  const providerOptions = useMemo(() => {
    const filtered = LLM_PROVIDER_DEFINITIONS.filter(p => {
      if (p.id === selectedProvider) return true;

      if (featureId !== 'translation' && (p.id === 'google_translate' || p.id === 'google_translate_free')) {
        return false;
      }

      const setting = currentLlmState.providers[p.id as LlmProvider];
      return isProviderConfigured(p.id as LlmProvider, setting);
    });

    return filtered.map((p) => ({
      value: p.id,
      label: p.label,
    }));
  }, [featureId, currentLlmState.providers, selectedProvider]);

  const filteredCandidates = useMemo(() => {
    const query = localModelName.trim().toLowerCase();
    if (!query) return modelCandidates;
    return modelCandidates.filter((c) => c.toLowerCase().includes(query));
  }, [modelCandidates, localModelName]);

  const fetchModelCandidates = useCallback(async (provider: LlmProvider) => {
    const setting = currentLlmState.providers[provider];
    if (!getProviderDefinition(provider).supportsModelListing || !setting) {
      setModelCandidates([]);
      setIsLoadingCandidates(false);
      return;
    }
    setIsLoadingCandidates(true);
    try {
      const result = await listLlmModels({ provider, baseUrl: setting.apiHost, apiKey: setting.apiKey });
      setModelCandidates(Array.isArray(result) ? result : []);
    } catch {
      setModelCandidates([]);
    } finally {
      setIsLoadingCandidates(false);
    }
  }, [currentLlmState.providers]);

  useEffect(() => {
    queueMicrotask(() => {
      void fetchModelCandidates(localProvider);
    });
  }, [fetchModelCandidates, localProvider, providerApiHost, providerApiKey]);

  const commitModelChange = (providerToSave: LlmProvider, modelToSave: string) => {
    const trimmedModel = modelToSave.trim();
    if (!trimmedModel) {
      return;
    }

    let nextState = addLlmModel(currentLlmState, { provider: providerToSave, model: trimmedModel });
    const entryId = nextState.modelOrder.find((id) => {
      const existing = nextState.models[id];
      return existing?.provider === providerToSave && existing.model === trimmedModel;
    });

    if (entryId) {
      nextState = setFeatureModelSelection(nextState, featureId, entryId);
      applyLlmSettings(nextState);
    }
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
    applyLlmSettings(setFeatureTemperature(currentLlmState, featureId, val));
  };

  const isComplete = isFeatureLlmConfigComplete(config, featureId);
  const featureTitleId = `feature-title-${featureId}`;
  const temperatureLabelId = `feature-temperature-label-${featureId}`;
  const statusBadge = !featureEnabled ? (
    <span className="status-badge off"><X size={12}/> {t('settings.llm.status_off')}</span>
  ) : isComplete ? (
    <span className="status-badge ready"><Check size={12}/> {t('settings.llm.status_ready')}</span>
  ) : (
    <span className="status-badge missing">
      <X size={12}/> {selectedProvider && localModelName ? t('settings.llm.status_missing_api_key') : t('settings.llm.status_missing_model')}
    </span>
  );

  return (
    <div
      className={`feature-card ${featureEnabled ? '' : 'feature-card-off'}`.trim()}
      data-feature-id={featureId}
    >
      <div className="feature-card-header">
        <div className="feature-card-title-group">
          <span className="feature-card-step">{String(stepNumber).padStart(2, '0')}</span>
          <span className="feature-card-icon">{icon}</span>
          <span className="feature-card-title-text" id={featureTitleId}>{title}</span>
        </div>
        <div className="feature-card-header-meta">
          <div className="feature-card-status">{statusBadge}</div>
          {headerAction ? (
            <div className="feature-card-header-action">{headerAction}</div>
          ) : null}
        </div>
      </div>

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

          {localProvider !== 'google_translate' && (
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
          )}
        </div>

        {localProvider !== 'google_translate' && (
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
                    aria-labelledby={`${featureTitleId} ${temperatureLabelId}`}
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
                    aria-labelledby={`${featureTitleId} ${temperatureLabelId}`}
                  />
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
