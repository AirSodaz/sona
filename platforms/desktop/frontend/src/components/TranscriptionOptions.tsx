import type React from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { asrConfigService } from '../services/asrConfigService';
import { modelService } from '../services/modelService';
import { useConfigStore } from '../stores/configStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import type { AsrSelectionSlot } from '../types/config';
import type { ModelCatalogSnapshot } from '../types/modelCatalog';
import { buildAsrModelDropdownOptions, resolveActiveAsrModelId } from '../utils/asrModelOptions';
import { languageDisplayName } from '../utils/languages';
import { ChevronUpIcon, SlidersIcon } from './Icons';
import { ParameterSettingsPopover } from './ParameterSettingsPopover';

/** Props for TranscriptionOptions component. */
interface TranscriptionOptionsProps {
  className?: string;
  disabled?: boolean;
  /** Which surface's active model governs the language picker inside the popover. */
  surface?: 'live' | 'batch';
}

/**
 * Component that renders a capsule button to toggle the transcription parameter preset popover.
 * Configures Active Model, Subtitle/Timeline Mode, and Language.
 * @param props Component props.
 * @return The rendered component.
 */
export function TranscriptionOptions({
  className = '',
  disabled: customDisabled,
  surface: customSurface,
}: TranscriptionOptionsProps): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const appLocale = i18n?.language ?? 'en';
  const [isExpanded, setIsExpanded] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const storeMode = useTranscriptRuntimeStore((state) => state.mode);
  const storeIsRecording = useTranscriptRuntimeStore((state) => state.isRecording);

  const surface = customSurface ?? (storeMode === 'batch' ? 'batch' : 'live');
  const disabled = customDisabled ?? (surface === 'live' && storeIsRecording);
  const slot: AsrSelectionSlot = surface === 'batch' ? 'batch' : 'live';

  const config = useConfigStore((state) => state.config);

  const [catalog, setCatalog] = useState<ModelCatalogSnapshot | null>(
    () => modelService.latestCatalogSnapshot
  );

  useEffect(() => {
    let isSubscribed = true;
    modelService
      .getModelCatalogSnapshot()
      .then((snapshot) => {
        if (isSubscribed && snapshot) {
          setCatalog(snapshot);
        }
      })
      .catch(() => {});

    return () => {
      isSubscribed = false;
    };
  }, []);

  useEffect(() => {
    if (disabled && isExpanded) {
      setIsExpanded(false);
    }
  }, [disabled, isExpanded]);

  const activeModelId = useMemo(
    () => resolveActiveAsrModelId(config, slot, catalog),
    [config, slot, catalog]
  );

  const activeModelLabel = useMemo(() => {
    if (catalog) {
      const options = buildAsrModelDropdownOptions({
        slot,
        snapshot: catalog,
        selectedModelId: activeModelId,
        modelConfig: config,
        t,
      });
      const match = options.find((opt) => opt.value === activeModelId);
      if (match?.ariaLabel) {
        return match.ariaLabel;
      }
    }

    if (!activeModelId) {
      return t('batch.select_model', { defaultValue: 'Model' });
    }

    if (activeModelId.includes('::')) {
      const submodel = activeModelId.split('::')[1];
      return submodel || activeModelId;
    }

    return activeModelId;
  }, [catalog, slot, activeModelId, config, t]);

  const effectiveLanguage = useMemo(() => {
    return asrConfigService.coerceConfiguredLanguage(config, slot, config.language);
  }, [config, slot]);

  const activeLanguageLabel = useMemo(() => {
    if (!effectiveLanguage || effectiveLanguage === 'auto') {
      if (appLocale.startsWith('zh')) return '自动检测';
      if (appLocale.startsWith('ja')) return '自動';
      if (appLocale.startsWith('ko')) return '자동';
      return 'Auto';
    }
    return languageDisplayName(effectiveLanguage, appLocale);
  }, [effectiveLanguage, appLocale]);

  const presetTitle = t('common.transcription_preset', {
    defaultValue: 'Preset',
  });

  const buttonAriaLabel = `${presetTitle}: ${activeModelLabel}, ${activeLanguageLabel}`;

  return (
    <div className={`options-container ${className}`}>
      <button
        ref={triggerRef}
        type="button"
        className={`btn btn-parameter-settings ${isExpanded ? 'active' : ''}`}
        onClick={() => !disabled && setIsExpanded((prev) => !prev)}
        disabled={disabled}
        aria-label={buttonAriaLabel}
        aria-expanded={isExpanded}
        aria-haspopup="true"
        aria-controls="parameter-settings-popover"
      >
        <span className="parameter-settings-capsule-main">
          <SlidersIcon className="parameter-settings-icon" width={14} height={14} />
          <span className="parameter-settings-label">{presetTitle}</span>
        </span>

        <span className="parameter-settings-capsule-summary">
          <span className="parameter-settings-summary-model">{activeModelLabel}</span>
          <span className="parameter-settings-summary-dot" aria-hidden="true">
            ·
          </span>
          <span className="parameter-settings-summary-lang">{activeLanguageLabel}</span>
        </span>

        <ChevronUpIcon
          className={`parameter-settings-chevron ${isExpanded ? 'open' : ''}`}
          width={14}
          height={14}
          aria-hidden="true"
        />
      </button>

      <ParameterSettingsPopover
        isOpen={isExpanded}
        onClose={() => setIsExpanded(false)}
        surface={surface}
        disabled={disabled}
        triggerRef={triggerRef}
        catalog={catalog}
        onCatalogLoaded={setCatalog}
      />
    </div>
  );
}
