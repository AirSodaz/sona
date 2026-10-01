import { ArrowRight, Bot, Languages, Sparkles, SpellCheck, Users } from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { isFeatureLlmConfigComplete } from '../../services/llm/configUtils';
import { getFeatureModelEntry } from '../../services/llm/state';
import { useLlmAssistantConfig, useSetConfig, useVocabularyConfig } from '../../stores/configStore';
import { BookIcon, SummaryIcon } from '../Icons';
import { SettingsContextSection } from './SettingsContextSection';
import {
  ScenarioCardTabs,
  SegmentedSubTabs,
  SettingsPageHeader,
  SettingsTabContainer,
} from './SettingsLayout';
import { useOptionalSettingsNavigation } from './SettingsNavigationContext';
import { SettingsSpeakerProfilesSection } from './SettingsSpeakerProfilesSection';
import { SettingsSummaryTemplateSection } from './SettingsSummaryTemplateSection';
import { SettingsTranslationSection } from './SettingsTranslationSection';
import { UnifiedDictionarySection } from './vocabulary/UnifiedDictionarySection';
export type VocabularySubTab = 'recognition' | 'prompts' | 'speakers';
export type PromptsSubTab = 'polish' | 'translation' | 'summary';

export interface SettingsVocabularyTabProps {
  initialSubTab?: VocabularySubTab;
  initialPromptsSubTab?: PromptsSubTab;
}

