import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Check, Loader2, X, ChevronDown, ChevronRight, Settings2, Sparkles, Globe } from 'lucide-react';
import { RobotIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { AppConfig, LlmProvider, LlmProviderSetting } from '../../types/transcript';
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
} from '../../services/llmConfig';
import { SettingsTabContainer, SettingsPageHeader, SettingsSection } from './SettingsLayout';
import './SettingsLLMServiceTab.css';

interface SettingsLLMServiceTabProps {
  config: AppConfig;
  updateConfig: (updates: Partial<AppConfig>) => void;
  changeLlmServiceType: (type: LlmProvider) => void;
}

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
    default: return 'gpt-4o-mini';
  }
}

function isProviderConfigured(provider: LlmProvider, setting: LlmProviderSetting | undefined): boolean {
  if (!setting) return false;
  const def = getProviderDefinition(provider);
  if (def.requiresApiKey && !(setting.apiKey || '').trim()) return false;
  return true;
}

// ------ FEATURE CARD COMPONENT ------
interface FeatureCardProps {
  featureId: 'polish' | 'translation';
  title: string;
  icon: React.ReactNode;
  config: AppConfig;
  applyLlmSettings: (s: AppConfig['llmSettings']) => void;
  t: (key: string) => string;
}

function FeatureCard({ featureId, title, icon, config, applyLlmSettings, t }: FeatureCardProps) {
  const currentLlmState = config.llmSettings ? config.llmSettings : ensureLlmState(config as AppConfig & Record<string, any>).llmSettings;
  const modelEntry = getFeatureModelEntry(config, featureId);
  const selectedProvider = modelEntry?.provider || 'open_ai';
  const selectedModel = modelEntry?.model || '';
  const temperature = featureId === 'polish' 
    ? (currentLlmState.selections.polishTemperature ?? DEFAULT_LLM_TEMPERATURE)
    : (currentLlmState.selections.translationTemperature ?? DEFAULT_LLM_TEMPERATURE);

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
  
  const providerOptions = useMemo(() => LLM_PROVIDER_DEFINITIONS.map((p) => ({
    value: p.id,
    label: p.label,
  })), []);
  
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
    // don't commit it until they select a model. Just clear the UI model
    setLocalModelName('');
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

  return (
    <div className="feature-card">
      <div className="feature-card-header">
        <span className="feature-card-icon">{icon}</span>
        {title}
        <div style={{ marginLeft: 'auto' }}>
          {isComplete ? (
            <span className="status-badge ready"><Check size={12}/> {t('settings.llm.status_ready')}</span>
          ) : (
             <span className="status-badge missing">
               <X size={12}/> {selectedProvider && localModelName ? t('settings.llm.status_missing_api_key') : t('settings.llm.status_missing_model')}
             </span>
          )}
        </div>
      </div>
      
      <div>
        <label className="settings-label" style={{ marginBottom: 4, display: 'block', fontSize: '0.9rem' }}>{t('settings.llm.credential_provider')}</label>
        <Dropdown
          id={`provider-${featureId}`}
          value={localProvider}
          onChange={handleProviderChange}
          options={providerOptions}
          style={{ width: '100%' }}
        />
      </div>

      <div ref={candidateContainerRef} className="model-combobox-wrapper">
        <label className="settings-label" style={{ marginBottom: 4, display: 'block', fontSize: '0.9rem' }}>{t('settings.llm.model_library')}</label>
        <div className="dropdown-container" style={{ margin: 0 }}>
          <input
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
            <div className="settings-hint" style={{ position: 'absolute', right: 12, top: 10 }}>
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

      <div>
         <label className="settings-label" style={{ marginBottom: 4, display: 'block', fontSize: '0.9rem' }}>{t(featureId === 'polish' ? 'settings.llm.polish_temperature' : 'settings.llm.translation_temperature')}</label>
         <div className="feature-temperature-container">
           <input
             type="range"
             className="feature-temperature-slider"
             min={0}
             max={2}
             step={0.05}
             value={temperature}
             onChange={(e) => handleTempChange(parseFloat(e.target.value))}
           />
           <input
             type="number"
             className="settings-input"
             style={{ padding: '4px 6px', textAlign: 'center' }}
             min={0}
             max={2}
             step={0.05}
             value={temperature}
             onChange={(e) => {
               const val = parseFloat(e.target.value);
               if (!Number.isNaN(val) && val >= 0 && val <= 2) handleTempChange(val);
             }}
           />
         </div>
      </div>
    </div>
  );
}

// ------ ACCORDION ITEM COMPONENT ------
interface AccordionItemProps {
  provider: LlmProvider;
  config: AppConfig;
  isOpen: boolean;
  onToggle: () => void;
  applyProviderUpdates: (updates: Partial<LlmProviderSetting>) => void;
  t: (key: string) => string;
}

