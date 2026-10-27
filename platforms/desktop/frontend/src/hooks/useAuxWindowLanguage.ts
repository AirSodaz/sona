import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { resolveAppLanguagePreference } from '../constants/appLanguages';
import { STORE_KEY_CONFIG, settingsStore } from '../services/storageService';
import type { AppConfig } from '../types/config';
import { logger } from '../utils/logger';

export function useAuxWindowLanguage(): void {
  const { i18n } = useTranslation();

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    const applyConfig = (config: AppConfig | null | undefined) => {
      if (disposed) {
        return;
      }

      const targetLang = resolveAppLanguagePreference(config?.appLanguage, navigator.language);
      if (i18n.language !== targetLang) {
        void i18n.changeLanguage(targetLang).catch((error) => {
          logger.warn('[useAuxWindowLanguage] Failed to change language', { error });
        });
      }
    };

    const setup = async () => {
      try {
        const storedConfig = await settingsStore.get<AppConfig>(STORE_KEY_CONFIG);
        if (disposed) {
          return;
        }
        applyConfig(storedConfig);
        const unlistenFn = await settingsStore.onKeyChange<AppConfig>(
          STORE_KEY_CONFIG,
          applyConfig
        );
        if (disposed) {
          unlistenFn();
          return;
        }
        unlisten = unlistenFn;
      } catch (error) {
        logger.warn('[useAuxWindowLanguage] Failed to synchronize language preference', {
          error,
        });
      }
    };

    void setup();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [i18n]);
}
