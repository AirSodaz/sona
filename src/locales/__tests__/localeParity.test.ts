import { describe, expect, it } from 'vitest';
import en from '../en.json';
import ja from '../ja.json';
import zh from '../zh.json';
import zhTW from '../zh-TW.json';

function flattenLocaleKeys(value: unknown, prefix = ''): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }

  return Object.entries(value).flatMap(([key, child]) => (
    flattenLocaleKeys(child, prefix ? `${prefix}.${key}` : key)
  ));
}

describe('locale resources', () => {
  it('keeps all UI locales at the same key parity', () => {
    const locales = {
      en,
      ja,
      zh,
      'zh-TW': zhTW,
    };
    const baseline = flattenLocaleKeys(en).sort();

    for (const [locale, resource] of Object.entries(locales)) {
      expect(flattenLocaleKeys(resource).sort(), locale).toEqual(baseline);
    }
  });
});
