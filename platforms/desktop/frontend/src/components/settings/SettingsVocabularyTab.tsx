import { Languages, Sparkles, SpellCheck, Users } from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSetConfig, useVocabularyConfig } from '../../stores/configStore';
import { BookIcon, SummaryIcon } from '../Icons';
import { SettingsContextSection } from './SettingsContextSection';
import { SettingsPageHeader, SettingsTabContainer } from './SettingsLayout';
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

  const handleTabKeyDown = (e: React.KeyboardEvent) => {
    const tabs: VocabularySubTab[] = ['recognition', 'prompts', 'speakers'];
    const currentIndex = tabs.indexOf(activeSubTab);
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      const nextTab = tabs[(currentIndex + 1) % tabs.length];
      setActiveSubTab(nextTab);
      document.getElementById(`settings-vocab-tab-${nextTab}`)?.focus();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      const prevTab = tabs[(currentIndex - 1 + tabs.length) % tabs.length];
      setActiveSubTab(prevTab);
      document.getElementById(`settings-vocab-tab-${prevTab}`)?.focus();
    }
  };

  const handlePromptsTabKeyDown = (e: React.KeyboardEvent) => {
    const tabs: PromptsSubTab[] = ['polish', 'translation', 'summary'];
    const currentIndex = tabs.indexOf(activePromptsSubTab);
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      const nextTab = tabs[(currentIndex + 1) % tabs.length];
      setActivePromptsSubTab(nextTab);
      document.getElementById(`settings-prompts-tab-${nextTab}`)?.focus();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      const prevTab = tabs[(currentIndex - 1 + tabs.length) % tabs.length];
      setActivePromptsSubTab(prevTab);
      document.getElementById(`settings-prompts-tab-${prevTab}`)?.focus();
    }
  };

  const { t } = useTranslation();
  const config = useVocabularyConfig();
  const updateConfig = useSetConfig();

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

      <div
        id="settings-vocab-categories"
        className="settings-scenario-cards three-columns"
        role="tablist"
        aria-label={t('settings.vocabulary_categories', { defaultValue: 'Vocabulary categories' })}
        onKeyDown={handleTabKeyDown}
        style={{
          border: '1px solid var(--color-border)',
          borderRadius: 'var(--radius-lg, 16px)',
          background: 'var(--color-bg-primary)',
          boxShadow: 'var(--shadow-xs, 0 1px 2px rgba(0, 0, 0, 0.04))',
        }}
      >
        {[
          {
            value: 'recognition' as const,
            label: t('settings.vocabulary_tab_recognition', { defaultValue: 'Unified Dictionary' }),
            description: t('settings.vocabulary_tab_recognition_desc', {
              defaultValue: 'Global vocabulary, text replacements, and project terms',
            }),
            icon: <SpellCheck size={18} />,
          },
          {
            value: 'prompts' as const,
            label: t('settings.vocabulary_tab_prompts', { defaultValue: 'AI Prompts & Templates' }),
            description: t('settings.vocabulary_tab_prompts_desc', {
              defaultValue: 'Polish keywords, context presets, and summary templates',
            }),
            icon: <Sparkles size={18} />,
          },
          {
            value: 'speakers' as const,
            label: t('settings.vocabulary_tab_speakers', { defaultValue: 'Speaker Profiles' }),
            description: t('settings.vocabulary_tab_speakers_desc', {
              defaultValue: 'Local voice sample profiles and references',
            }),
            icon: <Users size={18} />,
          },
        ].map((tab) => (
          <button
            id={`settings-vocab-tab-${tab.value}`}
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={activeSubTab === tab.value}
            aria-controls={`settings-vocab-panel-${tab.value}`}
            aria-label={tab.label}
            tabIndex={activeSubTab === tab.value ? 0 : -1}
            className={`settings-scenario-card${activeSubTab === tab.value ? ' active' : ''}`}
            onClick={() => setActiveSubTab(tab.value)}
          >
            <span className="settings-scenario-card-icon">{tab.icon}</span>
            <span className="settings-scenario-card-text">
              <span className="settings-scenario-card-label">{tab.label}</span>
              <span className="settings-scenario-card-description">{tab.description}</span>
            </span>
          </button>
        ))}
      </div>

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
          <div
            id="settings-vocab-prompts-subtabs"
            className="settings-subtab-nav"
            role="tablist"
            aria-label={t('settings.vocabulary_prompts_categories', {
              defaultValue: 'AI prompt and template categories',
            })}
            onKeyDown={handlePromptsTabKeyDown}
          >
            {[
              {
                value: 'polish' as const,
                label: t('settings.vocabulary_prompts_tab_polish', { defaultValue: 'Polish' }),
                description: t('settings.vocabulary_prompts_tab_polish_desc', {
                  defaultValue: 'Auto polish and style presets',
                }),
                icon: <Sparkles size={15} />,
              },
              {
                value: 'translation' as const,
                label: t('settings.vocabulary_prompts_tab_translation', {
                  defaultValue: 'Translation',
                }),
                description: t('settings.vocabulary_prompts_tab_translation_desc', {
                  defaultValue: 'Auto translation and default target language',
                }),
                icon: <Languages size={15} />,
              },
              {
                value: 'summary' as const,
                label: t('settings.vocabulary_prompts_tab_summary', { defaultValue: 'Summary' }),
                description: t('settings.vocabulary_prompts_tab_summary_desc', {
                  defaultValue: 'Auto summary and custom templates',
                }),
                icon: <SummaryIcon width={15} height={15} />,
              },
            ].map((tab) => (
              <button
                id={`settings-prompts-tab-${tab.value}`}
                key={tab.value}
                type="button"
                role="tab"
                aria-selected={activePromptsSubTab === tab.value}
                aria-controls={`settings-prompts-panel-${tab.value}`}
                aria-label={tab.label}
                title={tab.description}
                tabIndex={activePromptsSubTab === tab.value ? 0 : -1}
                className={`settings-subtab-btn${activePromptsSubTab === tab.value ? ' active' : ''}`}
                onClick={() => setActivePromptsSubTab(tab.value)}
              >
                <span className="settings-subtab-icon">{tab.icon}</span>
                <span className="settings-subtab-label">{tab.label}</span>
              </button>
            ))}
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
