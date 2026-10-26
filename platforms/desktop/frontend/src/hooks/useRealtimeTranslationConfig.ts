import { useCallback, useEffect, useMemo, useState } from 'react';
import { modelService } from '../services/modelService';
import { useConfigStore, useSetConfig } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { logger } from '../utils/logger';

export interface LanguageOption {
  value: string;
  label: string;
}

export const SOURCE_LANGUAGES: LanguageOption[] = [
  { value: 'zh', label: '中文 (Chinese)' },
  { value: 'en', label: 'English' },
  { value: 'ja', label: '日本語 (Japanese)' },
];

/**
 * Shared hook managing realtime translation configuration, language pairs,
 * model installation checks, and toggling logic across Subtitle Settings and Tools.
 */
export function useRealtimeTranslationConfig() {
  const updateConfig = useSetConfig();

  const realtimeTranslationEnabled = useConfigStore(
    (state) => state.config.realtimeTranslationEnabled ?? false
  );
  const realtimeTranslationSourceLanguage = useConfigStore(
    (state) => state.config.realtimeTranslationSourceLanguage || 'en'
  );
  const realtimeTranslationTargetLanguage = useConfigStore(
    (state) => state.config.realtimeTranslationTargetLanguage || 'zh'
  );
  const captionDisplayMode = useConfigStore(
    (state) => state.config.captionDisplayMode || 'bilingual'
  );

  const sourceLanguages = SOURCE_LANGUAGES;

  const targetLanguages = useMemo<LanguageOption[]>(() => {
    if (realtimeTranslationSourceLanguage === 'zh') {
      return [
        { value: 'en', label: 'English' },
        { value: 'ja', label: '日本語 (Japanese)' },
      ];
    }
    return [{ value: 'zh', label: '中文 (Chinese)' }];
  }, [realtimeTranslationSourceLanguage]);

  const targetLanguageValue = useMemo(() => {
    return targetLanguages.some((opt) => opt.value === realtimeTranslationTargetLanguage)
      ? realtimeTranslationTargetLanguage
      : targetLanguages[0].value;
  }, [targetLanguages, realtimeTranslationTargetLanguage]);

  const modelId = `opus-mt-${realtimeTranslationSourceLanguage}-${realtimeTranslationTargetLanguage}`;
  const [isModelInstalled, setIsModelInstalled] = useState(false);

  useEffect(() => {
    let active = true;
    if (typeof modelService?.isModelInstalled === 'function') {
      modelService
        .isModelInstalled(modelId)
        .then((installed) => {
          if (active) setIsModelInstalled(installed);
        })
        .catch((err) => {
          logger.warn('Failed to query model install status for', modelId, err);
          if (active) setIsModelInstalled(false);
        });
    }
    return () => {
      active = false;
    };
  }, [modelId]);

  const handleSourceLanguageChange = useCallback(
    (val: string) => {
      const updates: Record<string, unknown> = { realtimeTranslationSourceLanguage: val };
      if (val === 'en' || val === 'ja') {
        updates.realtimeTranslationTargetLanguage = 'zh';
      } else if (val === 'zh' && realtimeTranslationTargetLanguage === 'zh') {
        updates.realtimeTranslationTargetLanguage = 'en';
      }
      updateConfig(updates);
    },
    [realtimeTranslationTargetLanguage, updateConfig]
  );

  const handleTargetLanguageChange = useCallback(
    (val: string) => {
      updateConfig({ realtimeTranslationTargetLanguage: val });
    },
    [updateConfig]
  );

  const handleSwapLanguages = useCallback(() => {
    const nextSrc = realtimeTranslationTargetLanguage;
    let nextTgt = realtimeTranslationSourceLanguage;
    if (nextSrc === 'en' || nextSrc === 'ja') {
      nextTgt = 'zh';
    } else if (nextSrc === 'zh' && nextTgt === 'zh') {
      nextTgt = 'en';
    }
    updateConfig({
      realtimeTranslationSourceLanguage: nextSrc,
      realtimeTranslationTargetLanguage: nextTgt,
    });
  }, [realtimeTranslationSourceLanguage, realtimeTranslationTargetLanguage, updateConfig]);

  const handleToggleRealtimeTranslation = useCallback(
    async (enabled: boolean) => {
      if (!enabled) {
        if (captionDisplayMode === 'translation') {
          updateConfig({
            realtimeTranslationEnabled: false,
            captionDisplayMode: 'bilingual',
          });
        } else {
          updateConfig({ realtimeTranslationEnabled: false });
        }
        return;
      }

      let installed = false;
      try {
        if (typeof modelService?.isModelInstalled === 'function') {
          installed = await modelService.isModelInstalled(modelId);
        } else {
          installed = true;
        }
      } catch (err) {
        logger.warn('Failed to check translation model installation status; allowing toggle', err);
        installed = true;
      }

      setIsModelInstalled(installed);

      if (!installed) {
        await useDialogStore.getState().showError({
          code: 'model.not_installed',
          messageKey: 'settings.translation_model_not_installed',
        });
        return;
      }

      updateConfig({ realtimeTranslationEnabled: true });
    },
    [captionDisplayMode, modelId, updateConfig]
  );

  return {
    realtimeTranslationEnabled,
    realtimeTranslationSourceLanguage,
    realtimeTranslationTargetLanguage,
    captionDisplayMode,
    sourceLanguages,
    targetLanguages,
    targetLanguageValue,
    modelId,
    isModelInstalled,
    handleSourceLanguageChange,
    handleTargetLanguageChange,
    handleSwapLanguages,
    handleToggleRealtimeTranslation,
  };
}
