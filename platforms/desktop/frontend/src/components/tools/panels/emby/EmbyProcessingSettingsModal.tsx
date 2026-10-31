import { SlidersHorizontal } from 'lucide-react';
import type React from 'react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { asrConfigService } from '../../../../services/asrConfigService';
import {
  DEFAULT_EMBY_TOOL_OVERRIDES,
  loadEmbyToolOverrides,
  loadEmbyToolOverridesAsync,
  saveEmbyToolOverrides,
} from '../../../../services/emby/embySettingsService';
import { modelService } from '../../../../services/modelService';
import { ONLINE_ASR_PROVIDER_DEFINITIONS } from '../../../../services/onlineAsrProviders';
import { useConfigStore } from '../../../../stores/configStore';
import type { EmbySubtitleMode } from '../../../../types/batchQueue';
import type { AppConfig, AsrModelSelection } from '../../../../types/config';
import type { EmbyToolOverrides } from '../../../../types/emby';
import type { ModelCatalogSnapshot } from '../../../../types/modelCatalog';
import { buildAsrModelDropdownOptions } from '../../../../utils/asrModelOptions';
import { buildLanguagePickerOptions, coerceLanguage } from '../../../../utils/languages';
import { Dropdown, type DropdownOption } from '../../../Dropdown';
import { FormField } from '../../../FormField';
import { Modal } from '../../../Modal';
import { Switch } from '../../../Switch';

export interface EmbyProcessingSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSaved?: (overrides: EmbyToolOverrides) => void;
}

