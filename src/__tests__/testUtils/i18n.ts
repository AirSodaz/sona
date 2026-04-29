type TranslationOptions = Record<string, unknown> | undefined;
type TranslationOverride = string | ((options?: TranslationOptions) => string);

export function createReactI18nextMock(
  overrides: Record<string, TranslationOverride> = {},
) {
  const t = (key: string, options?: TranslationOptions): string => {
    const override = overrides[key];

    if (typeof override === 'function') {
      return override(options);
    }

    if (typeof override === 'string') {
      return override;
    }

    if (key === 'batch.supports' && typeof options?.formats === 'string') {
      return `Supports: ${options.formats}`;
    }

    if (key === 'batch.queue_title' && typeof options?.count !== 'undefined') {
      return `Queue (${String(options.count)})`;
    }

    return key;
  };

  return {
    useTranslation: () => ({
      t,
      i18n: {
        changeLanguage: async () => undefined,
        language: 'en',
      },
    }),
    initReactI18next: {
      type: '3rdParty' as const,
      init: () => undefined,
    },
  };
}
