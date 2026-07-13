import { describe, expect, it } from 'vitest';
import en from '../en.json';
import ja from '../ja.json';
import ko from '../ko.json';
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

function flattenLocaleStrings(value: unknown, prefix = ''): Record<string, string> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return typeof value === 'string' && prefix ? { [prefix]: value } : {};
  }

  return Object.fromEntries(
    Object.entries(value).flatMap(([key, child]) => (
      Object.entries(flattenLocaleStrings(child, prefix ? `${prefix}.${key}` : key))
    )),
  );
}

function getInterpolationNames(value: string): string[] {
  return Array.from(value.matchAll(/{{\s*([\w.]+)\s*}}/g))
    .map((match) => match[1])
    .sort();
}

describe('locale resources', () => {
  it('provides localized context menu action labels', () => {
    const locales = { en, ja, ko, zh, 'zh-TW': zhTW } as const;

    for (const [locale, resource] of Object.entries(locales)) {
      expect(resource.common.open, `${locale}:common.open`).toBeTruthy();
      expect(
        getInterpolationNames(resource.common.actions_for),
        `${locale}:common.actions_for`,
      ).toEqual(['item']);
    }
  });

  it('keeps all UI locales at the same key parity', () => {
    const locales = {
      en,
      ja,
      ko,
      zh,
      'zh-TW': zhTW,
    };
    const baseline = flattenLocaleKeys(en).sort();

    for (const [locale, resource] of Object.entries(locales)) {
      expect(flattenLocaleKeys(resource).sort(), locale).toEqual(baseline);
    }
  });

  it('keeps interpolation variable names unchanged across locales', () => {
    const locales = {
      ja,
      ko,
      zh,
      'zh-TW': zhTW,
    };
    const baseline = flattenLocaleStrings(en);

    for (const [locale, resource] of Object.entries(locales)) {
      const localized = flattenLocaleStrings(resource);

      for (const [key, value] of Object.entries(baseline)) {
        expect(
          getInterpolationNames(localized[key] ?? ''),
          `${locale}:${key}`,
        ).toEqual(getInterpolationNames(value));
      }
    }
  });

  it('keeps Korean copy free of obvious machine-translation fragments', () => {
    const forbiddenFragments = [
      '실패 to',
      '전사본ion',
      '분절s',
      'pro파일',
      '선택ed',
      '사용d',
      '만들기d',
      '다듬기ing',
      '내보내기ing',
      '중지ped',
      '열기AI',
      '로컬host',
      'al준비됨',
      '로컬ly',
      'Pro파일',
      'archive imported',
      'archive uploaded',
      'This archive',
      'Replace current data',
      'could not be saved',
      'requires HTTPS',
      '진단 unavailable',
      'No microphone',
      'currently turned off',
      '필수 dependencies',
      '자동 matching',
      'text 다듬기',
      '다듬기 keyword',
      'Default 다듬기',
      'Default 요약',
      'Built-in',
      'Custom 요약',
      '편집ed',
      '자동으로 replace',
      'Add Pro파일',
      'Can appear',
      'Needs more usable',
      '선택 streaming',
      '오디오/video',
      '파일 processing',
    ];
    const strings = flattenLocaleStrings(ko);

    const offenders = Object.entries(strings)
      .filter(([, value]) => (
        forbiddenFragments.some((fragment) => value.includes(fragment))
      ))
      .map(([key, value]) => `${key}: ${value}`);

    expect(offenders).toEqual([]);
  });
});
