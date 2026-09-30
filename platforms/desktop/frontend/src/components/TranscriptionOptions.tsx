import type React from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { SettingsIcon } from './Icons';
import { ParameterSettingsModal } from './ParameterSettingsModal';

/** Props for TranscriptionOptions component. */
interface TranscriptionOptionsProps {
  className?: string;
  disabled?: boolean;
  /** Which surface's active model governs the language picker inside the modal. */
  surface?: 'live' | 'batch';
}

/**
 * Component that renders a button to open parameter settings modal.
 * Replaces the inline controls for Subtitle Mode and Language.
 *
 * @param props Component props.
 * @return The rendered component.
 */
export function TranscriptionOptions({
  className = '',
  disabled: customDisabled,
  surface: customSurface,
}: TranscriptionOptionsProps): React.JSX.Element {
  const { t } = useTranslation();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const storeMode = useTranscriptRuntimeStore((state) => state.mode);
  const storeIsRecording = useTranscriptRuntimeStore((state) => state.isRecording);

  const surface = customSurface ?? (storeMode === 'batch' ? 'batch' : 'live');
  const disabled = customDisabled ?? (surface === 'live' && storeIsRecording);

  return (
    <div className={`options-container ${className}`}>
      <button
        type="button"
        className="btn btn-parameter-settings"
        onClick={() => setIsModalOpen(true)}
        disabled={disabled}
        aria-label={t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}
      >
        <SettingsIcon width={16} height={16} />
        <span>{t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}</span>
      </button>

      <ParameterSettingsModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        surface={surface}
        disabled={disabled}
      />
    </div>
  );
}
