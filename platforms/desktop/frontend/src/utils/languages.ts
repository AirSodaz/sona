import type { LanguageMode } from '../types/modelCatalog';

/** Minimal contract satisfied by both local models and online ASR providers. */
export interface LanguageCapable {
  languages: string[];
  languageMode: LanguageMode;
}

/** Number of language badges rendered before the `+N` overflow chip. */
export const VISIBLE_LANGUAGE_TAGS = 4;

/** Maximum number of languages to list in the tooltip before adding a summary counter. */
export const MAX_TOOLTIP_LANGUAGES = 24;

const FALLBACK_NAMES: Record<string, string> = {
  af: 'Afrikaans', am: 'Amharic', ar: 'Arabic', as: 'Assamese', az: 'Azerbaijani',
  ba: 'Bashkir', be: 'Belarusian', bg: 'Bulgarian', bn: 'Bengali', bo: 'Tibetan',
  br: 'Breton', bs: 'Bosnian', ca: 'Catalan', cs: 'Czech', cy: 'Welsh',
  da: 'Danish', de: 'German', el: 'Greek', en: 'English', es: 'Spanish',
  et: 'Estonian', eu: 'Basque', fa: 'Persian', fi: 'Finnish', fil: 'Filipino',
  fo: 'Faroese', fr: 'French', gl: 'Galician', gu: 'Gujarati', ha: 'Hausa',
  haw: 'Hawaiian', he: 'Hebrew', hi: 'Hindi', hr: 'Croatian', ht: 'Haitian Creole',
  hu: 'Hungarian', hy: 'Armenian', id: 'Indonesian', is: 'Icelandic', it: 'Italian',
  ja: 'Japanese', jv: 'Javanese', jw: 'Javanese', ka: 'Georgian', kab: 'Kabyle',
  kk: 'Kazakh', km: 'Khmer', kn: 'Kannada', ko: 'Korean', ks: 'Kashmiri',
  ky: 'Kyrgyz', la: 'Latin', lb: 'Luxembourgish', ln: 'Lingala', lo: 'Lao',
  lt: 'Lithuanian', lv: 'Latvian', mg: 'Malagasy', mi: 'Maori', mk: 'Macedonian',
  ml: 'Malayalam', mn: 'Mongolian', mr: 'Marathi', ms: 'Malay', mt: 'Maltese',
  my: 'Myanmar', ne: 'Nepali', nl: 'Dutch', nn: 'Nynorsk', no: 'Norwegian',
  oc: 'Occitan', or: 'Odia', pa: 'Punjabi', pl: 'Polish', ps: 'Pashto',
  pt: 'Portuguese', ro: 'Romanian', ru: 'Russian', sa: 'Sanskrit', sd: 'Sindhi',
  si: 'Sinhala', sk: 'Slovak', sl: 'Slovenian', sn: 'Shona', so: 'Somali',
  sq: 'Albanian', sr: 'Serbian', su: 'Sundanese', sv: 'Swedish', sw: 'Swahili',
  ta: 'Tamil', te: 'Telugu', tg: 'Tajik', th: 'Thai', tk: 'Turkmen',
  tl: 'Tagalog', tr: 'Turkish', tt: 'Tatar', ug: 'Uyghur', uk: 'Ukrainian',
  ur: 'Urdu', uz: 'Uzbek', vi: 'Vietnamese', yi: 'Yiddish', yo: 'Yoruba',
  yue: 'Cantonese', zh: 'Chinese',
};

let displayNamesCache: { locale: string; formatter: Intl.DisplayNames } | null = null;

/**
 * Resolves a localized endonym/exonym for an ISO 639 code via CLDR, falling
 * back to English names when the runtime lacks full ICU data.
 */
export function languageDisplayName(code: string, locale: string): string {
  try {
    if (!displayNamesCache || displayNamesCache.locale !== locale) {
      displayNamesCache = {
        locale,
        formatter: new Intl.DisplayNames(locale, { type: 'language' }),
      };
    }
    const name = displayNamesCache.formatter.of(code);
    if (name && name !== code) {
      return name;
    }
  } catch {
    displayNamesCache = null;
  }
  return FALLBACK_NAMES[code] ?? code.toUpperCase();
}

