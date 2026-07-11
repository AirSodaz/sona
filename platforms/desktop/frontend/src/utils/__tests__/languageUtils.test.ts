import { describe, it, expect } from 'vitest';
import { getLocalizedLanguageName } from '../languageUtils';

describe('getLocalizedLanguageName', () => {
  it('should format language names correctly based on active locale', () => {
    expect(getLocalizedLanguageName('fr', 'en')).toBe('French');
    expect(getLocalizedLanguageName('fr', 'zh')).toBe('法语');
  });

  it('should fall back gracefully on invalid codes', () => {
    expect(getLocalizedLanguageName('invalid-code', 'en')).toBe('invalid-code');
  });

  it('should handle custom overrides correctly', () => {
    expect(getLocalizedLanguageName('zh', 'zh')).toBe('中文 (简体)');
    expect(getLocalizedLanguageName('zh-TW', 'zh')).toBe('中文 (繁体)');
  });
});
