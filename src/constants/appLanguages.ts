import type { AppLanguagePreference, ResolvedAppLanguage } from '../types/config';

export interface AppLanguageOption {
  value: AppLanguagePreference;
  labelKey: string;
  defaultLabel: string;
}

export const SUPPORTED_APP_LANGUAGES: readonly ResolvedAppLanguage[] = [
  'en',
  'zh',
  'zh-TW',
  'ja',
] as const;

export const APP_LANGUAGE_OPTIONS: readonly AppLanguageOption[] = [
  { value: 'auto', labelKey: 'common.auto', defaultLabel: 'Automatic' },
  { value: 'en', labelKey: 'settings.language_en', defaultLabel: 'English' },
  { value: 'zh', labelKey: 'settings.language_zh', defaultLabel: '简体中文' },
  { value: 'zh-TW', labelKey: 'settings.language_zh_tw', defaultLabel: '繁體中文' },
  { value: 'ja', labelKey: 'settings.language_ja', defaultLabel: '日本語' },
] as const;

function normalizeLanguageCode(language: string | null | undefined): string {
  return (language ?? '').trim().replace(/_/g, '-').toLowerCase();
}

export function resolveDetectedAppLanguage(
  detected: string | readonly string[] | null | undefined,
): ResolvedAppLanguage {
  const candidates = Array.isArray(detected) ? detected : [detected];

  for (const candidate of candidates) {
    const normalized = normalizeLanguageCode(candidate);
    if (!normalized) {
      continue;
    }

    if (normalized === 'ja' || normalized.startsWith('ja-')) {
      return 'ja';
    }

    if (normalized === 'en' || normalized.startsWith('en-')) {
      return 'en';
    }

    if (normalized === 'zh' || normalized.startsWith('zh-')) {
      if (
        normalized.includes('hant')
        || normalized.includes('-tw')
        || normalized.includes('-hk')
        || normalized.includes('-mo')
      ) {
        return 'zh-TW';
      }

      return 'zh';
    }
  }

  return 'en';
}

export function resolveAppLanguagePreference(
  preference: AppLanguagePreference | string | null | undefined,
  detected: string | readonly string[] | null | undefined,
): ResolvedAppLanguage {
  const normalized = normalizeLanguageCode(preference);

  if (!normalized || normalized === 'auto') {
    return resolveDetectedAppLanguage(detected);
  }

  return resolveDetectedAppLanguage(normalized);
}
