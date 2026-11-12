import { Languages } from 'lucide-react';
import type React from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { LANGUAGE_OPTIONS } from '../../constants/languages';
import { useLlmAssistantConfig, useSetConfig } from '../../stores/configStore';
import { getLocalizedLanguageName } from '../../utils/languageUtils';
import { Dropdown } from '../Dropdown';
import { SettingsItem, SettingsSection } from './SettingsLayout';

export function SettingsTranslationSection(): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const config = useLlmAssistantConfig();
  const updateConfig = useSetConfig();

  const languageOptions = useMemo(
    () =>
      LANGUAGE_OPTIONS.map((lang) => ({
        value: lang.code,
        label: getLocalizedLanguageName(lang.code, i18n?.language || 'zh'),
      })),
    [i18n?.language]
  );

  return (
    <SettingsSection
      id="settings-vocabulary-translation"
      title={t('settings.translation_default_language_title', {
        defaultValue: 'Default Target Language',
      })}
      description={t('settings.translation_default_language_hint', {
        defaultValue: 'Default target language used when initiating transcript translation.',
      })}
      icon={<Languages size={20} />}
    >
      <SettingsItem
        title={t('settings.translation_default_language_title', {
          defaultValue: 'Default Target Language',
        })}
        hint={t('settings.translation_default_language_hint', {
          defaultValue: 'Default target language used when initiating transcript translation.',
        })}
      >
        <div style={{ width: '280px', maxWidth: '100%' }}>
          <Dropdown
            value={config.translationLanguage || 'zh'}
            onChange={(val) => updateConfig({ translationLanguage: val })}
            options={languageOptions}
          />
        </div>
      </SettingsItem>
    </SettingsSection>
  );
}
