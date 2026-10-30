import { SlidersHorizontal } from 'lucide-react';
import type React from 'react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DEFAULT_EMBY_TOOL_OVERRIDES,
  loadEmbyToolOverrides,
  saveEmbyToolOverrides,
} from '../../../../services/emby/embySettingsService';
import { modelService } from '../../../../services/modelService';
import { ONLINE_ASR_PROVIDER_DEFINITIONS } from '../../../../services/onlineAsrProviders';
import { useConfigStore } from '../../../../stores/configStore';
import type { EmbySubtitleMode } from '../../../../types/batchQueue';
import type { AsrModelSelection } from '../../../../types/config';
import type { EmbyToolOverrides } from '../../../../types/emby';
import type { ModelCatalogSnapshot } from '../../../../types/modelCatalog';
import { Dropdown, type DropdownOption } from '../../../Dropdown';
import { Modal } from '../../../Modal';

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
  const { t } = useTranslation();

  const recognitionLanguageOptions: DropdownOption[] = useMemo(
    () => [
      {
        value: 'auto',
        label: t('emby.asr_lang_auto', { defaultValue: 'auto (Auto-detect from audio)' }),
      },
      { value: 'zh-CN', label: 'zh-CN (简体中文)' },
      { value: 'zh-TW', label: 'zh-TW (繁體中文)' },
      { value: 'en', label: 'en (English)' },
      { value: 'ja', label: 'ja (日本語)' },
      { value: 'ko', label: 'ko (한국어)' },
      { value: 'es', label: 'es (Español)' },
      { value: 'fr', label: 'fr (Français)' },
      { value: 'de', label: 'de (Deutsch)' },
    ],
    [t]
  );
  const config = useConfigStore((state) => state.config);

  const [selectedAsrValue, setSelectedAsrValue] = useState<string>('__global__');
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
      const overrides = loadEmbyToolOverrides();

      if (!overrides.asrSelection) {
        setSelectedAsrValue('__global__');
      } else if (overrides.asrSelection.engine === 'online') {
        const pid = overrides.asrSelection.providerId || ONLINE_ASR_PROVIDER_DEFINITIONS[0].id;
        setSelectedAsrValue(pid);
      } else {
        setSelectedAsrValue(overrides.asrSelection.modelId || '__global__');
      }

      setSelectedAsrLanguage(overrides.asrLanguage ?? 'auto');
      setSelectedTranslationModelId(overrides.translationModelId || '__global__');
      setSelectedSubtitleMode(overrides.defaultSubtitleMode ?? 'bilingual');
      setOverwriteExistingExternal(overrides.overwriteExistingExternal ?? true);
    }
  }, [isOpen]);

  const asrOptions: DropdownOption[] = useMemo(() => {
    const list: DropdownOption[] = [
      {
        value: '__global__',
        label: t('emby.global_default', { defaultValue: 'Global Default (Follow Settings)' }),
      },
    ];

    // Local models from catalog
    const localOptions = (catalog?.selectionOptions?.batch ?? []).filter(
      (opt) => opt.isInstalled || opt.id === selectedAsrValue
    );
    for (const opt of localOptions) {
      list.push({
        value: opt.id,
        label: `${opt.label} (Local)`,
      });
    }

    // Configured online providers
    for (const provider of ONLINE_ASR_PROVIDER_DEFINITIONS) {
      const providerConfig = config.asr?.providers?.online?.[provider.id];
      const isConfigured = provider.isConfigured(
        (providerConfig ?? provider.defaultConfig) as typeof provider.defaultConfig,
        'batch'
      );
      if (isConfigured) {
        list.push({
          value: provider.id,
          label: `${provider.defaultConfig?.providerId || provider.id} (Cloud)`,
        });
      }
    }

    return list;
  }, [catalog, config.asr, selectedAsrValue, t]);

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
      const isOnline = ONLINE_ASR_PROVIDER_DEFINITIONS.some(
        (p) => selectedAsrValue === p.id || selectedAsrValue.startsWith(`${p.id}::`)
      );
      if (isOnline) {
        const [providerId, rawModelId] = selectedAsrValue.includes('::')
          ? selectedAsrValue.split('::')
          : [selectedAsrValue, undefined];
        const existingOverrides = loadEmbyToolOverrides();
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
        const modelPath = catalog?.modelPathById?.[selectedAsrValue] || '';
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
    setSelectedAsrLanguage(DEFAULT_EMBY_TOOL_OVERRIDES.asrLanguage ?? 'auto');
    setSelectedTranslationModelId('__global__');
    setSelectedSubtitleMode(DEFAULT_EMBY_TOOL_OVERRIDES.defaultSubtitleMode ?? 'bilingual');
    setOverwriteExistingExternal(DEFAULT_EMBY_TOOL_OVERRIDES.overwriteExistingExternal ?? true);
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('emby.processing_settings_title', {
        defaultValue: 'Emby Processing Settings',
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
        {/* ASR Section */}
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

          <label className="emby-form-field">
            <span className="emby-form-label">
              {t('emby.asr_model_selection', { defaultValue: 'ASR Model Selection' })}
            </span>
            <Dropdown
              options={asrOptions}
              value={selectedAsrValue}
              onChange={setSelectedAsrValue}
              aria-label={t('emby.asr_model_selection', { defaultValue: 'ASR Model Selection' })}
            />
          </label>

          <label className="emby-form-field">
            <span className="emby-form-label">
              {t('emby.asr_recognition_language', { defaultValue: 'Recognition Language' })}
            </span>
            <Dropdown
              options={recognitionLanguageOptions}
              value={selectedAsrLanguage}
              onChange={setSelectedAsrLanguage}
              aria-label={t('emby.asr_recognition_language', {
                defaultValue: 'Recognition Language',
              })}
            />
          </label>
        </div>

        {/* Translation Section */}
        <div
          className="emby-settings-section"
          style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
        >
          <h4 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>
            {t('emby.section_translation', { defaultValue: 'Translation Engine' })}
          </h4>

          <label className="emby-form-field">
            <span className="emby-form-label">
              {t('emby.translation_model_selection', { defaultValue: 'Translation Model' })}
            </span>
            <Dropdown
              options={translationOptions}
              value={selectedTranslationModelId}
              onChange={setSelectedTranslationModelId}
              aria-label={t('emby.translation_model_selection', {
                defaultValue: 'Translation Model',
              })}
            />
          </label>
        </div>

        {/* Subtitle Preferences Section */}
        <div
          className="emby-settings-section"
          style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
        >
          <h4 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>
            {t('emby.section_subtitles', { defaultValue: 'Subtitle Preferences' })}
          </h4>

          <label className="emby-form-field">
            <span className="emby-form-label">
              {t('emby.default_subtitle_mode', { defaultValue: 'Default Subtitle Mode' })}
            </span>
            <Dropdown
              options={subtitleModeOptions}
              value={selectedSubtitleMode}
              onChange={(val) => setSelectedSubtitleMode(val as EmbySubtitleMode)}
              aria-label={t('emby.default_subtitle_mode', {
                defaultValue: 'Default Subtitle Mode',
              })}
            />
          </label>

          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              cursor: 'pointer',
              fontSize: 13,
              marginTop: 4,
            }}
          >
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
