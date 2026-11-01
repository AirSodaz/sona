import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { asrConfigService, syncOnlineAsrProviderConfig } from '../services/asrConfigService';
import { modelService } from '../services/modelService';
import {
  getOnlineAsrProviderDefinition,
  getOnlineProviderConfig,
  providerSupportsSpeakerDiarization,
} from '../services/onlineAsrProviders';
import { speakerService } from '../services/speakerService';
import { useConfigStore } from '../stores/configStore';
import type { AsrSelectionSlot } from '../types/config';
import type { ModelCatalogSnapshot } from '../types/modelCatalog';
import { buildAsrModelDropdownOptions, resolveActiveAsrModelId } from '../utils/asrModelOptions';
import { buildLanguagePickerOptions, type LanguagePickerOption } from '../utils/languages';
import { buildScenarioLanguagePatch, getScenarioLanguage } from '../utils/scenarioModels';
import { Dropdown } from './Dropdown';
import { FormField } from './FormField';
import { Switch } from './Switch';
export interface ParameterSettingsPopoverProps {
  isOpen: boolean;
  onClose: () => void;
  /** Which transcription surface the settings apply to; decides the active model. */
  surface: 'live' | 'batch';
  disabled?: boolean;
  triggerRef: React.RefObject<HTMLButtonElement | null>;
  catalog?: ModelCatalogSnapshot | null;
  onCatalogLoaded?: (catalog: ModelCatalogSnapshot) => void;
}

/**
 * Dropup Popover for configuring transcription parameters (Subtitle Mode, Language).
 * Expands above the footer parameter settings button.
 */
