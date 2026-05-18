import React, { useMemo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings2, Sparkles, Globe, AlignLeft, Plus, X } from 'lucide-react';
import { RobotIcon } from '../Icons';
import { Switch } from '../Switch';
import { CustomLlmProviderStrategy, LlmProvider, LlmProviderSetting } from '../../types/transcript';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { LlmAssistantConfig } from '../../types/config';
import {
  addCustomProvider,
  buildLlmConfigPatch,
  getFeatureModelEntry,
  updateProviderSetting,
} from '../../services/llm/state';
import { listProviderDefinitions } from '../../services/llm/providers';
import { SettingsTabContainer, SettingsPageHeader, SettingsSection } from './SettingsLayout';
import { FeatureCard } from './llm/FeatureCard';
import { ProviderAccordionItem } from './llm/ProviderAccordionItem';
import { getCurrentLlmSettings, getCurrentLlmState } from './llm/helpers';
import './SettingsLLMServiceTab.css';

interface SettingsLLMServiceTabProps {
  isActive?: boolean;
}

export const SettingsLLMServiceTab = React.memo(function SettingsLLMServiceTab({ isActive = true }: SettingsLLMServiceTabProps): React.JSX.Element {
  const { t } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();
  const [expandedProvider, setExpandedProvider] = useState<LlmProvider | null>(null);
  const [isAddProviderOpen, setIsAddProviderOpen] = useState(false);
  const [customProviderName, setCustomProviderName] = useState('');
  const [customProviderStrategy, setCustomProviderStrategy] = useState<CustomLlmProviderStrategy>('openai_compatible');
  const summaryEnabled = config.summaryEnabled ?? true;

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
  const polishModel = getFeatureModelEntry(config, 'polish');
  const translationModel = getFeatureModelEntry(config, 'translation');
  const summaryModel = getFeatureModelEntry(config, 'summary');

  const activeProviders = useMemo(() => {
    const active = new Set<LlmProvider>();
    active.add(currentLlmState.activeProvider);
    if (polishModel) active.add(polishModel.provider);
    if (translationModel) active.add(translationModel.provider);
    if (summaryModel) active.add(summaryModel.provider);

    providerDefinitions.forEach(def => {
       const key = currentLlmState.providers[def.id]?.apiKey;
       if (key && key.trim()) {
          active.add(def.id);
       }
    });

    return Array.from(active);
  }, [currentLlmState, polishModel, providerDefinitions, summaryModel, translationModel]);
  const fallbackExpandedProvider = activeProviders[0] ?? providerDefinitions[0].id;
  const effectiveExpandedProvider = expandedProvider ?? fallbackExpandedProvider;

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
    setExpandedProvider(nextLlmSettings.activeProvider);
    setCustomProviderName('');
    setCustomProviderStrategy('openai_compatible');
    setIsAddProviderOpen(false);
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
        <FeatureCard
          key={`polish:${polishModel?.provider ?? 'open_ai'}:${polishModel?.model ?? ''}`}
          stepNumber={1}
          featureId="polish"
          title={t('settings.llm.polish_model')}
          icon={<Sparkles size={20} />}
          config={config}
          applyLlmSettings={applyLlmSettings}
          t={t}
          isActive={isActive}
        />
        <FeatureCard
          key={`translation:${translationModel?.provider ?? 'open_ai'}:${translationModel?.model ?? ''}`}
          stepNumber={2}
          featureId="translation"
          title={t('settings.llm.translation_model')}
          icon={<Globe size={20} />}
          config={config}
          applyLlmSettings={applyLlmSettings}
          t={t}
          isActive={isActive}
        />
        <FeatureCard
          key={`summary:${summaryModel?.provider ?? 'open_ai'}:${summaryModel?.model ?? ''}`}
          stepNumber={3}
          featureId="summary"
          title={t('settings.llm.summary_model')}
          icon={<AlignLeft size={20} />}
          config={config}
          applyLlmSettings={applyLlmSettings}
          t={t}
          isActive={isActive}
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

      <SettingsSection
        title={t('settings.llm.credentials_section')}
        description={t('settings.llm.credentials_hint')}
        icon={<Settings2 size={20} />}
      >
        <div className="accordion-container">
          {orderedProviderDefinitions.map(def => (
            <ProviderAccordionItem
               key={def.id}
               provider={def.id}
               config={config}
               isOpen={effectiveExpandedProvider === def.id}
               onToggle={() => setExpandedProvider(effectiveExpandedProvider === def.id ? null : def.id)}
               applyProviderUpdates={(updates) => applyProviderUpdates(def.id, updates)}
               t={t}
             />
           ))
          }
          <div className="custom-provider-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setIsAddProviderOpen(true)}
            >
              <Plus size={16} />
              <span>{t('settings.llm.add_custom_provider')}</span>
            </button>
          </div>
        </div>
      </SettingsSection>

      {isAddProviderOpen && (
        <div className="provider-modal-backdrop">
          <div
            className="provider-modal"
            role="dialog"
            aria-modal="true"
            aria-label={t('settings.llm.add_custom_provider')}
          >
            <div className="provider-modal-header">
              <h3>{t('settings.llm.add_custom_provider')}</h3>
              <button
                type="button"
                className="btn btn-icon btn-secondary-soft"
                aria-label={t('settings.llm.close_add_custom_provider')}
                onClick={() => setIsAddProviderOpen(false)}
              >
                <X size={16} />
              </button>
            </div>

            <div className="provider-modal-body">
              <div className="settings-item">
                <label className="settings-label" htmlFor="custom-provider-name">
                  {t('settings.llm.custom_provider_name')}
                </label>
                <input
                  id="custom-provider-name"
                  className="settings-input"
                  type="text"
                  value={customProviderName}
                  onChange={(event) => setCustomProviderName(event.target.value)}
                  autoFocus
                />
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
                    <button
                      key={strategy}
                      type="button"
                      className={`provider-mode-option ${customProviderStrategy === strategy ? 'selected' : ''}`}
                      aria-pressed={customProviderStrategy === strategy}
                      onClick={() => setCustomProviderStrategy(strategy as CustomLlmProviderStrategy)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            <div className="provider-modal-footer">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setIsAddProviderOpen(false)}
              >
                {t('settings.llm.add_custom_provider_cancel')}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleAddCustomProvider}
                disabled={!customProviderName.trim()}
              >
                {t('settings.llm.add_custom_provider_confirm')}
              </button>
            </div>
          </div>
        </div>
      )}
    </SettingsTabContainer>
  );
});