export function SettingsVocabularyTab({
  initialSubTab = 'recognition',
  initialPromptsSubTab = 'polish',
}: SettingsVocabularyTabProps = {}): React.JSX.Element {
  const [activeSubTab, setActiveSubTab] = useState<VocabularySubTab>(initialSubTab);
  const [activePromptsSubTab, setActivePromptsSubTab] = useState<PromptsSubTab>(
    initialPromptsSubTab || 'polish'
  );

  useEffect(() => {
    if (initialSubTab) {
      setActiveSubTab(initialSubTab);
    }
  }, [initialSubTab]);

  useEffect(() => {
    if (initialPromptsSubTab) {
      setActivePromptsSubTab(initialPromptsSubTab);
    }
  }, [initialPromptsSubTab]);

  const { t } = useTranslation();
  const config = useVocabularyConfig();
  const updateConfig = useSetConfig();
  const llmConfig = useLlmAssistantConfig();
  const navContext = useOptionalSettingsNavigation();

  const isLlmConfigured = isFeatureLlmConfigComplete(llmConfig, activePromptsSubTab);
  const activeModelEntry = getFeatureModelEntry(llmConfig, activePromptsSubTab);
  const modelDisplayName = activeModelEntry?.metadata?.displayName || activeModelEntry?.model;

  const llmStatusLabel = isLlmConfigured
    ? t('settings.vocabulary_prompts_llm_status_configured', {
        defaultValue: 'LLM Service: Configured',
      })
    : t('settings.vocabulary_prompts_llm_status_unconfigured', {
        defaultValue: 'LLM Service: Not Configured',
      });

  const llmStatusTooltip = isLlmConfigured
    ? modelDisplayName
      ? t('settings.vocabulary_prompts_llm_tooltip_configured', {
          model: modelDisplayName,
          defaultValue: `Model configured for this feature (${modelDisplayName}). Click to open LLM Service settings.`,
        })
      : t('settings.vocabulary_prompts_llm_tooltip_configured_no_model', {
          defaultValue: 'LLM service is configured. Click to open LLM Service settings.',
        })
    : t('settings.vocabulary_prompts_llm_tooltip_unconfigured', {
        defaultValue:
          'LLM service is not configured for this feature. Click to open LLM Service settings.',
      });

  const sets = config.textReplacementSets || [];
  const hotwordSets = config.hotwordSets || [];

  return (
    <SettingsTabContainer id="settings-panel-vocabulary" ariaLabelledby="settings-tab-vocabulary">
      <SettingsPageHeader
        icon={<BookIcon width={28} height={28} />}
        title={t('settings.vocabulary', { defaultValue: 'Vocabulary & Language' })}
        description={t('settings.vocabulary_description', {
          defaultValue:
            'Manage custom vocabulary, hotwords, text replacements, context presets, and summary templates.',
        })}
      />

      <ScenarioCardTabs<VocabularySubTab>
        id="settings-vocab-categories"
        ariaLabel={t('settings.vocabulary_categories', { defaultValue: 'Vocabulary categories' })}
        columns={3}
        activeTab={activeSubTab}
        onChange={setActiveSubTab}
        idPrefix="settings-vocab-tab"
        getPanelId={(value) => `settings-vocab-panel-${value}`}
        bordered
        items={[
          {
            value: 'recognition',
            label: t('settings.vocabulary_tab_recognition', { defaultValue: 'Unified Dictionary' }),
            description: t('settings.vocabulary_tab_recognition_desc', {
              defaultValue: 'Global vocabulary, text replacements, and project terms',
            }),
            icon: <SpellCheck size={18} />,
          },
          {
            value: 'prompts',
            label: t('settings.vocabulary_tab_prompts', { defaultValue: 'AI Prompts & Templates' }),
            description: t('settings.vocabulary_tab_prompts_desc', {
              defaultValue: 'Polish keywords, context presets, and summary templates',
            }),
            icon: <Sparkles size={18} />,
          },
          {
            value: 'speakers',
            label: t('settings.vocabulary_tab_speakers', { defaultValue: 'Speaker Profiles' }),
            description: t('settings.vocabulary_tab_speakers_desc', {
              defaultValue: 'Local voice sample profiles and references',
            }),
            icon: <Users size={18} />,
          },
        ]}
      />

      {activeSubTab === 'recognition' && (
        <div
          id="settings-vocab-panel-recognition"
          role="tabpanel"
          aria-labelledby="settings-vocab-tab-recognition"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-xl, 32px)',
            animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
          }}
        >
          <UnifiedDictionarySection
            content={config.dictionaryContent}
            onUpdateContent={(newContent) => updateConfig({ dictionaryContent: newContent })}
            hotwordSets={hotwordSets}
            textReplacementSets={sets}
            onUpdateHotwordSets={(nextSets) => updateConfig({ hotwordSets: nextSets })}
            onUpdateTextReplacementSets={(nextSets) =>
              updateConfig({ textReplacementSets: nextSets })
            }
          />
        </div>
      )}

      {activeSubTab === 'prompts' && (
        <div
          id="settings-vocab-panel-prompts"
          role="tabpanel"
          aria-labelledby="settings-vocab-tab-prompts"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-xl, 32px)',
            animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
          }}
        >
          <div className="settings-vocab-prompts-header">
            <SegmentedSubTabs<PromptsSubTab>
              id="settings-vocab-prompts-subtabs"
              ariaLabel={t('settings.vocabulary_prompts_categories', {
                defaultValue: 'AI prompt and template categories',
              })}
              activeTab={activePromptsSubTab}
              onChange={setActivePromptsSubTab}
              idPrefix="settings-prompts-tab"
              getPanelId={(value) => `settings-prompts-panel-${value}`}
              items={[
                {
                  value: 'polish',
                  label: t('settings.vocabulary_prompts_tab_polish', { defaultValue: 'Polish' }),
                  tooltip: t('settings.vocabulary_prompts_tab_polish_desc', {
                    defaultValue: 'Auto polish and style presets',
                  }),
                  icon: <Sparkles size={15} />,
                },
                {
                  value: 'translation',
                  label: t('settings.vocabulary_prompts_tab_translation', {
                    defaultValue: 'Translation',
                  }),
                  tooltip: t('settings.vocabulary_prompts_tab_translation_desc', {
                    defaultValue: 'Auto translation and default target language',
                  }),
                  icon: <Languages size={15} />,
                },
                {
                  value: 'summary',
                  label: t('settings.vocabulary_prompts_tab_summary', { defaultValue: 'Summary' }),
                  tooltip: t('settings.vocabulary_prompts_tab_summary_desc', {
                    defaultValue: 'Auto summary and custom templates',
                  }),
                  icon: <SummaryIcon width={15} height={15} />,
                },
              ]}
            />

            <button
              id="settings-vocab-llm-status-btn"
              type="button"
              className={`settings-vocab-llm-btn ${isLlmConfigured ? 'configured' : 'unconfigured'}`}
              onClick={() => navContext?.navigateToTab('llm_service')}
              data-tooltip={llmStatusTooltip}
              data-tooltip-pos="top"
              aria-label={`${llmStatusLabel}. ${t('settings.vocabulary_prompts_llm_jump_aria', { defaultValue: 'Jump to LLM Service settings' })}`}
            >
              <Bot size={15} className="settings-vocab-llm-icon" aria-hidden="true" />
              <span
                className={`settings-vocab-llm-dot ${isLlmConfigured ? 'configured' : 'unconfigured'}`}
                aria-hidden="true"
              />
              <span className="settings-vocab-llm-label">{llmStatusLabel}</span>
              {isLlmConfigured && modelDisplayName && (
                <span className="settings-vocab-llm-model-badge">{modelDisplayName}</span>
              )}
              <ArrowRight size={13} className="settings-vocab-llm-arrow" aria-hidden="true" />
            </button>
          </div>

          {activePromptsSubTab === 'polish' && (
            <div
              id="settings-prompts-panel-polish"
              role="tabpanel"
              aria-labelledby="settings-prompts-tab-polish"
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 'var(--spacing-xl, 32px)',
                animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
              }}
            >
              <SettingsContextSection />
            </div>
          )}

          {activePromptsSubTab === 'translation' && (
            <div
              id="settings-prompts-panel-translation"
              role="tabpanel"
              aria-labelledby="settings-prompts-tab-translation"
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 'var(--spacing-xl, 32px)',
                animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
              }}
            >
              <SettingsTranslationSection />
            </div>
          )}

          {activePromptsSubTab === 'summary' && (
            <div
              id="settings-prompts-panel-summary"
              role="tabpanel"
              aria-labelledby="settings-prompts-tab-summary"
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 'var(--spacing-xl, 32px)',
                animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
              }}
            >
              <SettingsSummaryTemplateSection />
            </div>
          )}
        </div>
      )}

      {activeSubTab === 'speakers' && (
        <div
          id="settings-vocab-panel-speakers"
          role="tabpanel"
          aria-labelledby="settings-vocab-tab-speakers"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-xl, 32px)',
            animation: 'fadeIn var(--transition-normal, 0.2s) ease-in-out',
          }}
        >
          <SettingsSpeakerProfilesSection />
        </div>
      )}
    </SettingsTabContainer>
  );
}

export default SettingsVocabularyTab;
