import React, { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';
import { Switch } from './Switch';
import { useConfigStore } from '../stores/configStore';
import { Modal } from './Modal';
import { FormField } from './FormField';
import { asrConfigService } from '../services/asrConfigService';
import type { AsrSelectionSlot } from '../types/config';
import {
  buildLanguagePickerOptions,
  type LanguagePickerOption,
} from '../utils/languages';

interface ParameterSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Which transcription surface the settings apply to; decides the active model. */
  surface: 'live' | 'batch';
  disabled?: boolean;
}

/**
 * Modal for configuring transcription parameters (Subtitle Mode, Language, Auto-Polish).
 *
 * The language picker is derived from the surface's active model:
 * - selectable models offer auto plus every supported language
 * - auto-detect-only models lock the picker to auto
 * - single-language models lock the picker to that language
 */
export function ParameterSettingsModal({
  isOpen,
  onClose,
  surface,
  disabled = false,
}: ParameterSettingsModalProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();
  const appLocale = i18n?.language ?? 'en';

  // Get config and setters from store
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);

  // Derived values
  const enableTimeline = config.enableTimeline ?? false;
  const language = config.language;
  const slot: AsrSelectionSlot = surface === 'batch' ? 'batch' : 'live';

  const capability = useMemo(
    () => (isOpen ? asrConfigService.resolveActiveLanguageCapability(config, slot) : null),
    [isOpen, config, slot],
  );

  const options = useMemo<LanguagePickerOption[]>(
    () => buildLanguagePickerOptions(capability, appLocale).map((option) => (
      option.group === 'common'
        ? { ...option, group: t('languages.common', { defaultValue: 'Common' }) }
        : option
    )),
    [capability, appLocale, t],
  );

  // Persisted selections can go stale when the active model changes; align
  // them silently with what the model actually accepts.
  useEffect(() => {
    if (!isOpen || disabled) {
      return;
    }
    const coerced = asrConfigService.coerceConfiguredLanguage(config, slot, language);
    if (coerced !== language) {
      setConfig({ language: coerced });
    }
  }, [isOpen, disabled, config, slot, language, setConfig]);

  if (!isOpen) return null;

  const locksLanguage = capability?.languageMode === 'auto' || capability?.languageMode === 'fixed';
  const languageHintKey = capability?.languageMode === 'fixed'
    ? 'batch.language_hint_fixed'
    : 'batch.language_hint_auto_detect';

  const dropdownStyle = {
    width: '180px',
    opacity: disabled ? 0.6 : 1,
    pointerEvents: disabled ? 'none' : 'auto',
  } as React.CSSProperties;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}
      size="md"
    >
      {/* Content */}
      <div
        className="options-container"
        style={{
          padding: 0,
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--spacing-lg)',
        }}
      >
        {/* Subtitle Mode */}
        <FormField
          id="parameter-settings-timeline"
          label={t('batch.timeline_mode')}
          description={t('batch.timeline_hint')}
          layout="horizontal"
        >
          <Switch
            id="parameter-settings-timeline"
            checked={enableTimeline}
            onChange={(val) => !disabled && setConfig({ enableTimeline: val })}
            disabled={disabled}
          />
        </FormField>

        {/* Language */}
        <FormField
          id="parameter-settings-language"
          label={t('batch.language')}
          description={locksLanguage ? t(languageHintKey) : t('batch.language_hint')}
          layout="horizontal"
        >
          <Dropdown
            id="parameter-settings-language"
            value={language}
            onChange={(val) => !disabled && setConfig({ language: val })}
            options={options.map((option) => ({
              value: option.value,
              label: option.label,
              group: option.group,
              ariaLabel: option.value === 'auto'
                ? undefined
                : `${option.label} (${option.value})`,
            }))}
            style={locksLanguage ? { ...dropdownStyle, pointerEvents: 'none', opacity: 0.75 } : dropdownStyle}
          />
        </FormField>
      </div>
    </Modal>
  );
}
