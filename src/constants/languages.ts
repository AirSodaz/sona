export const LANGUAGE_OPTIONS = ['zh', 'en', 'ja', 'ko', 'fr', 'de', 'es'] as const;

export type LanguageOption = typeof LANGUAGE_OPTIONS[number];
