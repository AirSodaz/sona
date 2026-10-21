import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { asrConfigService } from '../services/asrConfigService';
import { modelService } from '../services/modelService';
import { useConfigStore } from '../stores/configStore';
import type { AsrSelectionSlot } from '../types/config';
import type { ModelCatalogSnapshot } from '../types/modelCatalog';
import { buildAsrModelDropdownOptions, resolveActiveAsrModelId } from '../utils/asrModelOptions';
import { buildLanguagePickerOptions, type LanguagePickerOption } from '../utils/languages';
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
}: ParameterSettingsPopoverProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();
  const appLocale = i18n?.language ?? 'en';
  const popoverRef = useRef<HTMLDivElement>(null);

  // Get config and setters from store
  const config = useConfigStore((state) => state.config);
  const setConfig = useConfigStore((state) => state.setConfig);

  // Derived values
  const enableTimeline = config.enableTimeline ?? false;
  const language = config.language;
  const slot: AsrSelectionSlot = surface === 'batch' ? 'batch' : 'live';

  // Catalog snapshot and loading state
  const [catalog, setCatalog] = useState<ModelCatalogSnapshot | null>(
    () => modelService.latestCatalogSnapshot
  );
  const [isLoadingCatalog, setIsLoadingCatalog] = useState(false);

  useEffect(() => {
    if (!isOpen) return;

    let isSubscribed = true;
    setIsLoadingCatalog(true);

    modelService
      .getModelCatalogSnapshot()
      .then((snapshot) => {
        if (isSubscribed) {
          setCatalog(snapshot);
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
    };
  }, [isOpen]);

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
      setConfig({ language: coerced });
    }
  }, [isOpen, disabled, config, slot, language, setConfig]);

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
