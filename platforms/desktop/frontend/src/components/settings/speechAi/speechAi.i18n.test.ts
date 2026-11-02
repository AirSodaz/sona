import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import en from '../../../locales/en.json';
import ja from '../../../locales/ja.json';
import ko from '../../../locales/ko.json';
import zh from '../../../locales/zh.json';
import zhTW from '../../../locales/zh-TW.json';

const locales = { en, zh, 'zh-TW': zhTW, ja, ko };

function leafPaths(value: unknown, prefix = ''): string[] {
  if (typeof value === 'string') return [prefix];
  if (!value || typeof value !== 'object') return [];
  return Object.entries(value).flatMap(([key, child]) =>
    leafPaths(child, prefix ? `${prefix}.${key}` : key)
  );
}

function readPath(source: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>((current, key) => {
    if (!current || typeof current !== 'object') return undefined;
    return (current as Record<string, unknown>)[key];
  }, source);
}

const EXTRA_KEYS = [
  'settings.llm.no_local_models_available',
  'settings.llm.temperature_reset_tooltip',
  'settings.llm.temperature_reset_aria',
  'settings.model_hub',
  'settings.model_mode_live',
  'settings.model_mode_batch',
  'settings.log_level_info',
  'settings.language_zh',
];

describe('Speech & AI locale coverage', () => {
  const keys = [
    ...leafPaths(en.settings.speech_ai).map((path) => `settings.speech_ai.${path}`),
    ...EXTRA_KEYS,
  ];

  it('has the same Speech & AI strings in every locale', () => {
    const missing = Object.entries(locales).flatMap(([name, locale]) =>
      keys
        .filter((key) => {
          const value = readPath(locale, key);
          return typeof value !== 'string' || value.trim().length === 0;
        })
        .map((key) => `${name}: ${key}`)
    );
    expect(missing).toEqual([]);
  });

  it('does not fall back to defaultValue in the Speech & AI source', () => {
    const dir = dirname(fileURLToPath(import.meta.url));
    const files = readdirSync(dir).filter(
      (name) => (name.endsWith('.ts') || name.endsWith('.tsx')) && !name.endsWith('.test.ts')
    );
    const offenders = files.flatMap((name) => {
      const text = readFileSync(join(dir, name), 'utf8');
      return text.includes('defaultValue') ? [name] : [];
    });
    expect(offenders).toEqual([]);
  });

  it('uses corner quotes in Chinese Speech & AI copy', () => {
    const straightOrCurly = /["“”]/;
    const offenders = (['zh', 'zh-TW'] as const).flatMap((name) =>
      leafPaths(locales[name].settings.speech_ai)
        .filter((path) =>
          straightOrCurly.test(String(readPath(locales[name], `settings.speech_ai.${path}`)))
        )
        .map((path) => `${name}: ${path}`)
    );
    expect(offenders).toEqual([]);
    expect(zh.settings.speech_ai.count_hint).toContain('「自动」');
    expect(zh.settings.speech_ai.language_hint).toContain('「录音时」');
    expect(zh.settings.speech_ai.language_hint).toContain('「打开文件时」');
    expect(zhTW.settings.speech_ai.count_hint).toContain('「自動」');
    expect(zhTW.settings.speech_ai.language_hint).toContain('「錄音時」');
    expect(zhTW.settings.speech_ai.language_hint).toContain('「開啟檔案時」');
  });

  it('does not use native title tooltips in the Speech & AI source', () => {
    const dir = dirname(fileURLToPath(import.meta.url));
    const files = readdirSync(dir).filter((name) => name.endsWith('.tsx'));
    const offenders = files.flatMap((name) => {
      const text = readFileSync(join(dir, name), 'utf8');
      const nativeTitle = text.includes(' title="') || text.includes(" title='");
      return nativeTitle ? [name] : [];
    });
    expect(offenders).toEqual([]);
  });
});
