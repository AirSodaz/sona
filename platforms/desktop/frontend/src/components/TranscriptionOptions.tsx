import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { ChevronUpIcon, SettingsIcon } from './Icons';
import { ParameterSettingsPopover } from './ParameterSettingsPopover';

/** Props for TranscriptionOptions component. */
interface TranscriptionOptionsProps {
  className?: string;
  disabled?: boolean;
  /** Which surface's active model governs the language picker inside the popover. */
  surface?: 'live' | 'batch';
}

/**
 * Component that renders a button to toggle the parameter settings dropup popover.
 * Configures Subtitle Mode and Language.
 * @param props Component props.
 * @return The rendered component.
 */
export function TranscriptionOptions({
  className = '',
  disabled: customDisabled,
  surface: customSurface,
}: TranscriptionOptionsProps): React.JSX.Element {
  const { t } = useTranslation();
  const [isExpanded, setIsExpanded] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const storeMode = useTranscriptRuntimeStore((state) => state.mode);
  const storeIsRecording = useTranscriptRuntimeStore((state) => state.isRecording);

  const surface = customSurface ?? (storeMode === 'batch' ? 'batch' : 'live');
  const disabled = customDisabled ?? (surface === 'live' && storeIsRecording);

  useEffect(() => {
    if (disabled && isExpanded) {
      setIsExpanded(false);
    }
  }, [disabled, isExpanded]);

  return (
    <div className={`options-container ${className}`}>
      <button
        ref={triggerRef}
        type="button"
        className={`btn btn-parameter-settings ${isExpanded ? 'active' : ''}`}
        onClick={() => !disabled && setIsExpanded((prev) => !prev)}
        disabled={disabled}
        aria-label={t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}
        aria-expanded={isExpanded}
        aria-haspopup="true"
        aria-controls="parameter-settings-popover"
      >
        <SettingsIcon width={16} height={16} />
        <span>{t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}</span>
        <ChevronUpIcon
          className={`parameter-settings-chevron ${isExpanded ? 'open' : ''}`}
          width={14}
          height={14}
        />
      </button>

      <ParameterSettingsPopover
        isOpen={isExpanded}
        onClose={() => setIsExpanded(false)}
        surface={surface}
        disabled={disabled}
        triggerRef={triggerRef}
      />
    </div>
  );
}