/**
 * Formats a clean, aesthetic tooltip string for a model's supported languages.
 * Shows localized language names. If the list is very long (e.g. Whisper with ~100
 * languages or Meta Omnilingual with 1600+), it shows the first batch of languages
 * followed by a localized total count summary.
 */
export function formatLanguagesTooltip(
  languages: string[] | undefined,
  locale: string = 'zh',
  t?: (key: string, options?: Record<string, unknown>) => string,
): string {
  if (!languages || languages.length === 0) {
    return '';
  }

  const names = languages.map((code) => languageDisplayName(code, locale));

  if (languages.length <= MAX_TOOLTIP_LANGUAGES) {
    return names.join(', ');
  }

  const sample = names.slice(0, MAX_TOOLTIP_LANGUAGES).join(', ');
  if (t) {
    return t('languages.tooltip_summary', {
      sample,
      count: languages.length,
      defaultValue: `${sample} ... (${languages.length} languages in total)`,
    });
  }

  if (locale.startsWith('zh')) {
    return `${sample} 等共 ${languages.length} 种语言`;
  }
  if (locale.startsWith('ja')) {
    return `${sample} など合計 ${languages.length} 言語`;
  }
  if (locale.startsWith('ko')) {
    return `${sample} 등 총 ${languages.length}개 언어`;
  }
  return `${sample} ... (${languages.length} languages in total)`;
}

export function isLanguageCapableModel(value: unknown): value is LanguageCapable {
  return Boolean(
    value
    && typeof value === 'object'
    && Array.isArray((value as LanguageCapable).languages)
    && typeof (value as LanguageCapable).languageMode === 'string',
  );
}

export interface LanguagePickerOption {
  value: string;
  label: string;
  /** Optional grouping key; `common` sorts first in supporting dropdowns. */
  group?: string;
}

const COMMON_LANGUAGE_CODES = new Set(['zh', 'en', 'ja', 'ko', 'yue']);

/**
 * Builds the exact set of language options a client may offer for a model:
 * - selectable: auto + every supported language
 * - auto:       auto only (model detects language itself)
 * - fixed:      the model's single language only
 * - none/unknown: auto only, callers typically hide the picker instead
 */
export function buildLanguagePickerOptions(
  model: LanguageCapable | null | undefined,
  locale: string,
): LanguagePickerOption[] {
  if (!model) {
    return [{ value: 'auto', label: 'Auto' }];
  }

  const autoOption = (): LanguagePickerOption => ({
    value: 'auto',
    label: locale.startsWith('zh') ? '自动检测' : 'Auto detect',
  });

  switch (model.languageMode) {
    case 'fixed': {
      const [only] = model.languages;
      return only ? [{ value: only, label: languageDisplayName(only, locale) }] : [autoOption()];
    }
    case 'selectable': {
      const common: LanguagePickerOption[] = [];
      const rest: LanguagePickerOption[] = [];
      for (const code of model.languages) {
        const option = {
          value: code,
          label: languageDisplayName(code, locale),
          ...(COMMON_LANGUAGE_CODES.has(code) ? { group: 'common' as const } : {}),
        };
        (COMMON_LANGUAGE_CODES.has(code) ? common : rest).push(option);
      }
      return [autoOption(), ...common, ...rest];
    }
    case 'auto':
    case 'none':
    default:
      return [autoOption()];
  }
}

/**
 * Coerces a persisted language selection onto what the resolved model
 * actually accepts; protects against stale configs and cross-device sync.
 */
export function coerceLanguage(
  model: LanguageCapable | null | undefined,
  configured: string | null | undefined,
): string {
  const language = configured?.trim() || 'auto';
  if (!model) {
    return language;
  }

  // Legacy/custom entries without structured metadata pass through untouched.
  if (!Array.isArray(model.languages) || typeof model.languageMode !== 'string') {
    return language;
  }

  if (model.languageMode === 'none') {
    return 'auto';
  }
  if (model.languageMode === 'fixed') {
    return model.languages[0] ?? 'auto';
  }
  if (language === 'auto') {
    return 'auto';
  }
  if (model.languageMode === 'selectable') {
    const match = model.languages.find((code) => code.toLowerCase() === language.toLowerCase());
    if (match) {
      return match;
    }
  }
  return 'auto';
}
