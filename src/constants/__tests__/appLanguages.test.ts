import { describe, expect, it } from 'vitest';
import {
  APP_LANGUAGE_OPTIONS,
  resolveAppLanguagePreference,
  resolveDetectedAppLanguage,
} from '../appLanguages';

describe('app language support', () => {
  it('exposes the interface language preferences in settings order', () => {
    expect(APP_LANGUAGE_OPTIONS.map((option) => option.value)).toEqual([
      'auto',
      'en',
      'zh',
      'zh-TW',
      'ja',
    ]);
    expect(APP_LANGUAGE_OPTIONS.map((option) => option.defaultLabel)).toEqual([
      'Automatic',
      'English',
      '简体中文',
      '繁體中文',
      '日本語',
    ]);
  });

  it('maps detected browser locales to supported app locales', () => {
    expect(resolveDetectedAppLanguage('zh-CN')).toBe('zh');
    expect(resolveDetectedAppLanguage('zh-SG')).toBe('zh');
    expect(resolveDetectedAppLanguage('zh-Hans')).toBe('zh');
    expect(resolveDetectedAppLanguage('zh-TW')).toBe('zh-TW');
    expect(resolveDetectedAppLanguage('zh-HK')).toBe('zh-TW');
    expect(resolveDetectedAppLanguage('zh-MO')).toBe('zh-TW');
    expect(resolveDetectedAppLanguage('zh-Hant-HK')).toBe('zh-TW');
    expect(resolveDetectedAppLanguage('ja-JP')).toBe('ja');
    expect(resolveDetectedAppLanguage('en-US')).toBe('en');
    expect(resolveDetectedAppLanguage('fr-FR')).toBe('en');
  });

  it('keeps explicit saved preferences and resolves auto through detection', () => {
    expect(resolveAppLanguagePreference('ja', 'en-US')).toBe('ja');
    expect(resolveAppLanguagePreference('zh-TW', 'en-US')).toBe('zh-TW');
    expect(resolveAppLanguagePreference('auto', 'zh-HK')).toBe('zh-TW');
    expect(resolveAppLanguagePreference('auto', 'ja-JP')).toBe('ja');
  });
});