export function ParameterSettingsPopover({
  isOpen,
  onClose,
  surface,
  disabled = false,
  triggerRef,
  catalog: externalCatalog,
  onCatalogLoaded,
}: ParameterSettingsPopoverProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();
  const appLocale = i18n?.language ?? 'en';
  const popoverRef = useRef<HTMLDivElement>(null);

  // Get config and setters from store
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);

  // Derived values
  const enableTimeline = config.enableTimeline ?? false;
  const language = getScenarioLanguage(config, surface);
  const slot: AsrSelectionSlot = surface === 'batch' ? 'batch' : 'live';

  // Catalog snapshot and loading state
  const [internalCatalog, setInternalCatalog] = useState<ModelCatalogSnapshot | null>(
    () => externalCatalog ?? modelService.latestCatalogSnapshot
  );
  const [isLoadingCatalog, setIsLoadingCatalog] = useState(false);

  const catalog = externalCatalog ?? internalCatalog;
  const catalogRef = useRef(catalog);

  useEffect(() => {
    catalogRef.current = catalog;
  }, [catalog]);

  useEffect(() => {
    if (externalCatalog) {
      setInternalCatalog(externalCatalog);
    }
  }, [externalCatalog]);

  useEffect(() => {
    if (!isOpen) return;

    let isSubscribed = true;
    if (!catalogRef.current) {
      setIsLoadingCatalog(true);
    }

    modelService
      .getModelCatalogSnapshot()
      .then((snapshot) => {
        if (isSubscribed && snapshot) {
          setInternalCatalog(snapshot);
          onCatalogLoaded?.(snapshot);
        }
      })
      .catch(() => {
        // Fall back to existing cached snapshot on error
      })
      .finally(() => {
        if (isSubscribed) {
          setIsLoadingCatalog(false);
        }
      });

    return () => {
      isSubscribed = false;
      setIsLoadingCatalog(false);
    };
  }, [isOpen, onCatalogLoaded]);

  const activeModelId = useMemo(
    () => resolveActiveAsrModelId(config, slot, catalog),
    [config, slot, catalog]
  );

  const modelOptions = useMemo(
    () =>
      catalog
        ? buildAsrModelDropdownOptions({
            slot,
            snapshot: catalog,
            selectedModelId: activeModelId,
            modelConfig: config,
            t,
          })
        : [],
    [catalog, slot, activeModelId, config, t]
  );

  const handleModelChange = useCallback(
    (newModelId: string) => {
      if (disabled || !catalog) return;
      const patch = asrConfigService.buildModelSelectionPatch(config, slot, newModelId, catalog);
      if (Object.keys(patch).length > 0) {
        setConfig(patch);
      }
    },
    [disabled, catalog, config, slot, setConfig]
  );

  const capability = useMemo(
    () => (isOpen ? asrConfigService.resolveActiveLanguageCapability(config, slot) : null),
    [isOpen, config, slot]
  );

  const options = useMemo<LanguagePickerOption[]>(
    () =>
      buildLanguagePickerOptions(capability, appLocale).map((option) =>
        option.group === 'common'
          ? { ...option, group: t('languages.common', { defaultValue: 'Common' }) }
          : option
      ),
    [capability, appLocale, t]
  );

  // Persisted selections can go stale when the active model changes; align
  // them silently with what the model actually accepts.
  useEffect(() => {
    if (!isOpen || disabled) {
      return;
    }
    const coerced = asrConfigService.coerceConfiguredLanguage(config, slot, language);
    if (coerced !== language) {
      setConfig(buildScenarioLanguagePatch(surface, coerced));
    }
  }, [isOpen, disabled, config, slot, surface, language, setConfig]);
  const activeSelection = useMemo(
    () =>
      isOpen
        ? asrConfigService.getSelection(
            { ...config, asr: asrConfigService.normalizeAsrConfig(config) },
            slot
          )
        : null,
    [isOpen, config, slot]
  );
  const isOnline = activeSelection?.engine === 'online';
  const onlineProviderId = activeSelection?.providerId;
  const isCloudSpeakerSupported = Boolean(
    onlineProviderId && providerSupportsSpeakerDiarization(onlineProviderId)
  );

  const isCloudSpeakerEnabled = useMemo(() => {
    if (!onlineProviderId) return false;
    const providerConfig = getOnlineProviderConfig(config.asr?.providers, onlineProviderId);
    const def = getOnlineAsrProviderDefinition(onlineProviderId);
    return (
      (providerConfig?.speakerDiarization as boolean | undefined) ??
      def?.defaultConfig?.speakerDiarization !== false
    );
  }, [onlineProviderId, config.asr?.providers]);

  const handleCloudSpeakerToggle = useCallback(
    (enabled: boolean) => {
      if (!onlineProviderId || disabled) return;
      setConfig(
        syncOnlineAsrProviderConfig(config, onlineProviderId, {
          speakerDiarization: enabled,
        })
      );
    },
    [onlineProviderId, disabled, config, setConfig]
  );

  const isLocalSpeakerEnabled =
    surface === 'batch'
      ? Boolean(config.batchSpeakerDiarizationEnabled)
      : Boolean(config.liveSpeakerDiarizationEnabled);
  const hasLocalModels = speakerService.isConfigured(config, slot);

  let isSpeakerDisabled = disabled;
  let isSpeakerChecked = false;
  let speakerHint = t('batch.speaker_diarization_hint', {
    defaultValue: 'Distinguish speakers and identify voices',
  });
  let onSpeakerToggle: (checked: boolean) => void = (checked: boolean) => {
    if (disabled) return;
    setConfig(
      surface === 'batch'
        ? { batchSpeakerDiarizationEnabled: checked }
        : { liveSpeakerDiarizationEnabled: checked }
    );
  };

  if (isOnline) {
    if (!isCloudSpeakerSupported) {
      isSpeakerDisabled = true;
      isSpeakerChecked = false;
      speakerHint = t('batch.cloud_speaker_unsupported', {
        defaultValue: 'Current cloud model does not support speaker diarization',
      });
    } else {
      isSpeakerDisabled = disabled;
      isSpeakerChecked = isCloudSpeakerEnabled;
      speakerHint = t('batch.speaker_diarization_hint', {
        defaultValue: 'Distinguish speakers and identify voices',
      });
      onSpeakerToggle = handleCloudSpeakerToggle;
    }
  } else {
    isSpeakerDisabled = disabled;
    isSpeakerChecked = isLocalSpeakerEnabled;
    if (isLocalSpeakerEnabled && !hasLocalModels) {
      speakerHint = t('batch.speaker_unconfigured_hint', {
        defaultValue: 'No speaker model configured; will fall back to normal transcription',
      });
    } else {
      speakerHint = t('batch.speaker_diarization_hint', {
        defaultValue: 'Distinguish speakers and identify voices',
      });
    }
  }

  // Click outside and Esc key handling
  useEffect(() => {
    if (!isOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;

      if (popoverRef.current?.contains(target)) {
        return;
      }

      if (triggerRef.current?.contains(target)) {
        return;
      }

      const targetElement =
        event.target instanceof Element
          ? event.target
          : (event.target as Node | null)?.parentElement;

      if (targetElement?.closest('.dropdown-menu')) {
        return;
      }

      onClose();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        const targetElement =
          event.target instanceof Element
            ? event.target
            : (event.target as Node | null)?.parentElement;

        if (targetElement?.closest('.dropdown-menu')) {
          return;
        }

        event.preventDefault();
        onClose();
        triggerRef.current?.focus();
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, onClose, triggerRef]);

  if (!isOpen) return null;

  const locksLanguage = capability?.languageMode === 'auto' || capability?.languageMode === 'fixed';
  const languageHintKey =
    capability?.languageMode === 'fixed'
      ? 'batch.language_hint_fixed'
      : 'batch.language_hint_auto_detect';

  const dropdownStyle = {
    width: '150px',
    opacity: disabled ? 0.6 : 1,
    pointerEvents: disabled ? 'none' : 'auto',
  } as React.CSSProperties;

  return (
    <div
      ref={popoverRef}
      id="parameter-settings-popover"
      role="region"
      aria-label={t('common.parameter_settings', { defaultValue: 'Parameter Settings' })}
      className="parameter-settings-popover"
    >
      <FormField
        id="parameter-settings-model"
        label={t('batch.model', { defaultValue: 'Model' })}
        description={t('batch.model_hint', { defaultValue: 'Select transcription model' })}
        layout="horizontal"
      >
        <Dropdown
          id="parameter-settings-model"
          value={activeModelId}
          onChange={handleModelChange}
          options={modelOptions}
          placeholder={
            isLoadingCatalog && modelOptions.length === 0
              ? t('common.loading', { defaultValue: 'Loading...' })
              : modelOptions.length === 0
                ? t('batch.no_model_available', { defaultValue: 'No models available' })
                : t('batch.select_model', { defaultValue: 'Select model...' })
          }
          disabled={disabled || (modelOptions.length === 0 && !isLoadingCatalog)}
          style={dropdownStyle}
        />
      </FormField>

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
      <FormField
        id="parameter-settings-speaker"
        label={t('batch.speaker_diarization', { defaultValue: 'Speaker Diarization' })}
        description={
          <span
            className={
              !isOnline && isLocalSpeakerEnabled && !hasLocalModels
                ? 'parameter-settings-warning-text'
                : undefined
            }
          >
            {speakerHint}
          </span>
        }
        layout="horizontal"
      >
        <Switch
          id="parameter-settings-speaker"
          checked={isSpeakerChecked}
          onChange={onSpeakerToggle}
          disabled={isSpeakerDisabled}
        />
      </FormField>

      <FormField
        id="parameter-settings-language"
        label={t('batch.language')}
        description={locksLanguage ? t(languageHintKey) : t('batch.language_hint')}
        layout="horizontal"
      >
        <Dropdown
          id="parameter-settings-language"
          value={language}
          onChange={(val) => !disabled && setConfig(buildScenarioLanguagePatch(surface, val))}
          options={options.map((option) => ({
            value: option.value,
            label: option.label,
            group: option.group,
            ariaLabel: option.value === 'auto' ? undefined : `${option.label} (${option.value})`,
          }))}
          style={
            locksLanguage
              ? { ...dropdownStyle, pointerEvents: 'none', opacity: 0.75 }
              : dropdownStyle
          }
        />
      </FormField>
    </div>
  );
}
