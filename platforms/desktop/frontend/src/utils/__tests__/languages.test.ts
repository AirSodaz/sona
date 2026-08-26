import { describe, expect, it } from 'vitest';
import {
  buildLanguagePickerOptions,
  coerceLanguage,
  languageDisplayName,
  type LanguageCapable,
} from '../languages';

const selectableModel: LanguageCapable = {
  languages: ['en', 'ja', 'ko', 'yue', 'zh'],
  languageMode: 'selectable',
};

describe('buildLanguagePickerOptions', () => {
  it('offers auto plus every supported language with a common group for selectable models', () => {
    const options = buildLanguagePickerOptions(selectableModel, 'en');

    expect(options[0]).toMatchObject({ value: 'auto' });
    expect(options.map((option) => option.value)).toEqual(['auto', 'en', 'ja', 'ko', 'yue', 'zh']);
    const grouped = options.filter((option) => option.group === 'common').map((option) => option.value);
    expect(grouped).toEqual(['en', 'ja', 'ko', 'yue', 'zh']);
  });

  it('locks auto-detect models to a single auto option', () => {
    const options = buildLanguagePickerOptions(
      { languages: ['ar', 'de', 'en'], languageMode: 'auto' },
      'en',
    );

    expect(options).toHaveLength(1);
    expect(options[0].value).toBe('auto');
  });

  it('locks fixed models to their single language', () => {
    const options = buildLanguagePickerOptions({ languages: ['zh'], languageMode: 'fixed' }, 'en');

    expect(options).toHaveLength(1);
    expect(options[0].value).toBe('zh');
  });

  it('falls back to auto when no model is resolved', () => {
    expect(buildLanguagePickerOptions(null, 'en')).toEqual([{ value: 'auto', label: 'Auto' }]);
  });
});

describe('coerceLanguage', () => {
  it('keeps configured selections that the active model supports', () => {
    expect(coerceLanguage(selectableModel, 'ja')).toBe('ja');
    expect(coerceLanguage(selectableModel, 'JA')).toBe('JA'.toLowerCase());
    expect(coerceLanguage(selectableModel, 'auto')).toBe('auto');
  });

  it('resets unsupported selections and locked modes to a safe value', () => {
    expect(coerceLanguage(selectableModel, 'fr')).toBe('auto');
    expect(coerceLanguage({ languages: ['ar', 'de'], languageMode: 'auto' }, 'ja')).toBe('auto');
    expect(coerceLanguage({ languages: ['zh'], languageMode: 'fixed' }, 'en')).toBe('zh');
    expect(coerceLanguage({ languages: [], languageMode: 'none' }, 'en')).toBe('auto');
  });

  it('passes through legacy entries without structured metadata', () => {
    expect(coerceLanguage({} as LanguageCapable, 'zh')).toBe('zh');
    expect(coerceLanguage(null, 'zh')).toBe('zh');
  });
});

describe('languageDisplayName', () => {
  it('localizes ISO codes via CLDR with an English fallback', () => {
    expect(languageDisplayName('zh', 'en').toLowerCase()).toContain('chinese');
    expect(languageDisplayName('yue', 'en').length).toBeGreaterThan(1);
    expect(languageDisplayName('zh', 'zh')).toBe('中文');
  });
});