export function EmbyProcessingSettingsModal({
  isOpen,
  onClose,
  onSaved,
}: EmbyProcessingSettingsModalProps): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const appLocale = i18n?.language ?? 'en';
  const config = useConfigStore((state) => state.config);

  const [selectedAsrValue, setSelectedAsrValue] = useState<string>('__global__');
  const [selectedEnableTimeline, setSelectedEnableTimeline] = useState<boolean>(true);
  const [selectedAsrLanguage, setSelectedAsrLanguage] = useState<string>('auto');
  const [selectedTranslationModelId, setSelectedTranslationModelId] =
    useState<string>('__global__');
  const [selectedSubtitleMode, setSelectedSubtitleMode] = useState<EmbySubtitleMode>('bilingual');
  const [overwriteExistingExternal, setOverwriteExistingExternal] = useState<boolean>(true);
  const [catalog, setCatalog] = useState<ModelCatalogSnapshot | null>(
    () => modelService.latestCatalogSnapshot
  );

  useEffect(() => {
    if (isOpen && !catalog) {
      void modelService
        .getModelCatalogSnapshot()
        .then(setCatalog)
        .catch(() => {});
    }
  }, [isOpen, catalog]);

  useEffect(() => {
    if (isOpen) {
      let isMounted = true;
      const applyOverrides = (overrides: EmbyToolOverrides) => {
        if (!overrides.asrSelection) {
          setSelectedAsrValue('__global__');
        } else if (overrides.asrSelection.engine === 'online') {
          const pid = overrides.asrSelection.providerId || ONLINE_ASR_PROVIDER_DEFINITIONS[0].id;
          setSelectedAsrValue(
            overrides.asrSelection.modelId ? `${pid}::${overrides.asrSelection.modelId}` : pid
          );
        } else {
          setSelectedAsrValue(overrides.asrSelection.modelId || '__global__');
        }

        setSelectedEnableTimeline(
          overrides.enableTimeline ?? DEFAULT_EMBY_TOOL_OVERRIDES.enableTimeline ?? true
        );
        setSelectedAsrLanguage(overrides.asrLanguage ?? 'auto');
        setSelectedTranslationModelId(overrides.translationModelId || '__global__');
        setSelectedSubtitleMode(overrides.defaultSubtitleMode ?? 'bilingual');
        setOverwriteExistingExternal(overrides.overwriteExistingExternal ?? true);
      };

      applyOverrides(loadEmbyToolOverrides());

      void loadEmbyToolOverridesAsync().then((fresh) => {
        if (isMounted && fresh) {
          applyOverrides(fresh);
        }
      });

      return () => {
        isMounted = false;
      };
    }
  }, [isOpen]);

  const asrOptions: DropdownOption[] = useMemo(() => {
    const list: DropdownOption[] = [
      {
        value: '__global__',
        label: t('emby.global_default', { defaultValue: 'Global Default (Follow Settings)' }),
      },
    ];

    if (catalog) {
      const modelOpts = buildAsrModelDropdownOptions({
        slot: 'batch',
        snapshot: catalog,
        selectedModelId: selectedAsrValue === '__global__' ? '' : selectedAsrValue,
        modelConfig: config,
        t,
      });
      list.push(...modelOpts);
    }

    return list;
  }, [catalog, config, selectedAsrValue, t]);

  const capability = useMemo(() => {
    if (!isOpen) return null;
    if (selectedAsrValue === '__global__') {
      return asrConfigService.resolveActiveLanguageCapability(config, 'batch');
    }
    const isOnline = ONLINE_ASR_PROVIDER_DEFINITIONS.some(
      (p) => selectedAsrValue === p.id || selectedAsrValue.startsWith(`${p.id}::`)
    );
    let asrSelection: AsrModelSelection;
    if (isOnline) {
      const [providerId, rawModelId] = selectedAsrValue.includes('::')
        ? selectedAsrValue.split('::')
        : [selectedAsrValue, undefined];
      asrSelection = {
        engine: 'online',
        mode: 'batch',
        providerId: providerId as AsrModelSelection['providerId'],
        modelId: rawModelId,
        modelPath: '',
      };
    } else {
      const modelPath = catalog?.modelPathById?.[selectedAsrValue] || '';
      asrSelection = {
        engine: 'local',
        mode: 'batch',
        modelId: selectedAsrValue,
        modelPath,
      };
    }
    const tempConfig: AppConfig = {
      ...config,
      asr: {
        ...config.asr,
        selections: {
          ...config.asr?.selections,
          batch: asrSelection,
        },
      },
    };
    return asrConfigService.resolveActiveLanguageCapability(tempConfig, 'batch');
  }, [isOpen, selectedAsrValue, config, catalog]);

  const languageOptions = useMemo<DropdownOption[]>(() => {
    return buildLanguagePickerOptions(capability, appLocale).map((option) => ({
      value: option.value,
      label: option.label,
      group:
        option.group === 'common'
          ? t('languages.common', { defaultValue: 'Common' })
          : option.group,
      ariaLabel: option.value === 'auto' ? undefined : `${option.label} (${option.value})`,
    }));
  }, [capability, appLocale, t]);

  const locksLanguage = capability?.languageMode === 'auto' || capability?.languageMode === 'fixed';
  const languageHintKey =
    capability?.languageMode === 'fixed'
      ? 'batch.language_hint_fixed'
      : 'batch.language_hint_auto_detect';

  useEffect(() => {
    if (!isOpen) return;
    if (capability && selectedAsrLanguage !== 'auto') {
      const coerced = coerceLanguage(capability, selectedAsrLanguage);
      if (coerced !== selectedAsrLanguage) {
        setSelectedAsrLanguage(coerced);
      }
    }
  }, [isOpen, capability, selectedAsrLanguage]);

  const translationOptions: DropdownOption[] = useMemo(() => {
    const list: DropdownOption[] = [
      {
        value: '__global__',
        label: t('emby.global_default', { defaultValue: 'Global Default (Follow Settings)' }),
      },
    ];

    const rawModels = config.llmSettings?.models;
    const llmModels = Array.isArray(rawModels)
      ? rawModels
      : rawModels && typeof rawModels === 'object'
        ? Object.values(rawModels)
        : [];
    for (const m of llmModels) {
      if (m?.id) {
        list.push({
          value: m.id,
          label: `${m.metadata?.displayName || m.model} (${m.provider})`,
        });
      }
    }

    return list;
  }, [config.llmSettings?.models, t]);

  const subtitleModeOptions: DropdownOption[] = useMemo(
    () => [
      {
        value: 'bilingual',
        label: t('emby.mode_bilingual', { defaultValue: 'Bilingual Subtitles' }),
      },
      {
        value: 'pure_translation',
        label: t('emby.mode_pure', { defaultValue: 'Translation Only' }),
      },
      {
        value: 'separate_tracks',
        label: t('emby.mode_separate', {
          defaultValue: 'Separate Original & Translation Tracks',
        }),
      },
    ],
    [t]
  );

  const handleSave = () => {
    let asrSelection: AsrModelSelection | null = null;
    if (selectedAsrValue !== '__global__') {
      const existingOverrides = loadEmbyToolOverrides();
      const isOnline = ONLINE_ASR_PROVIDER_DEFINITIONS.some(
        (p) => selectedAsrValue === p.id || selectedAsrValue.startsWith(`${p.id}::`)
      );
      if (isOnline) {
        const [providerId, rawModelId] = selectedAsrValue.includes('::')
          ? selectedAsrValue.split('::')
          : [selectedAsrValue, undefined];
        const modelId =
          rawModelId ??
          (existingOverrides.asrSelection?.engine === 'online' &&
          existingOverrides.asrSelection.providerId === providerId
            ? existingOverrides.asrSelection.modelId
            : undefined);
        asrSelection = {
          engine: 'online',
          mode: 'batch',
          providerId: providerId as AsrModelSelection['providerId'],
          modelId,
          modelPath: '',
        };
      } else {
        const modelPath =
          catalog?.modelPathById?.[selectedAsrValue] ||
          (existingOverrides.asrSelection?.engine === 'local' &&
          existingOverrides.asrSelection.modelId === selectedAsrValue
            ? existingOverrides.asrSelection.modelPath
            : '');
        asrSelection = {
          engine: 'local',
          mode: 'batch',
          modelId: selectedAsrValue,
          modelPath,
        };
      }
    }

    const newOverrides: EmbyToolOverrides = {
      asrSelection,
      asrLanguage: selectedAsrLanguage,
      enableTimeline: selectedEnableTimeline,
      translationModelId:
        selectedTranslationModelId === '__global__' ? null : selectedTranslationModelId,
      defaultSubtitleMode: selectedSubtitleMode,
      overwriteExistingExternal,
    };

    saveEmbyToolOverrides(newOverrides);
    onSaved?.(newOverrides);
    onClose();
  };

  const handleResetDefaults = () => {
    setSelectedAsrValue('__global__');
    setSelectedEnableTimeline(DEFAULT_EMBY_TOOL_OVERRIDES.enableTimeline ?? true);
    setSelectedAsrLanguage(DEFAULT_EMBY_TOOL_OVERRIDES.asrLanguage ?? 'auto');
    setSelectedTranslationModelId('__global__');
    setSelectedSubtitleMode(DEFAULT_EMBY_TOOL_OVERRIDES.defaultSubtitleMode ?? 'bilingual');
    setOverwriteExistingExternal(DEFAULT_EMBY_TOOL_OVERRIDES.overwriteExistingExternal ?? true);
  };

  const dropdownControlStyle = {
    minWidth: 200,
    maxWidth: 260,
  } as React.CSSProperties;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('emby.processing_settings_title', {
        defaultValue: 'Media Processing Settings',
      })}
      size="md"
      footer={
        <div
          className="emby-modal-footer-actions"
          style={{ justifyContent: 'space-between', width: '100%' }}
        >
          <button type="button" className="btn btn-secondary" onClick={handleResetDefaults}>
            {t('common.restore_default', { defaultValue: 'Restore Default' })}
          </button>
          <div style={{ display: 'flex', gap: 8 }}>
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button type="button" className="btn btn-primary" onClick={handleSave}>
              {t('common.save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      }
    >
      <div
        className="emby-config-modal-body"
        style={{ display: 'flex', flexDirection: 'column', gap: 20 }}
      >
        {/* ASR Section - 3 items matching Create Page bottom-left */}
        <div
          className="emby-settings-section"
          style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
        >
          <h4
            style={{
              margin: 0,
              fontSize: 14,
              fontWeight: 600,
              display: 'flex',
              alignItems: 'center',
              gap: 6,
            }}
          >
            <SlidersHorizontal size={14} />
            {t('emby.section_asr', { defaultValue: 'Speech Recognition (ASR)' })}
          </h4>

          {/* 1. Model Selection */}
          <FormField
            id="emby-settings-model"
            label={t('batch.model', { defaultValue: 'Model' })}
            description={t('batch.model_hint', { defaultValue: 'Select transcription model' })}
            layout="horizontal"
          >
            <Dropdown
              id="emby-settings-model"
              options={asrOptions}
              value={selectedAsrValue}
              onChange={setSelectedAsrValue}
              aria-label={t('batch.model', { defaultValue: 'Model' })}
              style={dropdownControlStyle}
            />
          </FormField>

          {/* 2. Subtitle Mode (Timeline Mode) */}
          <FormField
            id="emby-settings-timeline"
            label={t('batch.timeline_mode', { defaultValue: 'Subtitle Mode' })}
            description={t('batch.timeline_hint', {
              defaultValue: 'Split into smaller units',
            })}
            layout="horizontal"
          >
            <Switch
              id="emby-settings-timeline"
              checked={selectedEnableTimeline}
              onChange={setSelectedEnableTimeline}
            />
          </FormField>

          {/* 3. Recognition Language */}
          <FormField
            id="emby-settings-language"
            label={t('emby.asr_recognition_language', { defaultValue: 'Recognition Language' })}
            description={
              locksLanguage
                ? t(languageHintKey)
                : t('batch.language_hint', {
                    defaultValue: 'Select audio language',
                  })
            }
            layout="horizontal"
          >
            <Dropdown
              id="emby-settings-language"
              options={languageOptions}
              value={selectedAsrLanguage}
              onChange={setSelectedAsrLanguage}
              aria-label={t('emby.asr_recognition_language', {
                defaultValue: 'Recognition Language',
              })}
              style={
                locksLanguage
                  ? { ...dropdownControlStyle, pointerEvents: 'none', opacity: 0.75 }
                  : dropdownControlStyle
              }
            />
          </FormField>
        </div>

        {/* Translation Section */}
        <div
          className="emby-settings-section"
          style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
        >
          <h4 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>
            {t('emby.section_translation', { defaultValue: 'Translation Engine' })}
          </h4>

          <FormField
            id="emby-settings-translation"
            label={t('emby.translation_model_selection', { defaultValue: 'Translation Model' })}
            description={t('emby.translation_model_hint', {
              defaultValue: 'Used for bilingual or translated subtitle generation',
            })}
            layout="horizontal"
          >
            <Dropdown
              id="emby-settings-translation"
              options={translationOptions}
              value={selectedTranslationModelId}
              onChange={setSelectedTranslationModelId}
              aria-label={t('emby.translation_model_selection', {
                defaultValue: 'Translation Model',
              })}
              style={dropdownControlStyle}
            />
          </FormField>
        </div>

        {/* Subtitle Preferences Section */}
        <div
          className="emby-settings-section"
          style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
        >
          <h4 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>
            {t('emby.section_subtitles', { defaultValue: 'Subtitle Preferences' })}
          </h4>

          <FormField
            id="emby-settings-default-mode"
            label={t('emby.default_subtitle_mode', { defaultValue: 'Default Subtitle Mode' })}
            description={t('emby.default_subtitle_mode_hint', {
              defaultValue: 'Default subtitle format when adding media tasks',
            })}
            layout="horizontal"
          >
            <Dropdown
              id="emby-settings-default-mode"
              options={subtitleModeOptions}
              value={selectedSubtitleMode}
              onChange={(val) => setSelectedSubtitleMode(val as EmbySubtitleMode)}
              aria-label={t('emby.default_subtitle_mode', {
                defaultValue: 'Default Subtitle Mode',
              })}
              style={dropdownControlStyle}
            />
          </FormField>

          <label className="emby-checkbox-field" style={{ marginTop: 4 }}>
            <input
              type="checkbox"
              checked={overwriteExistingExternal}
              onChange={(e) => setOverwriteExistingExternal(e.target.checked)}
            />
            <span>
              {t('emby.overwrite_external', {
                defaultValue: 'Overwrite existing external subtitle of same language',
              })}
            </span>
          </label>
        </div>
      </div>
    </Modal>
  );
}
