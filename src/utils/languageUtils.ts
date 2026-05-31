const displayNamesCache = new Map<string, Intl.DisplayNames>();

export function getLocalizedLanguageName(langCode: string, locale: string = 'en'): string {
  const normLocale = locale.startsWith('zh') ? 'zh' : 'en'; // Standardize sub-locales

  let displayNames = displayNamesCache.get(normLocale);
  if (!displayNames) {
    try {
      displayNames = new Intl.DisplayNames([normLocale], { type: 'language', fallback: 'none' });
      displayNamesCache.set(normLocale, displayNames);
    } catch {
      // Graceful fallback
    }
  }

  // Static custom overrides for preferred i18n names in Sona
  const fallbacks: Record<string, Record<string, string>> = {
    'zh': { 'zh': '中文 (简体)', 'en': 'Chinese (Simplified)' },
    'zh-TW': { 'zh': '中文 (繁体)', 'en': 'Chinese (Traditional)' }
  };

  if (fallbacks[langCode]?.[normLocale]) {
    return fallbacks[langCode][normLocale];
  }

  if (displayNames) {
    try {
      const name = displayNames.of(langCode);
      if (name) {
        return name.charAt(0).toUpperCase() + name.slice(1);
      }
    } catch {
      // Fallback to raw code if displayNames.of throws
    }
  }

  return fallbacks[langCode]?.['en'] || langCode;
}
