import React, { useMemo, useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings2, Sparkles, Globe, AlignLeft, Plus, X } from 'lucide-react';
import { RobotIcon } from '../Icons';
import { CustomLlmProviderStrategy, LlmFeature, LlmProvider, LlmProviderSetting } from '../../types/transcript';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { LlmAssistantConfig } from '../../types/config';
import {
  addCustomProvider,
  buildLlmConfigPatch,
  getFeatureModelEntry,
  removeCustomProvider,
  setActiveProvider,
  updateCustomProvider,
  updateProviderSetting,
} from '../../services/llm/state';
import { createProviderSetting, getProviderDefinition, listProviderDefinitions } from '../../services/llm/providers';
import { SettingsTabContainer, SettingsPageHeader, SettingsSection, SettingsItem } from './SettingsLayout';
import { FeatureCard } from './llm/FeatureCard';
import { ProviderAccordionItem } from './llm/ProviderAccordionItem';
import { getCurrentLlmSettings, getCurrentLlmState, isProviderConfiguredForConfig } from './llm/helpers';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import './SettingsLLMServiceTab.css';

interface SettingsLLMServiceTabProps {
  isActive?: boolean;
  onOpenProviderDetails?: (provider: LlmProvider) => void;
}

export const SettingsLLMServiceTab = React.memo(function SettingsLLMServiceTab({
  isActive = true,
  onOpenProviderDetails,
}: SettingsLLMServiceTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const [expandedProvider, setExpandedProvider] = useState<LlmProvider | null>(null);
  const [isAddProviderOpen, setIsAddProviderOpen] = useState(false);
  const [providerToAdd, setProviderToAdd] = useState<LlmProvider | null>(null);
  const [activeFeature, setActiveFeature] = useState<LlmFeature>('polish');
  const [editingProvider, setEditingProvider] = useState<LlmProvider | null>(null);
  const [editProviderName, setEditProviderName] = useState('');
  const [editProviderStrategy, setEditProviderStrategy] = useState<CustomLlmProviderStrategy>('openai_compatible');
  const [setupApiHost, setSetupApiHost] = useState('');
  const [setupApiKey, setSetupApiKey] = useState('');
  const [customProviderName, setCustomProviderName] = useState('');
  const [customProviderStrategy, setCustomProviderStrategy] = useState<CustomLlmProviderStrategy>('openai_compatible');
  const addProviderModalRef = useRef<HTMLDivElement>(null);
  const editProviderModalRef = useRef<HTMLDivElement>(null);

  const handleCloseAddProvider = useCallback(() => {
    setIsAddProviderOpen(false);
    setProviderToAdd(null);
    setCustomProviderName('');
  }, []);

  const handleCloseEditProvider = useCallback(() => {
    setEditingProvider(null);
  }, []);

  useFocusTrap(isAddProviderOpen, handleCloseAddProvider, addProviderModalRef);
  useFocusTrap(Boolean(editingProvider), handleCloseEditProvider, editProviderModalRef);

  const applyLlmSettings = useCallback((nextLlmSettings: LlmAssistantConfig['llmSettings']) => {
    if (!nextLlmSettings) return;
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [updateConfig]);

  const applyProviderUpdates = useCallback((provider: LlmProvider, updates: Partial<LlmProviderSetting>) => {
    const currentLlmState = getCurrentLlmState(config);
    const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, provider, updates);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [config, updateConfig]);

  const currentLlmState = getCurrentLlmSettings(config);
  const selectedProviderDefinition = providerToAdd
    ? getProviderDefinition(providerToAdd, currentLlmState.customProviders)
    : null;
  const providerDefinitions = useMemo(
    () => listProviderDefinitions(currentLlmState.customProviders),
    [currentLlmState.customProviders],
  );
  const orderedProviderDefinitions = useMemo(
    () => [...providerDefinitions].sort((a, b) => {
      const aIsCustom = a.id.startsWith('custom-');
      const bIsCustom = b.id.startsWith('custom-');
      if (aIsCustom && !bIsCustom) return 1;
      if (!aIsCustom && bIsCustom) return -1;
      return 0;
    }),
    [providerDefinitions],
  );
  const configuredProviderDefinitions = useMemo(
    () => orderedProviderDefinitions.filter((def) => def.id !== 'google_translate_free' && isProviderConfiguredForConfig(
      config,
      def.id,
      currentLlmState.providers[def.id],
    )),
    [config, currentLlmState.providers, orderedProviderDefinitions],
  );
  const availableProviderDefinitions = useMemo(
    () => orderedProviderDefinitions.filter((def) => !isProviderConfiguredForConfig(
      config,
      def.id,
      currentLlmState.providers[def.id],
    )),
    [config, currentLlmState.providers, orderedProviderDefinitions],
  );
  const polishModel = getFeatureModelEntry(config, 'polish');
  const translationModel = getFeatureModelEntry(config, 'translation');
  const summaryModel = getFeatureModelEntry(config, 'summary');

  const effectiveExpandedProvider = expandedProvider;

  const openAddProvider = () => {
    setProviderToAdd(null);
    setSetupApiHost('');
    setSetupApiKey('');
    setCustomProviderName('');
    setIsAddProviderOpen(true);
  };

  const selectProviderToAdd = (provider: LlmProvider) => {
    const setting = currentLlmState.providers[provider] ?? createProviderSetting(provider, currentLlmState.customProviders);
    setProviderToAdd(provider);
    setSetupApiHost(setting.apiHost);
    setSetupApiKey(setting.apiKey);
  };

  const handleAddProvider = () => {
    if (!providerToAdd) return;
    const seeded = setActiveProvider(currentLlmState, providerToAdd);
    const nextLlmSettings = updateProviderSetting(seeded, providerToAdd, {
      apiHost: setupApiHost.trim(),
      apiKey: setupApiKey,
    });
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    setExpandedProvider(providerToAdd);
    setIsAddProviderOpen(false);
    setProviderToAdd(null);
  };

  const openEditProvider = (provider: LlmProvider) => {
    const custom = currentLlmState.customProviders?.[provider as `custom-${string}`];
    if (!custom) return;
    setEditingProvider(provider);
    setEditProviderName(custom.name);
    setEditProviderStrategy(custom.strategy);
  };

  const handleSaveProviderEdit = () => {
    if (!editingProvider || !editProviderName.trim()) return;
    const nextLlmSettings = updateCustomProvider(currentLlmState, editingProvider, {
      name: editProviderName.trim(),
      strategy: editProviderStrategy,
    });
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    setEditingProvider(null);
  };

  const handleDeleteProvider = (provider: LlmProvider) => {
    if (!window.confirm(t('settings.llm.delete_provider_confirm', { defaultValue: 'Delete this provider and its saved models?' }))) return;
    const nextLlmSettings = removeCustomProvider(currentLlmState, provider);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    setExpandedProvider(null);
  };

  const handleAddCustomProvider = () => {
    const name = customProviderName.trim();
    if (!name) {
      return;
    }

    const nextLlmSettings = addCustomProvider(currentLlmState, {
      name,
      strategy: customProviderStrategy,
    });
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
    const setting = nextLlmSettings.providers[nextLlmSettings.activeProvider] ?? createProviderSetting(nextLlmSettings.activeProvider, nextLlmSettings.customProviders);
    setProviderToAdd(nextLlmSettings.activeProvider);
    setSetupApiHost(setting.apiHost);
    setSetupApiKey(setting.apiKey);
    setCustomProviderName('');
    setCustomProviderStrategy('openai_compatible');
  };

  return (
    <SettingsTabContainer id="settings-panel-llm_service" ariaLabelledby="settings-tab-llm_service">
      <SettingsPageHeader
          icon={<RobotIcon width={28} height={28} />}
          title={t('settings.llm.title')}
          description={t('settings.llm.description', { defaultValue: 'Configure LLM providers and models used for polishing, translating, and summarizing transcripts.' })}
      />

      <SettingsSection
        title={t('settings.llm.feature_models')}
        description={t('settings.llm.feature_models_runtime_hint')}
        icon={<Settings2 size={20} />}
      >
        <div className="settings-scenario-cards llm-feature-tabs" role="tablist" aria-label={t('settings.llm.feature_models')}>
          {([
            {
              value: 'polish' as const,
              label: t('settings.llm.polish_model'),
              description: t('settings.llm.polish_model_description', { defaultValue: 'Improve wording and readability' }),
              icon: <Sparkles size={18} />,
            },
            {
              value: 'translation' as const,
              label: t('settings.llm.translation_model'),
              description: t('settings.llm.translation_model_description', { defaultValue: 'Translate transcript text between languages' }),
              icon: <Globe size={18} />,
            },
            {
              value: 'summary' as const,
              label: t('settings.llm.summary_model'),
              description: t('settings.llm.summary_model_description', { defaultValue: 'Create concise summaries from transcripts' }),
              icon: <AlignLeft size={18} />,
            },
          ]).map(({ value, label, description, icon }) => (
            <button id={`settings-llm-feature-tab-${value}`} key={value} type="button" role="tab" aria-label={label} aria-selected={activeFeature === value} aria-controls="settings-llm-feature-panel" className={`settings-scenario-card${activeFeature === value ? ' active' : ''}`} onClick={() => setActiveFeature(value)}>
              <span className="settings-scenario-card-icon">{icon}</span>
              <span className="settings-scenario-card-text">
                <span className="settings-scenario-card-label">{label}</span>
                <span className="settings-scenario-card-description">{description}</span>
              </span>
            </button>
          ))}
        </div>
        <div id="settings-llm-feature-panel" className="llm-feature-panel" role="tabpanel" aria-labelledby={`settings-llm-feature-tab-${activeFeature}`}>
          {activeFeature === 'polish' && <FeatureCard key={`polish:${polishModel?.provider ?? 'open_ai'}:${polishModel?.model ?? ''}`} stepNumber={1} featureId="polish" title={t('settings.llm.polish_model')} icon={<Sparkles size={20} />} config={config} applyLlmSettings={applyLlmSettings} t={t} isActive={isActive} showHeaderTitle={false} />}
          {activeFeature === 'translation' && <FeatureCard key={`translation:${translationModel?.provider ?? 'open_ai'}:${translationModel?.model ?? ''}`} stepNumber={2} featureId="translation" title={t('settings.llm.translation_model')} icon={<Globe size={20} />} config={config} applyLlmSettings={applyLlmSettings} t={t} isActive={isActive} showHeaderTitle={false} />}
          {activeFeature === 'summary' && <FeatureCard key={`summary:${summaryModel?.provider ?? 'open_ai'}:${summaryModel?.model ?? ''}`} stepNumber={3} featureId="summary" title={t('settings.llm.summary_model')} icon={<AlignLeft size={20} />} config={config} applyLlmSettings={applyLlmSettings} t={t} isActive={isActive} showHeaderTitle={false} />}
        </div>
      </SettingsSection>

      <SettingsSection
        title={t('settings.llm.credentials_section')}
        description={t('settings.llm.credentials_hint')}
        icon={<Settings2 size={20} />}
        contentClassName="accordion-container"
      >
        {configuredProviderDefinitions.map(def => (
          <ProviderAccordionItem
             key={def.id}
             provider={def.id}
             config={config}
             isOpen={effectiveExpandedProvider === def.id}
             onToggle={() => setExpandedProvider(effectiveExpandedProvider === def.id ? null : def.id)}
             applyProviderUpdates={(updates) => applyProviderUpdates(def.id, updates)}
             onOpenDetails={onOpenProviderDetails ? () => onOpenProviderDetails(def.id) : undefined}
             onEdit={def.id.startsWith('custom-') ? () => openEditProvider(def.id) : undefined}
             onDelete={def.id.startsWith('custom-') ? () => handleDeleteProvider(def.id) : undefined}
             t={t}
           />
         ))
        }
        {configuredProviderDefinitions.length === 0 && (
          <div className="settings-model-empty provider-empty-state">
            {t('settings.llm.no_configured_providers', { defaultValue: 'No providers configured yet.' })}
          </div>
        )}
        <div className="custom-provider-actions">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={openAddProvider}
          >
            <Plus size={16} />
            <span>{t('settings.llm.add_custom_provider')}</span>
          </button>
        </div>
      </SettingsSection>

      <SettingsSection>
        <SettingsItem
          title={t('settings.llm.timeout_label', { defaultValue: 'Request Timeout (s)' })}
          hint={t('settings.llm.timeout_hint', { defaultValue: 'Maximum time allowed for an LLM request to complete, in seconds. Default is 180.' })}
        >
          <input
            type="number"
            className="input-text"
            value={config.llmRequestTimeoutSeconds ?? 180}
            onChange={(e) => updateConfig({ llmRequestTimeoutSeconds: parseInt(e.target.value, 10) || 180 })}
            min={1}
            max={3600}
            style={{ width: '120px' }}
          />
        </SettingsItem>
      </SettingsSection>

      {isAddProviderOpen && (
        <div
          className="provider-modal-backdrop"
          data-focus-trap-overlay="true"
          onClick={handleCloseAddProvider}
        >
          <div
            ref={addProviderModalRef}
            className="provider-modal"
            role="dialog"
            aria-modal="true"
            aria-label={t('settings.llm.add_custom_provider')}
            tabIndex={-1}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="provider-modal-header">
              <h3>{providerToAdd
                ? t('settings.llm.configure_provider', { defaultValue: 'Configure provider' })
                : t('settings.llm.add_provider', { defaultValue: 'Add provider' })}</h3>
              <button
                type="button"
                className="btn btn-icon btn-secondary-soft"
                aria-label={t('settings.llm.close_add_custom_provider')}
                onClick={handleCloseAddProvider}
              >
                <X size={16} />
              </button>
            </div>

            <div className="provider-modal-body">
              {!providerToAdd ? (
                <>
                  {(['llm', 'translation'] as const).map((category) => {
                    const providers = availableProviderDefinitions.filter((def) => category === 'translation'
                      ? def.id === 'google_translate' || def.id === 'google_translate_free'
                      : def.id !== 'google_translate' && def.id !== 'google_translate_free');
                    if (providers.length === 0) return null;
                    return (
                      <div className="provider-picker-group" key={category}>
                        <div className="settings-label">{category === 'llm'
                          ? t('settings.llm.provider_category_llm', { defaultValue: 'LLM' })
                          : t('settings.llm.provider_category_translation', { defaultValue: 'Translation model / API' })}</div>
                        <div className="provider-picker-grid">
                          {providers.map((def) => (
                            <button key={def.id} type="button" className="provider-picker-option" onClick={() => selectProviderToAdd(def.id)}>
                              <span>{t(def.labelKey, { defaultValue: def.labelDefault })}</span>
                              <span className="provider-picker-meta">{def.requiresApiKey ? t('settings.llm.requires_api_key', { defaultValue: 'API key' }) : t('settings.llm.no_api_key_required', { defaultValue: 'No key required' })}</span>
                            </button>
                          ))}
                        </div>
                      </div>
                    );
                  })}
                  <div className="provider-picker-group">
                    <div className="settings-label">{t('settings.llm.custom_provider', { defaultValue: 'Custom provider' })}</div>
                    <input id="custom-provider-name" aria-label={t('settings.llm.custom_provider_name')} className="settings-input" type="text" value={customProviderName} onChange={(event) => setCustomProviderName(event.target.value)} placeholder={t('settings.llm.custom_provider_name')} />
                    <div className="provider-mode-options">
                      {[
                        ['openai_compatible', t('settings.llm.api_mode_openai_compatible')],
                        ['openai_responses', t('settings.llm.api_mode_openai_responses')],
                        ['anthropic', t('settings.llm.api_mode_claude')],
                        ['gemini', t('settings.llm.api_mode_gemini')],
                      ].map(([strategy, label]) => (
                        <button key={strategy} type="button" className={`provider-mode-option ${customProviderStrategy === strategy ? 'selected' : ''}`} aria-pressed={customProviderStrategy === strategy} onClick={() => setCustomProviderStrategy(strategy as CustomLlmProviderStrategy)}>{label}</button>
                      ))}
                    </div>
                  </div>
                </>
              ) : (
                <>
                  <div className="settings-item">
                    <label className="settings-label" htmlFor="setup-provider-host">{t('settings.llm.base_url')}</label>
                    <input id="setup-provider-host" className="settings-input" value={setupApiHost} onChange={(event) => setSetupApiHost(event.target.value)} autoFocus />
                  </div>
                  {getProviderDefinition(providerToAdd, currentLlmState.customProviders).requiresApiKey && (
                    <div className="settings-item">
                      <label className="settings-label" htmlFor="setup-provider-key">{t('settings.llm.api_key')}</label>
                      <input id="setup-provider-key" className="settings-input" type="password" value={setupApiKey} onChange={(event) => setSetupApiKey(event.target.value)} />
                    </div>
                  )}
                </>
              )}
            </div>

            <div className="provider-modal-footer">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => providerToAdd ? setProviderToAdd(null) : handleCloseAddProvider()}
              >
                {providerToAdd ? t('common.back', { defaultValue: 'Back' }) : t('settings.llm.add_custom_provider_cancel')}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => providerToAdd ? handleAddProvider() : handleAddCustomProvider()}
                disabled={providerToAdd
                  ? ((!setupApiHost.trim() && !selectedProviderDefinition?.defaultApiHost)
                    || (selectedProviderDefinition?.requiresApiKey && !setupApiKey.trim()))
                  : !customProviderName.trim()}
              >
                {providerToAdd ? t('settings.llm.save_provider', { defaultValue: 'Save provider' }) : t('settings.llm.add_custom_provider_confirm')}
              </button>
            </div>
          </div>
        </div>
      )}

      {editingProvider && (
        <div
          className="provider-modal-backdrop"
          data-focus-trap-overlay="true"
          onClick={handleCloseEditProvider}
        >
          <div
            ref={editProviderModalRef}
            className="provider-modal"
            role="dialog"
            aria-modal="true"
            aria-label={t('settings.llm.edit_provider', { defaultValue: 'Edit provider' })}
            tabIndex={-1}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="provider-modal-header">
              <h3>{t('settings.llm.edit_provider', { defaultValue: 'Edit provider' })}</h3>
              <button
                type="button"
                className="btn btn-icon btn-secondary-soft"
                aria-label={t('settings.llm.close_edit_provider', { defaultValue: 'Close edit provider' })}
                onClick={handleCloseEditProvider}
              >
                <X size={16} />
              </button>
            </div>
            <div className="provider-modal-body">
              <div className="settings-item">
                <label className="settings-label" htmlFor="edit-provider-name">{t('settings.llm.custom_provider_name')}</label>
                <input id="edit-provider-name" className="settings-input" value={editProviderName} onChange={(event) => setEditProviderName(event.target.value)} autoFocus />
              </div>
              <div className="settings-item">
                <span className="settings-label">{t('settings.llm.custom_provider_api_mode')}</span>
                <div className="provider-mode-options">
                  {[
                    ['openai_compatible', t('settings.llm.api_mode_openai_compatible')],
                    ['openai_responses', t('settings.llm.api_mode_openai_responses')],
                    ['anthropic', t('settings.llm.api_mode_claude')],
                    ['gemini', t('settings.llm.api_mode_gemini')],
                  ].map(([strategy, label]) => (
                    <button key={strategy} type="button" className={`provider-mode-option ${editProviderStrategy === strategy ? 'selected' : ''}`} aria-pressed={editProviderStrategy === strategy} onClick={() => setEditProviderStrategy(strategy as CustomLlmProviderStrategy)}>{label}</button>
                  ))}
                </div>
              </div>
            </div>
            <div className="provider-modal-footer">
              <button type="button" className="btn btn-secondary" onClick={handleCloseEditProvider}>{t('common.cancel')}</button>
              <button type="button" className="btn btn-primary" onClick={handleSaveProviderEdit} disabled={!editProviderName.trim()}>{t('common.save')}</button>
            </div>
          </div>
        </div>
      )}

    </SettingsTabContainer>
  );
});
