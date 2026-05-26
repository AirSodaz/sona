import React from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from './Dropdown';
import { Switch } from './Switch';
import { useConfigStore } from '../stores/configStore';
import { Modal } from './Modal';
import { FormField } from './FormField';

interface ParameterSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  disabled?: boolean;
}

/**
 * Modal for configuring transcription parameters (Subtitle Mode, Language, Auto-Polish).
 */
export function ParameterSettingsModal({
  isOpen,
  onClose,
  disabled = false,
}: ParameterSettingsModalProps): React.JSX.Element | null {
  const { t } = useTranslation();

  // Get config and setters from store
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);

  // Derived values
  const enableTimeline = config.enableTimeline ?? false;
  const language = config.language;

  if (!isOpen) return null;

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
          description={t('batch.language_hint')}
          layout="horizontal"
        >
          <Dropdown
            id="parameter-settings-language"
            value={language}
            onChange={(val) => !disabled && setConfig({ language: val })}
            options={[
              { value: 'auto', label: 'Auto' },
              { value: 'zh', label: 'Chinese' },
              { value: 'en', label: 'English' },
              { value: 'ja', label: 'Japanese' },
              { value: 'ko', label: 'Korean' },
              { value: 'yue', label: 'Cantonese' },
            ]}
            style={dropdownStyle}
          />
        </FormField>
      </div>
    </Modal>
  );
}
