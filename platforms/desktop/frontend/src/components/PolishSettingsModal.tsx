import type React from 'react';
import { useTranslation } from 'react-i18next';
import { isFeatureLlmConfigComplete } from '../services/llm/configUtils';
import { useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { coercePolishPresetId, getPolishPresetOptions } from '../utils/polishPresets';
import { Dropdown } from './Dropdown';
import { Modal } from './Modal';
import { Switch } from './Switch';

interface PolishSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * Modal for configuring advanced polish settings.
 */
export function PolishSettingsModal({
  isOpen,
  onClose,
}: PolishSettingsModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const globalConfig = useConfigStore((state) => state.config);
  const config = useEffectiveConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);

  const autoPolish = globalConfig.autoPolish ?? false;
  const autoPolishFrequency = globalConfig.autoPolishFrequency ?? 5;
  const isLlmConfigured = isFeatureLlmConfigComplete(config, 'polish');

  if (!isOpen) return null;

  const presetOptions = getPolishPresetOptions(globalConfig.polishCustomPresets, t);

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={t('polish.advanced_settings')} size="md">
      <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-lg)' }}>
        {/* Auto Polish */}
        <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between' }}>
          <div
            style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)', flex: 1 }}
          >
            <span
              style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}
            >
              {t('batch.auto_polish', { defaultValue: 'Auto-Polish' })}
            </span>
            <span
              style={{ fontSize: '0.75rem', color: 'var(--color-text-secondary)', lineHeight: 1.4 }}
            >
              {isLlmConfigured
                ? t('batch.auto_polish_hint', {
                    defaultValue: 'Automatically polish text with LLM',
                  })
                : t('polish.error_config_missing', {
                    defaultValue: 'Please configure LLM service first',
                  })}
            </span>
          </div>
          <Switch
            checked={autoPolish}
            onChange={(val) => isLlmConfigured && setConfig({ autoPolish: val })}
            disabled={!isLlmConfigured}
          />
        </div>

        {/* Auto Polish Frequency */}
        {autoPolish && (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <label
              htmlFor="auto-polish-frequency"
              style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}
            >
              {t('batch.auto_polish_frequency', { defaultValue: 'Auto-Polish Frequency' })}
            </label>
            <input
              id="auto-polish-frequency"
              type="number"
              min={1}
              max={100}
              value={autoPolishFrequency}
              onChange={(e) => {
                const val = parseInt(e.target.value, 10);
                if (!Number.isNaN(val) && val > 0) {
                  setConfig({ autoPolishFrequency: val });
                }
              }}
              style={{
                width: '80px',
                padding: '6px 10px',
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--color-border)',
                backgroundColor: 'var(--color-bg-input)',
                color: 'var(--color-text-primary)',
                fontSize: '0.875rem',
                outline: 'none',
              }}
            />
          </div>
        )}

        <div
          style={{
            height: '1px',
            background: 'var(--color-border)',
            opacity: 0.5,
            margin: '4px 0',
          }}
        />

        {/* Preset */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
          <label
            style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}
          >
            {t('polish.mode_label', { defaultValue: 'Polish Mode' })}
          </label>
          <Dropdown
            value={coercePolishPresetId(config.polishPresetId, globalConfig.polishCustomPresets)}
            onChange={(val) => setConfig({ polishPresetId: val })}
            options={presetOptions}
            style={{ width: '100%' }}
          />
        </div>
      </div>
    </Modal>
  );
}
