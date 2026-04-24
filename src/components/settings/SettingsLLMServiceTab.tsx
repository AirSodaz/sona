import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Check, Loader2, X, ChevronDown, ChevronRight, Settings2, Sparkles, Globe, AlignLeft } from 'lucide-react';
import { RobotIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { LlmFeature, LlmProvider, LlmProviderSetting } from '../../types/transcript';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { LlmAssistantConfig } from '../../types/config';
import { normalizeError } from '../../utils/errorUtils';
import {
  addLlmModel,
  buildLlmConfigPatch,
  DEFAULT_LLM_TEMPERATURE,
  ensureLlmState,
  getFeatureModelEntry,
  getProviderDefinition,
  LLM_PROVIDER_DEFINITIONS,
  setFeatureModelSelection,
  setFeatureTemperature,
  updateProviderSetting,
  isFeatureLlmConfigComplete,
  buildLlmConfig,
  createProviderSetting,
} from '../../services/llmConfig';
import { SettingsTabContainer, SettingsPageHeader, SettingsSection } from './SettingsLayout';
import './SettingsLLMServiceTab.css';

function getModelPlaceholder(provider: LlmProvider): string {
  switch (provider) {
    case 'azure_openai': return 'gpt-4o-deployment';
    case 'anthropic': return 'claude-sonnet-4-20250514';
    case 'gemini': return 'gemini-2.5-flash';
    case 'ollama': return 'qwen3:8b';
    case 'deep_seek': return 'deepseek-chat';
    case 'kimi': return 'moonshot-v1-8k';
    case 'qwen':
    case 'qwen_portal': return 'qwen-max';
    case 'groq': return 'llama-3.3-70b-versatile';
    case 'x_ai': return 'grok-3-mini';
    case 'mistral_ai': return 'mistral-large-latest';
    case 'perplexity': return 'sonar';
    case 'google_translate':
    case 'google_translate_free': return 'default';
    default: return 'gpt-4o-mini';
  }
}

function isProviderConfigured(provider: LlmProvider, setting: LlmProviderSetting | undefined): boolean {
  const def = getProviderDefinition(provider);
  
  // 1. Check API Key if required
  if (def.requiresApiKey) {
    if (!setting || !(setting.apiKey || '').trim()) return false;
  }
  
  // 2. Check Base URL (API Host)
  // If there is no default host and no user-provided host, it's not ready for selection
  const effectiveHost = setting?.apiHost || def.defaultApiHost;
  if (!effectiveHost || !effectiveHost.trim()) return false;
  
  return true;
}

// ------ FEATURE CARD COMPONENT ------
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

function FeatureCard({
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
  const currentLlmState = config.llmSettings ? config.llmSettings : ensureLlmState(config as any).llmSettings;
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

  // Sync state if external changes happen
  useEffect(() => {
    setLocalProvider(selectedProvider);
    setLocalModelName(selectedModel);
  }, [selectedProvider, selectedModel]);

  // Candidates logic
  const [modelCandidates, setModelCandidates] = useState<string[]>([]);
  const [isLoadingCandidates, setIsLoadingCandidates] = useState(false);
  const [isCandidateMenuOpen, setIsCandidateMenuOpen] = useState(false);
  const [highlightedCandidateIndex, setHighlightedCandidateIndex] = useState(-1);
  const candidateContainerRef = useRef<HTMLDivElement>(null);
  
  const providerOptions = useMemo(() => {
    let filtered = LLM_PROVIDER_DEFINITIONS.filter(p => {
      // Always show the currently selected provider to avoid empty selection state
      if (p.id === selectedProvider) return true;
      
      // Feature-specific exclusions
      if (featureId !== 'translation' && (p.id === 'google_translate' || p.id === 'google_translate_free')) {
        return false;
      }

      // Filter by configuration status
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

  const fetchModelCandidates = async (provider: LlmProvider) => {
    const setting = currentLlmState.providers[provider];
    if (!getProviderDefinition(provider).supportsModelListing || !setting) {
      setModelCandidates([]);
      setIsLoadingCandidates(false);
      return;
    }
    setIsLoadingCandidates(true);
    try {
      const result = await invoke<string[]>('list_llm_models', {
        request: { provider, baseUrl: setting.apiHost, apiKey: setting.apiKey },
      });
      setModelCandidates(Array.isArray(result) ? result : []);
    } catch (_error) {
      setModelCandidates([]);
    } finally {
      setIsLoadingCandidates(false);
    }
  };

  useEffect(() => {
    fetchModelCandidates(localProvider);
  }, [localProvider, currentLlmState.providers[localProvider]?.apiHost, currentLlmState.providers[localProvider]?.apiKey]);

  const commitModelChange = (providerToSave: LlmProvider, modelToSave: string) => {
    const trimmedModel = modelToSave.trim();
    if (!trimmedModel) {
       // if they clear it, optionally unassign the feature model
       return; 
    }
    
    // Add explicitly to model library and assign
    let nextState = addLlmModel(currentLlmState, { provider: providerToSave, model: trimmedModel });
    
    // Find the newly added item id
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
      // don't commit it until they select a model. Just clear the UI model
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

// ------ ACCORDION ITEM COMPONENT ------
interface AccordionItemProps {
  provider: LlmProvider;
  config: LlmAssistantConfig;
  isOpen: boolean;
  onToggle: () => void;
  applyProviderUpdates: (updates: Partial<LlmProviderSetting>) => void;
  t: (key: string) => string;
}

function ProviderAccordionItem({ provider, config, isOpen, onToggle, applyProviderUpdates, t }: AccordionItemProps) {
  const currentLlmState = config.llmSettings ? config.llmSettings : ensureLlmState(config as any).llmSettings;
  const def = getProviderDefinition(provider);
  const setting = currentLlmState.providers[provider];
  
  const isConfigured = isProviderConfigured(provider, setting);
  
  const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [testMessage, setTestMessage] = useState('');

  const handleTestConnection = async () => {
    const effectiveSetting = setting || createProviderSetting(provider);
    setTestStatus('loading');
    setTestMessage('');
    try {
      const providerConfig = buildLlmConfig(provider, effectiveSetting);
      // Get a model to test
      const entryId = currentLlmState.modelOrder.find(id => currentLlmState.models[id].provider === provider);
      const testModel = entryId ? currentLlmState.models[entryId].model : getModelPlaceholder(provider);
      
      const testProviderConfig = { ...providerConfig, model: testModel };

      await invoke<string>('generate_llm_text', {
        request: { config: testProviderConfig, input: 'Hello, this is a connection test.' },
      });
      setTestStatus('success');
      setTestMessage(testModel);
      // Reset back to idle after 3 seconds
      setTimeout(() => {
        setTestStatus('idle');
        setTestMessage('');
      }, 3000);
    } catch (error) {
      setTestStatus('error');
      setTestMessage(normalizeError(error).message);
    }
  };

  return (
    <div className="accordion-item">
      <div className="accordion-header" onClick={onToggle}>
        <div className="accordion-title-container">
          {isOpen ? <ChevronDown size={18} /> : <ChevronRight size={18} />}
          <span>{def.label}</span>
        </div>
        <div className="accordion-header-status">
          {isConfigured && (
            <span className="status-badge ready"><Check size={12}/> {t('settings.llm.status_ready')}</span>
          )}
          {!isConfigured && def.requiresApiKey && (
             <span className="status-badge missing"><X size={12}/> {t('settings.llm.status_missing_api_key')}</span>
          )}
        </div>
      </div>
      {isOpen && (
        <div className="accordion-content">
           {def.id === 'google_translate_free' ? (
             <div className="settings-hint" style={{ color: 'var(--color-success)', marginBottom: '12px', fontSize: '0.95rem' }}>
               {t('settings.llm.free_service_hint')}
             </div>
           ) : (
           <>
           <div className="settings-item">
             <label className="settings-label" htmlFor={`llm-${def.id}-host`}>{def.apiHostLabel || t('settings.llm.base_url')}</label>
             {def.editableApiHost === false ? (
               <div className="settings-input" style={{ alignItems: 'center', display: 'flex', minHeight: 40, opacity: 0.75 }} id={`llm-${def.id}-host`}>
                 {setting?.apiHost || def.defaultApiHost}
               </div>
             ) : (
               <input
                 id={`llm-${def.id}-host`}
                 type="text"
                 className="settings-input"
                 value={setting?.apiHost || ''}
                 onChange={(e) => applyProviderUpdates({ apiHost: e.target.value })}
                 placeholder={def.defaultApiHost}
               />
             )}
           </div>

           <div className="settings-item">
             <label className="settings-label" htmlFor={`llm-${def.id}-key`}>{t('settings.llm.api_key')}</label>
             <input
               id={`llm-${def.id}-key`}
               type="password"
               className="settings-input"
               value={setting?.apiKey || ''}
               onChange={(e) => applyProviderUpdates({ apiKey: e.target.value })}
               placeholder={def.requiresApiKey ? 'sk-...' : t('settings.llm.optional_api_key')}
             />
           </div>

           {setting?.apiVersion !== undefined && (
             <div className="settings-item">
               <label className="settings-label" htmlFor={`llm-${def.id}-version`}>{t('settings.llm.api_version')}</label>
               <input
                 id={`llm-${def.id}-version`}
                 type="text"
                 className="settings-input"
                 value={setting.apiVersion}
                 onChange={(e) => applyProviderUpdates({ apiVersion: e.target.value })}
                 placeholder={def.defaultApiVersion || ''}
               />
             </div>
           )}

           {setting?.apiPath !== undefined && (
             <div className="settings-item">
               <label className="settings-label" htmlFor={`llm-${def.id}-path`}>{t('settings.llm.api_path')}</label>
               <input
                 id={`llm-${def.id}-path`}
                 type="text"
                 className="settings-input"
                 value={setting.apiPath}
                 onChange={(e) => applyProviderUpdates({ apiPath: e.target.value })}
                 readOnly={provider === 'open_ai_responses' || provider === 'volcengine' || provider === 'perplexity'}
               />
             </div>
           )}
           </>
           )}
           
           <div className="feature-field">
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {(() => {
                  let testBtnClass = 'btn-secondary';
                  let icon = null;
                  let label = t('settings.llm.test_connection');

                  if (testStatus === 'loading') {
                    icon = <Loader2 className="animate-spin" size={16} />;
                    label = t('settings.llm.testing');
                  } else if (testStatus === 'success') {
                    testBtnClass = 'btn-success-flash';
                    icon = <Check size={16} />;
                    label = t('settings.llm.connection_success');
                  } else if (testStatus === 'error') {
                    testBtnClass = 'btn-error-flash';
                    icon = <X size={16} />;
                    label = t('settings.llm.connection_failed');
                  }

                  return (
                    <button
                      type="button"
                      className={`btn ${testBtnClass} btn-loading-wrapper`}
                      style={{ width: 'fit-content', minWidth: '120px' }}
                      onClick={handleTestConnection}
                      disabled={testStatus === 'loading'}
                    >
                      <div className="btn-content-inner" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
                        {icon}
                        <span>{label}</span>
                      </div>
                    </button>
                  );
                })()}
                
                {testStatus === 'error' && testMessage && (
                  <div className="connection-error-detail">
                    <X size={12} />
                    <span>{testMessage}</span>
                  </div>
                )}
              </div>
           </div>
        </div>
      )}
    </div>
  );
}

// ------ MAIN TAB COMPONENT ------
export function SettingsLLMServiceTab(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const [expandedProvider, setExpandedProvider] = useState<LlmProvider | null>(null);
  const summaryEnabled = config.summaryEnabled ?? true;

  const applyLlmSettings = useCallback((nextLlmSettings: LlmAssistantConfig['llmSettings']) => {
    if (!nextLlmSettings) return;
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [updateConfig]);

  const applyProviderUpdates = useCallback((provider: LlmProvider, updates: Partial<LlmProviderSetting>) => {
    const currentLlmState = config.llmSettings ? { llmSettings: config.llmSettings } : ensureLlmState(config as any);
    const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, provider, updates);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [config, updateConfig]);

  const currentLlmState = config.llmSettings ? config.llmSettings : ensureLlmState(config as any).llmSettings;
  
  const activeProviders = useMemo(() => {
    const active = new Set<LlmProvider>();
    const polishModel = getFeatureModelEntry(config, 'polish');
    if (polishModel) active.add(polishModel.provider);
    
    const translationModel = getFeatureModelEntry(config, 'translation');
    if (translationModel) active.add(translationModel.provider);

    const summaryModel = getFeatureModelEntry(config, 'summary');
    if (summaryModel) active.add(summaryModel.provider);

    LLM_PROVIDER_DEFINITIONS.forEach(def => {
       const key = currentLlmState.providers[def.id]?.apiKey;
       if (key && key.trim()) {
          active.add(def.id);
       }
    });
    
    return Array.from(active);
  }, [config, currentLlmState]);

  useEffect(() => {
    if (!expandedProvider && activeProviders.length > 0) {
      setExpandedProvider(activeProviders[0]);
    } else if (!expandedProvider) {
      setExpandedProvider(LLM_PROVIDER_DEFINITIONS[0].id);
    }
  }, [activeProviders, expandedProvider]);

  return (
    <SettingsTabContainer id="settings-panel-llm" ariaLabelledby="settings-tab-llm">
      <SettingsPageHeader 
          icon={<RobotIcon width={28} height={28} />}
          title={t('settings.llm.title')} 
          description={t('settings.llm.description', { defaultValue: 'Configure LLM providers and models used for polishing, translating, and summarizing transcripts.' })} 
      />
      
      {/* 1. Feature Cards Section */}
      <SettingsSection 
        title={t('settings.llm.feature_models')}
        description={t('settings.llm.feature_models_runtime_hint')}
        icon={<Settings2 size={20} />}
      >
        <FeatureCard
          stepNumber={1}
          featureId="polish"
          title={t('settings.llm.polish_model')}
          icon={<Sparkles size={20} />}
          config={config}
          applyLlmSettings={applyLlmSettings}
          t={t}
        />
        <FeatureCard
          stepNumber={2}
          featureId="translation"
          title={t('settings.llm.translation_model')}
          icon={<Globe size={20} />}
          config={config}
          applyLlmSettings={applyLlmSettings}
          t={t}
        />
        <FeatureCard
          stepNumber={3}
          featureId="summary"
          title={t('settings.llm.summary_model')}
          icon={<AlignLeft size={20} />}
          config={config}
          applyLlmSettings={applyLlmSettings}
          t={t}
          featureEnabled={summaryEnabled}
          headerAction={(
            <div className="feature-card-toggle">
              <span className="feature-card-toggle-label">{t('settings.llm.enable_summary')}</span>
              <Switch
                checked={summaryEnabled}
                onChange={(enabled) => updateConfig({ summaryEnabled: enabled })}
                aria-label={t('settings.llm.enable_summary')}
              />
            </div>
          )}
        />
      </SettingsSection>

      {/* 2. Provider Credentials Section */}
      <SettingsSection
        title={t('settings.llm.credentials_section')}
        description={t('settings.llm.credentials_hint')}
        icon={<Settings2 size={20} />}
      >
        <div className="accordion-container">
          {LLM_PROVIDER_DEFINITIONS.map(def => def)
           .sort((a, b) => {
             const aActive = activeProviders.includes(a.id as LlmProvider);
             const bActive = activeProviders.includes(b.id as LlmProvider);
             if (aActive && !bActive) return -1;
             if (!aActive && bActive) return 1;
             return 0;
           })
           .map(def => (
             <ProviderAccordionItem
               key={def.id}
               provider={def.id}
               config={config}
               isOpen={expandedProvider === def.id}
               onToggle={() => setExpandedProvider(expandedProvider === def.id ? null : def.id)}
               applyProviderUpdates={(updates) => applyProviderUpdates(def.id, updates)}
               t={t}
             />
           ))
          }
        </div>
      </SettingsSection>
    </SettingsTabContainer>
  );
}