function ProviderAccordionItem({ provider, config, isOpen, onToggle, applyProviderUpdates, t }: AccordionItemProps) {
  const currentLlmState = config.llmSettings ? config.llmSettings : ensureLlmState(config as AppConfig & Record<string, any>).llmSettings;
  const def = getProviderDefinition(provider);
  const setting = currentLlmState.providers[provider];
  
  const isConfigured = isProviderConfigured(provider, setting);
  
  const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [testMessage, setTestMessage] = useState('');

  const handleTestConnection = async () => {
    if (!setting) return;
    setTestStatus('loading');
    setTestMessage('');
    try {
      const providerConfig = buildLlmConfig(provider, setting || { apiHost: def.defaultApiHost, apiKey: '' });
      // Get a model to test
      const entryId = currentLlmState.modelOrder.find(id => currentLlmState.models[id].provider === provider);
      const testModel = entryId ? currentLlmState.models[entryId].model : getModelPlaceholder(provider);
      
      const testProviderConfig = { ...providerConfig, model: testModel };

      await invoke<string>('generate_llm_text', {
        request: { config: testProviderConfig, input: 'Hello, this is a connection test.' },
      });
      setTestStatus('success');
      setTestMessage(testModel);
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
           <div className="settings-item">
             <label className="settings-label">{def.apiHostLabel || t('settings.llm.base_url')}</label>
             {def.editableApiHost === false ? (
               <div className="settings-input" style={{ alignItems: 'center', display: 'flex', minHeight: 40, opacity: 0.75 }}>
                 {setting?.apiHost || def.defaultApiHost}
               </div>
             ) : (
               <input
                 type="text"
                 className="settings-input"
                 value={setting?.apiHost || ''}
                 onChange={(e) => applyProviderUpdates({ apiHost: e.target.value })}
                 placeholder={def.defaultApiHost}
               />
             )}
           </div>

           <div className="settings-item">
             <label className="settings-label">{t('settings.llm.api_key')}</label>
             <input
               type="password"
               className="settings-input"
               value={setting?.apiKey || ''}
               onChange={(e) => applyProviderUpdates({ apiKey: e.target.value })}
               placeholder={def.requiresApiKey ? 'sk-...' : t('settings.llm.optional_api_key')}
             />
           </div>

           {setting?.apiVersion !== undefined && (
             <div className="settings-item">
               <label className="settings-label">{t('settings.llm.api_version')}</label>
               <input
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
               <label className="settings-label">{t('settings.llm.api_path')}</label>
               <input
                 type="text"
                 className="settings-input"
                 value={setting.apiPath}
                 onChange={(e) => applyProviderUpdates({ apiPath: e.target.value })}
                 readOnly={provider === 'open_ai_responses' || provider === 'volcengine' || provider === 'perplexity'}
               />
             </div>
           )}
           
           <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginTop: '8px' }}>
              <button
                 type="button"
                 className="btn btn-secondary btn-loading-wrapper"
                 onClick={handleTestConnection}
                 disabled={testStatus === 'loading'}
               >
                 <span className={testStatus === 'loading' ? 'btn-text-hidden' : ''}>{t('settings.llm.test_connection')}</span>
                 {testStatus === 'loading' && (
                   <div className="btn-spinner-overlay"><Loader2 className="animate-spin" size={16} /></div>
                 )}
              </button>
              
              {testMessage && (
                <div className={`connection-status ${testStatus === 'error' ? 'error' : 'success'}`} style={{ margin: 0, padding: 0 }}>
                  {testStatus === 'error' ? <X size={16} style={{ marginTop: 2, marginRight: 4 }} /> : <Check size={16} style={{ marginTop: 2, marginRight: 4 }} />}
                  <span style={{ fontSize: '0.85rem' }}>
                     {testStatus === 'error' ? t('settings.llm.connection_failed') : t('settings.llm.connection_success')}
                     {testStatus === 'error' && testMessage ? `: ${testMessage}` : ''}
                  </span>
                </div>
              )}
           </div>
        </div>
      )}
    </div>
  );
}

// ------ MAIN TAB COMPONENT ------
export function SettingsLLMServiceTab({
  config,
  updateConfig,
}: SettingsLLMServiceTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const [expandedProvider, setExpandedProvider] = useState<LlmProvider | null>(null);

  const applyLlmSettings = useCallback((nextLlmSettings: AppConfig['llmSettings']) => {
    if (!nextLlmSettings) return;
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [updateConfig]);

  const applyProviderUpdates = useCallback((provider: LlmProvider, updates: Partial<LlmProviderSetting>) => {
    const currentLlmState = config.llmSettings ? { llmSettings: config.llmSettings } : ensureLlmState(config as AppConfig & Record<string, any>);
    const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, provider, updates);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [config, updateConfig]);

  const currentLlmState = config.llmSettings ? config.llmSettings : ensureLlmState(config as AppConfig & Record<string, any>).llmSettings;
  
  const activeProviders = useMemo(() => {
    const active = new Set<LlmProvider>();
    const polishModel = getFeatureModelEntry(config, 'polish');
    if (polishModel) active.add(polishModel.provider);
    
    const translationModel = getFeatureModelEntry(config, 'translation');
    if (translationModel) active.add(translationModel.provider);

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
          description={t('settings.llm.description', { defaultValue: 'Configure LLM providers and models used for polishing and translating transcripts.' })} 
      />
      
      {/* 1. Feature Cards Section */}
      <SettingsSection 
        title={t('settings.llm.feature_models')}
        description={t('settings.llm.feature_models_runtime_hint')}
        icon={<Settings2 size={20} />}
      >
        <div className="feature-cards-grid">
           <FeatureCard
             featureId="polish"
             title={t('settings.llm.polish_model')}
             icon={<Sparkles size={20} color="var(--color-primary, #646cff)" />}
             config={config}
             applyLlmSettings={applyLlmSettings}
             t={t}
           />
           <FeatureCard
             featureId="translation"
             title={t('settings.llm.translation_model')}
             icon={<Globe size={20} color="var(--color-primary, #646cff)" />}
             config={config}
             applyLlmSettings={applyLlmSettings}
             t={t}
           />
        </div>
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
