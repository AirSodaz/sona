import React, { useMemo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings2, Sparkles, Globe, AlignLeft } from 'lucide-react';
import { RobotIcon } from '../Icons';
import { Switch } from '../Switch';
import { LlmProvider, LlmProviderSetting } from '../../types/transcript';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { LlmAssistantConfig } from '../../types/config';
import {
  buildLlmConfigPatch,
  getFeatureModelEntry,
  updateProviderSetting,
} from '../../services/llm/state';
import { LLM_PROVIDER_DEFINITIONS } from '../../services/llm/providers';
import { SettingsTabContainer, SettingsPageHeader, SettingsSection } from './SettingsLayout';
import { FeatureCard } from './llm/FeatureCard';
import { ProviderAccordionItem } from './llm/ProviderAccordionItem';
import { getCurrentLlmSettings, getCurrentLlmState } from './llm/helpers';
import './SettingsLLMServiceTab.css';

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
    const currentLlmState = getCurrentLlmState(config);
    const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, provider, updates);
    updateConfig(buildLlmConfigPatch(nextLlmSettings));
  }, [config, updateConfig]);

  const currentLlmState = getCurrentLlmSettings(config);
  const polishModel = getFeatureModelEntry(config, 'polish');
  const translationModel = getFeatureModelEntry(config, 'translation');
  const summaryModel = getFeatureModelEntry(config, 'summary');

  const activeProviders = useMemo(() => {
    const active = new Set<LlmProvider>();
    if (polishModel) active.add(polishModel.provider);
    if (translationModel) active.add(translationModel.provider);
    if (summaryModel) active.add(summaryModel.provider);

    LLM_PROVIDER_DEFINITIONS.forEach(def => {
       const key = currentLlmState.providers[def.id]?.apiKey;
       if (key && key.trim()) {
          active.add(def.id);
       }
    });

    return Array.from(active);
  }, [currentLlmState, polishModel, summaryModel, translationModel]);
  const fallbackExpandedProvider = activeProviders[0] ?? LLM_PROVIDER_DEFINITIONS[0].id;
  const effectiveExpandedProvider = expandedProvider ?? fallbackExpandedProvider;

  return (
    <SettingsTabContainer id="settings-panel-llm" ariaLabelledby="settings-tab-llm">
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
               isOpen={effectiveExpandedProvider === def.id}
               onToggle={() => setExpandedProvider(effectiveExpandedProvider === def.id ? null : def.id)}
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
